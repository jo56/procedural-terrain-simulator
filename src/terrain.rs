use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wgpu::util::DeviceExt;
use wgpu::*;

use crate::camera::FlyCamera;

// Constants matching shader
const CHUNK_SIZE: u32 = 64;
const CHUNK_WORLD_SIZE: f32 = 256.0;
const VIEW_RADIUS: i32 = 16; // 33x33 chunks visible
const MAX_CHUNKS: usize = 1089; // 33x33 = 1089
const TERRAIN_WORKGROUP_SIZE: u32 = 8; // Must match @workgroup_size in shader

// Default rendering constants (for TerrainSettings::default())
// Note: Presets use different values (e.g., ambient 0.35 vs default 0.25)
pub const DEFAULT_FOG_START: f32 = 800.0;
pub const DEFAULT_FOG_DISTANCE: f32 = 3000.0;

/// Terrain generation settings that can be modified at runtime
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TerrainSettings {
    // Generation parameters
    pub terrain_scale: f32,
    pub height_scale: f32,
    pub octaves: u32,
    pub warp_strength: f32,
    pub height_variance: f32,
    pub roughness: f32,
    pub pattern_type: u32,
    pub seed: u32,

    // Lighting/fog
    pub ambient: f32,
    pub fog_start: f32,
    pub fog_distance: f32,

    // Terrain colors (RGB 0-1)
    pub color_abyss: [f32; 3],
    pub color_deep_water: [f32; 3],
    pub color_shallow_water: [f32; 3],
    pub color_sand: [f32; 3],
    pub color_grass: [f32; 3],
    pub color_rock: [f32; 3],
    pub color_snow: [f32; 3],
    pub color_sky: [f32; 3],

    // Sky gradient colors (RGB 0-1)
    pub color_sky_top: [f32; 3],
    pub color_sky_horizon: [f32; 3],
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            terrain_scale: 0.001,
            height_scale: 150.0,
            octaves: 2,
            warp_strength: 20.0,
            height_variance: 0.5,
            roughness: 0.35,
            pattern_type: 4,
            seed: 0,
            ambient: 0.25, // Note: Presets use PRESET_AMBIENT (0.35) instead
            fog_start: DEFAULT_FOG_START,
            fog_distance: DEFAULT_FOG_DISTANCE,
            color_abyss: [0.4, 0.4, 0.4],
            color_deep_water: [0.6, 0.6, 0.6],
            color_shallow_water: [0.7, 0.7, 0.7],
            color_sand: [0.85, 0.85, 0.85],
            color_grass: [0.75, 0.75, 0.75],
            color_rock: [0.9, 0.9, 0.9],
            color_snow: [0.98, 0.98, 0.98],
            color_sky: [0.05, 0.05, 0.05],
            color_sky_top: [0.02, 0.02, 0.02],
            color_sky_horizon: [0.15, 0.15, 0.15],
        }
    }
}

/// Chunk coordinate in chunk-space
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub fn from_world_pos(pos: Vec3) -> Self {
        Self {
            x: (pos.x / CHUNK_WORLD_SIZE).floor() as i32,
            z: (pos.z / CHUNK_WORLD_SIZE).floor() as i32,
        }
    }

    pub fn world_offset(&self) -> [f32; 2] {
        [
            self.x as f32 * CHUNK_WORLD_SIZE,
            self.z as f32 * CHUNK_WORLD_SIZE,
        ]
    }

    /// Test if this chunk's AABB is visible within the frustum planes
    /// Uses a conservative test - returns true if chunk might be visible
    pub fn is_visible_in_frustum(&self, frustum_planes: &[Vec4; 6], height_scale: f32) -> bool {
        let offset = self.world_offset();

        // Chunk AABB bounds
        let min_x = offset[0];
        let max_x = offset[0] + CHUNK_WORLD_SIZE;
        let min_z = offset[1];
        let max_z = offset[1] + CHUNK_WORLD_SIZE;
        // Use conservative height bounds (terrain can go from -height_scale to +height_scale)
        let min_y = -height_scale * 0.5;
        let max_y = height_scale;

        // Test AABB against each frustum plane
        for plane in frustum_planes {
            // Find the corner of the AABB most aligned with the plane normal (p-vertex)
            let px = if plane.x >= 0.0 { max_x } else { min_x };
            let py = if plane.y >= 0.0 { max_y } else { min_y };
            let pz = if plane.z >= 0.0 { max_z } else { min_z };

            // If the p-vertex is outside this plane, the AABB is completely outside the frustum
            let dist = plane.x * px + plane.y * py + plane.z * pz + plane.w;
            if dist < 0.0 {
                return false;
            }
        }

        true
    }
}

/// State of a chunk slot
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChunkState {
    Empty,
    Ready,
}

/// A reusable slot for chunk data
pub struct ChunkSlot {
    pub state: ChunkState,
    pub coord: Option<ChunkCoord>,
    pub params_buffer: Buffer,
    pub _height_buffer: Buffer,
    pub uniform_buffer: Buffer,
    pub compute_bind_group: BindGroup,
    pub render_bind_group: BindGroup,
    pub last_used_frame: u64,
}

/// Compute shader parameters - must match shader layout
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ComputeParams {
    chunk_offset: [f32; 2],
    terrain_scale: f32,
    height_scale: f32,
    octaves: u32,
    warp_strength: f32,
    height_variance: f32,
    roughness: f32,
    pattern_type: u32,
    seed: u32,
    _padding: [f32; 2], // Align to 16 bytes
}

/// Fragment shader color parameters - must match shader layout
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ColorParams {
    color_abyss: [f32; 4],         // w unused, for alignment
    color_deep_water: [f32; 4],
    color_shallow_water: [f32; 4],
    color_sand: [f32; 4],
    color_grass: [f32; 4],
    color_rock: [f32; 4],
    color_snow: [f32; 4],
    color_sky: [f32; 4],
    color_sky_top: [f32; 4],
    color_sky_horizon: [f32; 4],
    ambient: f32,
    fog_start: f32,
    fog_distance: f32,
    _padding: f32,
}

/// Convert RGB color to RGBA with alpha=1.0 for shader uniform alignment
fn rgb_to_rgba(rgb: [f32; 3]) -> [f32; 4] {
    [rgb[0], rgb[1], rgb[2], 1.0]
}

/// Per-chunk uniforms for rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ChunkUniform {
    chunk_offset: [f32; 2],
    _padding: [f32; 2],
}

/// Vertex data for terrain grid
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TerrainVertex {
    local_uv: [f32; 2],
}

impl TerrainVertex {
    const ATTRIBS: [VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];

    fn desc() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<TerrainVertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Manages terrain chunks, streaming, and rendering
pub struct TerrainRenderer {
    // Shared geometry
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,

    // Chunk pool
    slots: Vec<ChunkSlot>,
    coord_to_slot: HashMap<ChunkCoord, usize>,
    current_frame: u64,

    // Pipelines
    compute_pipeline: ComputePipeline,
    render_pipeline: RenderPipeline,

    // Bind group layout for compute shader
    _compute_bind_group_layout: BindGroupLayout,

    // Camera uniform buffer
    camera_uniform_buffer: Buffer,
    camera_bind_group: BindGroup,

    // Color uniform buffer
    color_uniform_buffer: Buffer,
    color_bind_group: BindGroup,

    // Terrain settings
    pub settings: TerrainSettings,
    needs_regeneration: bool,
}

impl TerrainRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        settings: TerrainSettings,
    ) -> Result<Self, String> {
        // Load shader
        let shader_source = include_str!("../shaders/terrain.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Terrain Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layouts
        let compute_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let chunk_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Chunk Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let color_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Color Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create compute pipeline
        let compute_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[&compute_bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Height Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: Some("compute_height"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create render pipeline
        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &chunk_bind_group_layout, &color_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Terrain Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TerrainVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: crate::webgpu::GpuState::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create shared geometry
        let (vertex_buffer, index_buffer, index_count) = Self::create_grid_buffers(device);

        // Create camera uniform buffer
        let camera_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Camera Uniform Buffer"),
            size: std::mem::size_of::<crate::camera::CameraUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });

        // Create color uniform buffer
        let color_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Color Uniform Buffer"),
            size: std::mem::size_of::<ColorParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let color_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Color Bind Group"),
            layout: &color_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: color_uniform_buffer.as_entire_binding(),
            }],
        });

        // Pre-allocate chunk slots
        let mut slots = Vec::with_capacity(MAX_CHUNKS);
        for i in 0..MAX_CHUNKS {
            slots.push(Self::create_chunk_slot(
                device,
                &chunk_bind_group_layout,
                &compute_bind_group_layout,
                i,
            ));
        }

        let mut renderer = Self {
            vertex_buffer,
            index_buffer,
            index_count,
            slots,
            coord_to_slot: HashMap::new(),
            current_frame: 0,
            compute_pipeline,
            render_pipeline,
            _compute_bind_group_layout: compute_bind_group_layout,
            camera_uniform_buffer,
            camera_bind_group,
            color_uniform_buffer,
            color_bind_group,
            settings,
            needs_regeneration: false,
        };

        // Generate initial chunks around origin
        renderer.generate_initial_chunks(device, queue);

        Ok(renderer)
    }

    fn create_grid_buffers(device: &Device) -> (Buffer, Buffer, u32) {
        // Create vertex buffer (UV coordinates)
        let mut vertices = Vec::with_capacity((CHUNK_SIZE * CHUNK_SIZE) as usize);
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let u = x as f32 / (CHUNK_SIZE - 1) as f32;
                let v = z as f32 / (CHUNK_SIZE - 1) as f32;
                vertices.push(TerrainVertex { local_uv: [u, v] });
            }
        }

        let vertex_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Terrain Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        // Create index buffer
        let mut indices: Vec<u32> = Vec::new();
        for z in 0..(CHUNK_SIZE - 1) {
            for x in 0..(CHUNK_SIZE - 1) {
                let tl = z * CHUNK_SIZE + x;
                let tr = tl + 1;
                let bl = tl + CHUNK_SIZE;
                let br = bl + 1;

                // Two triangles per quad
                indices.push(tl);
                indices.push(bl);
                indices.push(tr);
                indices.push(tr);
                indices.push(bl);
                indices.push(br);
            }
        }

        let index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Terrain Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        (vertex_buffer, index_buffer, indices.len() as u32)
    }

    fn create_chunk_slot(
        device: &Device,
        chunk_bind_group_layout: &BindGroupLayout,
        compute_bind_group_layout: &BindGroupLayout,
        index: usize,
    ) -> ChunkSlot {
        let height_count = CHUNK_SIZE * CHUNK_SIZE;

        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("Chunk {} Params Buffer", index)),
            size: std::mem::size_of::<ComputeParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let height_buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("Chunk {} Height Buffer", index)),
            size: (height_count * 4) as u64, // f32 per vertex
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("Chunk {} Uniform Buffer", index)),
            size: std::mem::size_of::<ChunkUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let render_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("Chunk {} Render Bind Group", index)),
            layout: chunk_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: height_buffer.as_entire_binding(),
                },
            ],
        });

        let compute_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("Chunk {} Compute Bind Group", index)),
            layout: compute_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: height_buffer.as_entire_binding(),
                },
            ],
        });

        ChunkSlot {
            state: ChunkState::Empty,
            coord: None,
            params_buffer,
            _height_buffer: height_buffer,
            uniform_buffer,
            compute_bind_group,
            render_bind_group,
            last_used_frame: 0,
        }
    }

    fn generate_initial_chunks(&mut self, device: &Device, queue: &Queue) {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Initial Chunks Encoder"),
        });

        for dz in -VIEW_RADIUS..=VIEW_RADIUS {
            for dx in -VIEW_RADIUS..=VIEW_RADIUS {
                let coord = ChunkCoord::new(dx, dz);
                self.generate_chunk(queue, &mut encoder, coord);
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        log::info!(
            "Generated {} initial chunks",
            (VIEW_RADIUS * 2 + 1) * (VIEW_RADIUS * 2 + 1)
        );
    }

    fn generate_chunk(
        &mut self,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        coord: ChunkCoord,
    ) {
        // Find a free slot or recycle LRU
        let slot_idx = self.get_free_slot();

        // Remove old mapping if recycling
        if let Some(old_coord) = self.slots[slot_idx].coord {
            self.coord_to_slot.remove(&old_coord);
        }

        // Setup slot
        let slot = &mut self.slots[slot_idx];
        slot.state = ChunkState::Ready;
        slot.coord = Some(coord);
        slot.last_used_frame = self.current_frame;

        self.coord_to_slot.insert(coord, slot_idx);

        // Update chunk uniform
        let chunk_uniform = ChunkUniform {
            chunk_offset: coord.world_offset(),
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&slot.uniform_buffer, 0, bytemuck::cast_slice(&[chunk_uniform]));

        // Dispatch compute shader
        let compute_params = ComputeParams {
            chunk_offset: coord.world_offset(),
            terrain_scale: self.settings.terrain_scale,
            height_scale: self.settings.height_scale,
            octaves: self.settings.octaves,
            warp_strength: self.settings.warp_strength,
            height_variance: self.settings.height_variance,
            roughness: self.settings.roughness,
            pattern_type: self.settings.pattern_type,
            seed: self.settings.seed,
            _padding: [0.0, 0.0],
        };

        queue.write_buffer(&slot.params_buffer, 0, bytemuck::cast_slice(&[compute_params]));

        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Height Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &slot.compute_bind_group, &[]);

            let workgroups = (CHUNK_SIZE + TERRAIN_WORKGROUP_SIZE - 1) / TERRAIN_WORKGROUP_SIZE;
            compute_pass.dispatch_workgroups(workgroups, workgroups, 1);
        }
    }

    fn get_free_slot(&mut self) -> usize {
        // First try to find an empty slot
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.state == ChunkState::Empty {
                return i;
            }
        }

        // Otherwise find LRU slot
        let mut oldest_frame = u64::MAX;
        let mut oldest_idx = 0;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.last_used_frame < oldest_frame {
                oldest_frame = slot.last_used_frame;
                oldest_idx = i;
            }
        }
        oldest_idx
    }

    pub fn update(&mut self, device: &Device, queue: &Queue, camera_pos: Vec3) {
        self.current_frame += 1;

        let camera_chunk = ChunkCoord::from_world_pos(camera_pos);

        // Determine needed chunks
        let visible_span = (VIEW_RADIUS * 2 + 1) as usize;
        let mut needed_chunks = Vec::with_capacity(visible_span * visible_span);
        for dz in -VIEW_RADIUS..=VIEW_RADIUS {
            for dx in -VIEW_RADIUS..=VIEW_RADIUS {
                let coord = ChunkCoord::new(camera_chunk.x + dx, camera_chunk.z + dz);
                needed_chunks.push(coord);
            }
        }

        // Mark existing chunks as used
        for coord in &needed_chunks {
            if let Some(&slot_idx) = self.coord_to_slot.get(coord) {
                self.slots[slot_idx].last_used_frame = self.current_frame;
            }
        }

        // Generate missing chunks
        let mut encoder: Option<CommandEncoder> = None;
        for coord in needed_chunks {
            if !self.coord_to_slot.contains_key(&coord) {
                let encoder_ref = encoder.get_or_insert_with(|| {
                    device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("Chunk Update Encoder"),
                    })
                });
                self.generate_chunk(queue, encoder_ref, coord);
            }
        }

        if let Some(encoder) = encoder {
            queue.submit(std::iter::once(encoder.finish()));
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        color_view: &TextureView,
        depth_view: &TextureView,
        camera: &FlyCamera,
        queue: &Queue,
    ) {
        // Update camera uniform
        queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::cast_slice(&[camera.uniform_data()]),
        );

        // Update color uniform
        let color_params = ColorParams {
            color_abyss: rgb_to_rgba(self.settings.color_abyss),
            color_deep_water: rgb_to_rgba(self.settings.color_deep_water),
            color_shallow_water: rgb_to_rgba(self.settings.color_shallow_water),
            color_sand: rgb_to_rgba(self.settings.color_sand),
            color_grass: rgb_to_rgba(self.settings.color_grass),
            color_rock: rgb_to_rgba(self.settings.color_rock),
            color_snow: rgb_to_rgba(self.settings.color_snow),
            color_sky: rgb_to_rgba(self.settings.color_sky),
            color_sky_top: rgb_to_rgba(self.settings.color_sky_top),
            color_sky_horizon: rgb_to_rgba(self.settings.color_sky_horizon),
            ambient: self.settings.ambient,
            fog_start: self.settings.fog_start,
            fog_distance: self.settings.fog_distance,
            _padding: 0.0,
        };
        queue.write_buffer(
            &self.color_uniform_buffer,
            0,
            bytemuck::cast_slice(&[color_params]),
        );

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Terrain Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: self.settings.color_sky_horizon[0] as f64,
                            g: self.settings.color_sky_horizon[1] as f64,
                            b: self.settings.color_sky_horizon[2] as f64,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(2, &self.color_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);

            // Extract frustum planes for culling
            let frustum_planes = camera.extract_frustum_planes();
            let height_scale = self.settings.height_scale;

            // Draw only visible chunks (frustum culling)
            for slot in &self.slots {
                if slot.state == ChunkState::Ready {
                    if let Some(coord) = slot.coord {
                        // Skip chunks outside the camera frustum
                        if !coord.is_visible_in_frustum(&frustum_planes, height_scale) {
                            continue;
                        }
                    }
                    render_pass.set_bind_group(1, &slot.render_bind_group, &[]);
                    render_pass.draw_indexed(0..self.index_count, 0, 0..1);
                }
            }
        }
    }

    /// Update terrain settings and mark for regeneration
    pub fn update_settings(&mut self, settings: TerrainSettings) {
        self.settings = settings;
        self.needs_regeneration = true;
        log::info!("Terrain settings updated, regeneration queued");
    }

    /// Queue terrain regeneration with current settings (e.g., from R key)
    pub fn queue_regeneration(&mut self) {
        self.needs_regeneration = true;
        log::info!("Terrain regeneration queued");
    }

    /// Regenerate all chunks with current settings
    pub fn regenerate_all_chunks(&mut self, device: &Device, queue: &Queue, camera_pos: Vec3) {
        // Clear all chunks
        for slot in &mut self.slots {
            slot.state = ChunkState::Empty;
            slot.coord = None;
        }
        self.coord_to_slot.clear();

        // Generate chunks around camera position
        let camera_chunk = ChunkCoord::from_world_pos(camera_pos);

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Regenerate Chunks Encoder"),
        });

        for dz in -VIEW_RADIUS..=VIEW_RADIUS {
            for dx in -VIEW_RADIUS..=VIEW_RADIUS {
                let coord = ChunkCoord::new(camera_chunk.x + dx, camera_chunk.z + dz);
                self.generate_chunk(queue, &mut encoder, coord);
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        self.needs_regeneration = false;
        log::info!("Regenerated all terrain chunks");
    }

    /// Check if regeneration is needed and perform it
    pub fn check_regeneration(&mut self, device: &Device, queue: &Queue, camera_pos: Vec3) {
        if self.needs_regeneration {
            self.regenerate_all_chunks(device, queue, camera_pos);
        }
    }
}
