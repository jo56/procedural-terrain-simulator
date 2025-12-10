use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use wgpu::*;

/// Maximum number of particles
const MAX_PARTICLES: u32 = 50000;

/// Workgroup size for particle compute shader (must match @workgroup_size in shader)
const PARTICLE_WORKGROUP_SIZE: u32 = 256;

/// Golden ratio for uniform distribution of particles
const GOLDEN_RATIO: f32 = 0.618034;

/// Multiplier for converting density setting to particle count
const PARTICLE_DENSITY_MULTIPLIER: f32 = 10000.0;

/// Particle settings that can be modified at runtime
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ParticleSettings {
    pub particle_type: u32,       // 0=rain, 1=snow
    pub density: f32,             // Affects particle count
    pub max_particles: u32,       // Cap on particle count
    pub speed: f32,               // Fall speed
    pub wind_x: f32,              // Wind in X direction
    pub wind_z: f32,              // Wind in Z direction
    pub particle_size: f32,       // Size multiplier
    pub particle_color: [f32; 4], // RGBA color
    pub spawn_height: f32,        // Height above camera to spawn
    pub spawn_radius: f32,        // Radius around camera to spawn
}

impl Default for ParticleSettings {
    fn default() -> Self {
        Self {
            particle_type: 0,
            density: 0.0,  // 0 = disabled
            max_particles: 10000,
            speed: 25.0,
            wind_x: 0.0,
            wind_z: 0.0,
            particle_size: 0.5,  // Smaller particles
            particle_color: [0.7, 0.8, 0.9, 0.6],
            spawn_height: 100.0,  // Reduced spawn height
            spawn_radius: 300.0,
        }
    }
}

/// A single particle - must match WGSL struct layout exactly
/// WGSL vec3f has 16-byte alignment, so we need explicit padding
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Particle {
    position: [f32; 3],   // offset 0, size 12
    _pad1: f32,           // offset 12, size 4 (align velocity to 16-byte boundary)
    velocity: [f32; 3],   // offset 16, size 12
    life: f32,            // offset 28, size 4 (no padding - f32 only needs 4-byte align)
    size: f32,            // offset 32, size 4
    _pad2: [f32; 3],      // offset 36, size 12 (pad struct to 48 bytes)
}

/// Simulation parameters for compute shader - must match WGSL layout
/// WGSL vec3f has 16-byte alignment
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct SimParams {
    delta_time: f32,
    time: f32,
    _pad1: [f32; 2],      // Pad to align camera_pos to offset 16
    camera_pos: [f32; 3], // vec3f at offset 16
    wind_x: f32,          // Fills the vec3f padding (offset 28)
    wind_z: f32,
    spawn_height: f32,
    spawn_radius: f32,
    despawn_height: f32,
    particle_type: u32,
    speed: f32,
    particle_count: u32,
    _padding: f32,
}

/// Render parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct RenderParams {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    particle_size: f32,
    particle_color: [f32; 4],
    particle_type: u32,
    time: f32,
    _padding: [f32; 2],
}

/// GPU-accelerated particle system
pub struct ParticleSystem {
    // Double-buffered particle storage (ping-pong)
    particle_buffers: [Buffer; 2],
    current_buffer: usize,

    // Compute pipeline
    compute_pipeline: ComputePipeline,
    compute_bind_groups: [BindGroup; 2],
    sim_params_buffer: Buffer,

    // Render pipeline
    render_pipeline: RenderPipeline,
    render_bind_groups: [BindGroup; 2],
    render_params_buffer: Buffer,

    // Settings
    pub settings: ParticleSettings,
    active_particle_count: u32,
    current_time: f32,
    initialized: bool,
}

impl ParticleSystem {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Result<Self, String> {
        // Load shader
        let shader_source = include_str!("../shaders/particles.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Particles Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create particle buffers (double-buffered)
        let particle_buffer_size = (MAX_PARTICLES as usize * std::mem::size_of::<Particle>()) as u64;
        let particle_buffers = [
            device.create_buffer(&BufferDescriptor {
                label: Some("Particle Buffer A"),
                size: particle_buffer_size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&BufferDescriptor {
                label: Some("Particle Buffer B"),
                size: particle_buffer_size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];

        // Create simulation params buffer
        let sim_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sim Params Buffer"),
            size: std::mem::size_of::<SimParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create render params buffer
        let render_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Render Params Buffer"),
            size: std::mem::size_of::<RenderParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create compute bind group layout
        let compute_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Particle Compute Bind Group Layout"),
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
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
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

        // Create compute bind groups for ping-pong
        let compute_bind_groups = [
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Particle Compute Bind Group 0"),
                layout: &compute_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Particle Compute Bind Group 1"),
                layout: &compute_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                ],
            }),
        ];

        // Create compute pipeline
        let compute_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Particle Compute Pipeline Layout"),
            bind_group_layouts: &[&compute_bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Particle Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: Some("simulate"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create render bind group layout - uniform is used in both vertex and fragment
        let render_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Particle Render Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
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

        // Create render bind groups (one for each buffer)
        let render_bind_groups = [
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Particle Render Bind Group 0"),
                layout: &render_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: render_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Particle Render Bind Group 1"),
                layout: &render_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: render_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                ],
            }),
        ];

        // Create render pipeline
        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Particle Render Pipeline Layout"),
            bind_group_layouts: &[&render_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Particle Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_particle"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_particle"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: crate::webgpu::GpuState::DEPTH_FORMAT,
                depth_write_enabled: false, // Particles don't write depth
                depth_compare: CompareFunction::Less, // But are occluded by terrain
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            particle_buffers,
            current_buffer: 0,
            compute_pipeline,
            compute_bind_groups,
            sim_params_buffer,
            render_pipeline,
            render_bind_groups,
            render_params_buffer,
            settings: ParticleSettings::default(),
            active_particle_count: 0,
            current_time: 0.0,
            initialized: false,
        })
    }

    /// Initialize particles around camera
    fn initialize_particles(&mut self, queue: &Queue, camera_pos: Vec3) {
        let count = self.calculate_particle_count();
        if count == 0 {
            self.active_particle_count = 0;
            self.initialized = true;
            return;
        }

        let mut particles: Vec<Particle> = Vec::with_capacity(count as usize);

        for i in 0..count {
            // Random position around camera
            let seed = i as f32 * GOLDEN_RATIO;
            let angle = seed * std::f32::consts::TAU * 100.0;
            let radius = (seed * 123.456).fract() * self.settings.spawn_radius;
            let height = camera_pos.y + self.settings.spawn_height * (seed * 789.0).fract();

            let x = camera_pos.x + angle.cos() * radius;
            let z = camera_pos.z + angle.sin() * radius;

            // Initial velocity based on particle type
            let velocity = match self.settings.particle_type {
                0 => [self.settings.wind_x * 0.1, -self.settings.speed, self.settings.wind_z * 0.1],
                1 => [self.settings.wind_x * 0.05, -self.settings.speed * 0.3, self.settings.wind_z * 0.05],
                _ => [0.0, -self.settings.speed, 0.0],
            };

            particles.push(Particle {
                position: [x, height, z],
                _pad1: 0.0,
                velocity,
                life: 1.0 + (seed * 999.0).fract() * 7.0, // Random 1-8 seconds for staggered respawning
                size: 0.8 + (seed * 123.0).fract() * 0.4, // 0.8 to 1.2
                _pad2: [0.0, 0.0, 0.0],
            });
        }

        self.active_particle_count = count;

        // Write to BOTH buffers to ensure compute shader always has valid data
        // (ping-pong double buffering requires both buffers to be initialized)
        queue.write_buffer(&self.particle_buffers[0], 0, bytemuck::cast_slice(&particles));
        queue.write_buffer(&self.particle_buffers[1], 0, bytemuck::cast_slice(&particles));

        self.initialized = true;
        log::info!("Initialized {} particles", count);
    }

    /// Calculate particle count based on density
    fn calculate_particle_count(&self) -> u32 {
        if self.settings.density <= 0.0 {
            return 0;
        }
        let base_count = (self.settings.density * PARTICLE_DENSITY_MULTIPLIER) as u32;
        base_count.min(self.settings.max_particles).min(MAX_PARTICLES)
    }

    /// Update particle settings with validation
    pub fn update_settings(&mut self, settings: ParticleSettings) {
        // Validate incoming settings
        if settings.density.is_nan() {
            log::warn!("Invalid density value (NaN), ignoring settings update");
            return;
        }
        if settings.spawn_radius <= 0.0 || settings.spawn_height <= 0.0 {
            log::warn!("Invalid spawn radius or height, ignoring settings update");
            return;
        }

        self.settings = settings;

        // Always reinitialize when settings change to ensure fresh particle state
        // This fixes issues where stale/corrupted particle data persists
        self.initialized = false;
    }

    /// Force particle system to reinitialize on next update
    /// Useful for recovering from invalid state
    // Currently unused since particles are disabled by default, but kept here for future use
    #[allow(dead_code)]
    pub fn force_reinitialize(&mut self) {
        self.initialized = false;
        self.active_particle_count = 0;
        log::info!("Particle system marked for reinitialization");
    }

    /// Update simulation - adds compute pass to the provided encoder
    /// The encoder should be submitted by the caller after all passes are added
    pub fn update(&mut self, encoder: &mut CommandEncoder, queue: &Queue, camera_pos: Vec3, dt: f32) {
        self.current_time += dt;

        // Validate camera position - skip if invalid
        if camera_pos.x.is_nan() || camera_pos.y.is_nan() || camera_pos.z.is_nan() {
            log::warn!("Invalid camera position (NaN), skipping particle update");
            return;
        }

        // Skip if no particles
        if self.settings.density <= 0.0 {
            self.active_particle_count = 0;
            return;
        }

        // Initialize particles if needed
        if !self.initialized {
            self.initialize_particles(queue, camera_pos);
        }

        if self.active_particle_count == 0 {
            return;
        }

        // Update simulation params
        let sim_params = SimParams {
            delta_time: dt.min(0.1), // Cap delta time
            time: self.current_time,
            _pad1: [0.0, 0.0], // Align camera_pos to 16-byte boundary
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            wind_x: self.settings.wind_x,
            wind_z: self.settings.wind_z,
            spawn_height: self.settings.spawn_height,
            spawn_radius: self.settings.spawn_radius,
            despawn_height: camera_pos.y - 50.0,
            particle_type: self.settings.particle_type,
            speed: self.settings.speed,
            particle_count: self.active_particle_count,
            _padding: 0.0,
        };
        queue.write_buffer(&self.sim_params_buffer, 0, bytemuck::cast_slice(&[sim_params]));

        // Add compute pass to the shared encoder (no separate submit!)
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Particle Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_groups[self.current_buffer], &[]);

            let workgroups = (self.active_particle_count + PARTICLE_WORKGROUP_SIZE - 1) / PARTICLE_WORKGROUP_SIZE;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Swap buffers - compute wrote to the "other" buffer, which render will now read from
        self.current_buffer = 1 - self.current_buffer;
    }

    /// Render particles
    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        color_view: &TextureView,
        depth_view: &TextureView,
        camera_view_proj: [[f32; 4]; 4],
        camera_pos: Vec3,
        queue: &Queue,
    ) {
        // Skip if no particles
        if self.active_particle_count == 0 {
            return;
        }

        // Update render params
        let render_params = RenderParams {
            view_proj: camera_view_proj,
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            particle_size: self.settings.particle_size,
            particle_color: self.settings.particle_color,
            particle_type: self.settings.particle_type,
            time: self.current_time,
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.render_params_buffer, 0, bytemuck::cast_slice(&[render_params]));

        // Render pass
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Particle Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.render_bind_groups[self.current_buffer], &[]);

        // Draw 6 vertices per particle (2 triangles for billboard quad)
        render_pass.draw(0..6, 0..self.active_particle_count);
    }
}
