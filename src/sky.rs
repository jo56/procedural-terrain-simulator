use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use wgpu::*;

/// Maximum number of sky objects
const MAX_STARS: u32 = 8000;
const MAX_CELESTIAL: u32 = 200; // Suns and moons combined

/// Default parallax factor for moons
pub const DEFAULT_MOON_PARALLAX: f32 = 0.08;

/// Sky sphere radii for different object types
const STAR_SPHERE_RADIUS: f32 = 1000.0;
const SUN_SPHERE_RADIUS: f32 = 800.0;
const MOON_SPHERE_RADIUS: f32 = 900.0;

/// Phi angle constraints for sky object placement
const STAR_PHI_MULTIPLIER: f32 = 0.95;  // Stars cover 0 to ~86 degrees
const CELESTIAL_PHI_MIN: f32 = 0.05;    // Minimum angle (~3 degrees) for suns/moons
const CELESTIAL_PHI_RANGE: f32 = 1.52;  // Range up to ~90 degrees

/// Minimum Y position for sky objects (prevents objects below horizon)
const STAR_Y_MIN: f32 = 0.01;
const CELESTIAL_Y_MIN: f32 = 0.05;

/// Seed offsets for generating unique positions for suns and moons
const SUN_SEED_OFFSET: u32 = 10000;
const MOON_SEED_OFFSET: u32 = 20000;

/// Types of sky objects for unified generation
#[derive(Clone, Copy)]
enum SkyObjectType {
    Star,
    Sun,
    Moon,
}

/// Configuration for sky object generation
struct SkyObjectConfig {
    sphere_radius: f32,
    phi_min: f32,
    phi_range: f32,
    y_min: f32,
}

impl SkyObjectConfig {
    fn for_type(obj_type: SkyObjectType) -> Self {
        match obj_type {
            SkyObjectType::Star => Self {
                sphere_radius: STAR_SPHERE_RADIUS,
                phi_min: 0.0,
                phi_range: std::f32::consts::FRAC_PI_2 * STAR_PHI_MULTIPLIER,
                y_min: STAR_Y_MIN,
            },
            SkyObjectType::Sun => Self {
                sphere_radius: SUN_SPHERE_RADIUS,
                phi_min: CELESTIAL_PHI_MIN,
                phi_range: CELESTIAL_PHI_RANGE,
                y_min: CELESTIAL_Y_MIN,
            },
            SkyObjectType::Moon => Self {
                sphere_radius: MOON_SPHERE_RADIUS,
                phi_min: CELESTIAL_PHI_MIN,
                phi_range: CELESTIAL_PHI_RANGE,
                y_min: CELESTIAL_Y_MIN,
            },
        }
    }
}

/// Sky object settings that can be modified at runtime
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SkySettings {
    // Star settings
    pub star_count: u32,
    pub star_size_min: f32,
    pub star_size_max: f32,
    pub star_color: [f32; 3],
    pub star_twinkle_speed: f32,
    pub star_parallax: f32,

    // Sun settings
    pub sun_count: u32,
    pub sun_size: f32,
    pub sun_color: [f32; 3],
    pub sun_parallax: f32,

    // Moon settings
    pub moon_count: u32,
    pub moon_size: f32,
    pub moon_color: [f32; 3],
    pub moon_parallax: f32,

    // Random seed for object placement
    pub seed: u32,
}

impl Default for SkySettings {
    fn default() -> Self {
        Self {
            star_count: 4000,
            star_size_min: 0.5,
            star_size_max: 2.0,
            star_color: [0.95, 0.95, 0.95],   // Matches chalk theme
            star_twinkle_speed: 1.0,
            star_parallax: 0.1,
            sun_count: 60,
            sun_size: 50.0,
            sun_color: [1.0, 1.0, 1.0],       // Matches chalk theme
            sun_parallax: 0.05,
            moon_count: 60,
            moon_size: 30.0,
            moon_color: [0.9, 0.9, 0.9],      // Matches chalk theme
            moon_parallax: DEFAULT_MOON_PARALLAX,
            seed: 0,
        }
    }
}

/// A single sky object (star, sun, or moon)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct SkyObject {
    position: [f32; 3],      // Position on sky sphere
    size: f32,               // Object size
    color: [f32; 3],         // Object color
    object_type: u32,        // 0=star, 1=sun, 2=moon
    seed: f32,               // For twinkle animation
    parallax_factor: f32,    // Parallax strength
    _padding: [f32; 2],      // Align to 48 bytes
}

/// Sky uniforms for shaders
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct SkyUniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    time: f32,
}

#[derive(Default, Copy, Clone)]
struct SkyObjectCounts {
    stars: u32,
    suns: u32,
    moons: u32,
}

struct SkyGeneration {
    objects: Vec<SkyObject>,
    counts: SkyObjectCounts,
}

/// Manages sky objects and rendering
pub struct SkyRenderer {
    // Object storage
    object_buffer: Buffer,
    object_count: u32,
    object_cache: Vec<SkyObject>,

    // Uniforms
    uniform_buffer: Buffer,
    bind_group: BindGroup,

    // Pipeline
    render_pipeline: RenderPipeline,

    // Settings
    pub settings: SkySettings,
    needs_regeneration: bool,
    objects_dirty: bool,
    current_time: f32,
}

impl SkyRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Result<Self, String> {
        // Load shader
        let shader_source = include_str!("../shaders/sky.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sky Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sky Bind Group Layout"),
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

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Sky Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline (no depth test, additive blend)
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Sky Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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
                cull_mode: None, // No culling for billboards
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None, // No depth testing - sky is always behind
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create object buffer (pre-allocate max size)
        let max_objects = MAX_STARS + MAX_CELESTIAL;
        let object_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sky Object Buffer"),
            size: (max_objects as usize * std::mem::size_of::<SkyObject>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sky Uniform Buffer"),
            size: std::mem::size_of::<SkyUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Sky Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: object_buffer.as_entire_binding(),
                },
            ],
        });

        let mut renderer = Self {
            object_buffer,
            object_count: 0,
            object_cache: Vec::new(),
            uniform_buffer,
            bind_group,
            render_pipeline,
            settings: SkySettings::default(),
            needs_regeneration: false,
            objects_dirty: true,
            current_time: 0.0,
        };

        // Generate initial objects
        renderer.regenerate_objects();

        Ok(renderer)
    }

    /// Simple hash function for pseudo-random generation
    fn hash(n: u32) -> f32 {
        let mut x = n;
        x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
        x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
        x = (x >> 16) ^ x;
        (x as f32) / (u32::MAX as f32)
    }

    /// Generate all sky objects (stars, suns, moons) based on current settings
    fn generate_all_objects(&self) -> SkyGeneration {
        let mut objects: Vec<SkyObject> = Vec::new();
        let base_seed = self.settings.seed;

        // Generate stars
        let star_count = self.settings.star_count.min(MAX_STARS);
        for i in 0..star_count {
            objects.push(self.generate_star(base_seed.wrapping_add(i)));
        }

        // Generate suns
        let sun_count = self.settings.sun_count.min(MAX_CELESTIAL);
        for i in 0..sun_count {
            objects.push(self.generate_sun(base_seed.wrapping_add(SUN_SEED_OFFSET + i)));
        }

        // Generate moons
        let moon_count = self.settings.moon_count.min(MAX_CELESTIAL - sun_count);
        for i in 0..moon_count {
            objects.push(self.generate_moon(base_seed.wrapping_add(MOON_SEED_OFFSET + i)));
        }

        SkyGeneration {
            objects,
            counts: SkyObjectCounts {
                stars: star_count,
                suns: sun_count,
                moons: moon_count,
            },
        }
    }

    /// Generate sky objects based on current settings
    pub fn regenerate_objects(&mut self) {
        let generation = self.generate_all_objects();
        self.object_cache = generation.objects;
        self.object_count = self.object_cache.len() as u32;
        self.needs_regeneration = false;
        self.objects_dirty = true;
        log::info!(
            "Generated {} sky objects ({} stars, {} suns, {} moons)",
            self.object_count,
            generation.counts.stars,
            generation.counts.suns,
            generation.counts.moons
        );
    }

    /// Update settings and mark for regeneration if needed
    pub fn update_settings(&mut self, settings: SkySettings) {
        if self.settings != settings {
            self.settings = settings;
            self.needs_regeneration = true;
        }
    }

    /// Check if regeneration is needed and perform it
    pub fn check_regeneration(&mut self) {
        if self.needs_regeneration {
            self.regenerate_objects();
        }
    }

    /// Update time for animations
    pub fn update(&mut self, dt: f32) {
        self.current_time += dt;
    }

    /// Render sky objects
    pub fn render(
        &mut self,
        encoder: &mut CommandEncoder,
        color_view: &TextureView,
        camera_view_proj: [[f32; 4]; 4],
        camera_pos: Vec3,
        queue: &Queue,
    ) {
        // Update uniforms and GPU buffers before deciding to draw
        self.write_uniforms(queue, camera_view_proj, camera_pos);
        self.update_object_buffer(queue);

        // Skip if no objects
        if self.object_count == 0 {
            return;
        }

        // Create render pass (no depth attachment)
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Sky Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load, // Don't clear - sky gradient already drawn
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);

        // Draw 6 vertices per object (2 triangles for billboard quad)
        render_pass.draw(0..6, 0..self.object_count);
    }

    /// Update object buffer with current settings (colors, sizes, etc.)
    fn update_object_buffer(&mut self, queue: &Queue) {
        if !self.objects_dirty {
            return;
        }

        if !self.object_cache.is_empty() {
            queue.write_buffer(
                &self.object_buffer,
                0,
                bytemuck::cast_slice(&self.object_cache),
            );
        }
        self.objects_dirty = false;
    }

    fn write_uniforms(
        &self,
        queue: &Queue,
        camera_view_proj: [[f32; 4]; 4],
        camera_pos: Vec3,
    ) {
        let uniforms = SkyUniforms {
            view_proj: camera_view_proj,
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            time: self.current_time,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Generate a sky object of the specified type at the given seed
    fn generate_sky_object(&self, seed: u32, obj_type: SkyObjectType) -> SkyObject {
        let config = SkyObjectConfig::for_type(obj_type);

        // Calculate spherical coordinates
        let theta = Self::hash(seed) * std::f32::consts::TAU;
        let phi = config.phi_min + Self::hash(seed.wrapping_add(1)) * config.phi_range;

        // Convert to cartesian coordinates
        let x = phi.cos() * theta.cos();
        let y = phi.sin();
        let z = phi.cos() * theta.sin();

        // Apply minimum Y and normalize to sphere radius
        let pos = Vec3::new(x, y.max(config.y_min), z).normalize() * config.sphere_radius;

        // Get type-specific properties
        let (size, color, object_type_id, twinkle_seed, parallax) = match obj_type {
            SkyObjectType::Star => (
                self.settings.star_size_min +
                    Self::hash(seed.wrapping_add(2)) * (self.settings.star_size_max - self.settings.star_size_min),
                self.settings.star_color,
                0,
                Self::hash(seed.wrapping_add(3)) * 100.0, // Stars twinkle
                self.settings.star_parallax,
            ),
            SkyObjectType::Sun => (
                self.settings.sun_size,
                self.settings.sun_color,
                1,
                0.0, // Suns don't twinkle
                self.settings.sun_parallax,
            ),
            SkyObjectType::Moon => (
                self.settings.moon_size,
                self.settings.moon_color,
                2,
                0.0, // Moons don't twinkle
                self.settings.moon_parallax,
            ),
        };

        SkyObject {
            position: [pos.x, pos.y, pos.z],
            size,
            color,
            object_type: object_type_id,
            seed: twinkle_seed,
            parallax_factor: parallax,
            _padding: [0.0, 0.0],
        }
    }

    /// Generate a star object at the given seed
    fn generate_star(&self, seed: u32) -> SkyObject {
        self.generate_sky_object(seed, SkyObjectType::Star)
    }

    /// Generate a sun object at the given seed
    fn generate_sun(&self, seed: u32) -> SkyObject {
        self.generate_sky_object(seed, SkyObjectType::Sun)
    }

    /// Generate a moon object at the given seed
    fn generate_moon(&self, seed: u32) -> SkyObject {
        self.generate_sky_object(seed, SkyObjectType::Moon)
    }
}
