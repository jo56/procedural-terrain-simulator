// ============================================
// CONSTANTS
// ============================================

const CHUNK_SIZE: u32 = 64u;  // Vertices per chunk edge
const CHUNK_WORLD_SIZE: f32 = 256.0;  // World units per chunk

// ============================================
// NOISE FUNCTIONS (Simplex 2D)
// Based on: https://gist.github.com/munrocket/236ed5ba7e409b8bdf1ff6eca5dcdc39
// ============================================

fn mod289_2(x: vec2f) -> vec2f {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn mod289_3(x: vec3f) -> vec3f {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn permute3(x: vec3f) -> vec3f {
    return mod289_3(((x * 34.0) + 1.0) * x);
}

fn simplex_noise_2d(v: vec2f) -> f32 {
    let C = vec4f(
        0.211324865405187,   // (3.0 - sqrt(3.0)) / 6.0
        0.366025403784439,   // 0.5 * (sqrt(3.0) - 1.0)
        -0.577350269189626,  // -1.0 + 2.0 * C.x
        0.024390243902439    // 1.0 / 41.0
    );

    var i = floor(v + dot(v, C.yy));
    let x0 = v - i + dot(i, C.xx);
    var i1 = select(vec2f(0.0, 1.0), vec2f(1.0, 0.0), x0.x > x0.y);
    var x12 = x0.xyxy + C.xxzz;
    x12 = vec4f(x12.x - i1.x, x12.y - i1.y, x12.z, x12.w);

    i = mod289_2(i);
    var p = permute3(permute3(i.y + vec3f(0.0, i1.y, 1.0)) + i.x + vec3f(0.0, i1.x, 1.0));
    var m = max(0.5 - vec3f(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), vec3f(0.0));
    m *= m;
    m *= m;

    let x = 2.0 * fract(p * C.www) - 1.0;
    let h = abs(x) - 0.5;
    let ox = floor(x + 0.5);
    let a0 = x - ox;

    m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
    let g = vec3f(a0.x * x0.x + h.x * x0.y, a0.yz * x12.xz + h.yz * x12.yw);
    return 130.0 * dot(m, g);
}

// Fractal Brownian Motion for more natural terrain
fn fbm(p: vec2f, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * simplex_noise_2d(pos * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    return value;
}

// Domain warping for more interesting terrain
fn domain_warp(p: vec2f, warp_strength: f32) -> vec2f {
    let offset = vec2f(
        fbm(p + vec2f(0.0, 0.0), 3),
        fbm(p + vec2f(5.2, 1.3), 3)
    );
    return p + offset * warp_strength;
}

// ============================================
// COMPUTE SHADER - HEIGHT GENERATION
// ============================================

struct ComputeParams {
    chunk_offset: vec2f,   // World position of chunk origin
    terrain_scale: f32,    // Horizontal scale for noise
    height_scale: f32,     // Vertical scale for output
    octaves: u32,          // FBM octaves
    warp_strength: f32,    // Domain warping strength
    height_variance: f32,  // Height variation multiplier
    roughness: f32,        // FBM persistence
    pattern_type: u32,     // 0=standard, 1=ridged, 2=islands, 3=valleys, 4=terraced
    seed: u32,             // Random seed for terrain variation
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0) var<uniform> compute_params: ComputeParams;
@group(0) @binding(1) var<storage, read_write> height_buffer: array<f32>;

// FBM with configurable roughness
fn fbm_rough(p: vec2f, octaves: i32, roughness: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * simplex_noise_2d(pos * frequency);
        amplitude *= roughness;
        frequency *= 2.0;
    }
    return value;
}

// Island falloff - creates ocean around terrain
fn island_falloff(world_pos: vec2f, center: vec2f, radius: f32) -> f32 {
    let dist = length(world_pos - center) / radius;
    // Smooth falloff that creates islands
    let falloff = 1.0 - smoothstep(0.3, 1.0, dist);
    return falloff;
}

@compute @workgroup_size(8, 8)
fn compute_height(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= CHUNK_SIZE || id.y >= CHUNK_SIZE) {
        return;
    }

    let index = id.y * CHUNK_SIZE + id.x;

    // Calculate world position of this vertex
    let local_uv = vec2f(f32(id.x), f32(id.y)) / f32(CHUNK_SIZE - 1u);
    let world_pos = compute_params.chunk_offset + local_uv * CHUNK_WORLD_SIZE;

    // Apply seed offset to create different terrain for each seed
    // Using small multipliers to stay in a similar noise region (preserves terrain style)
    let seed_offset = vec2f(f32(compute_params.seed) * 0.1, f32(compute_params.seed) * 0.137);

    // Apply noise at multiple scales
    let noise_pos = (world_pos + seed_offset) * compute_params.terrain_scale;

    // Use octaves from params (cast to i32 for fbm)
    let octaves = i32(compute_params.octaves);
    let roughness = compute_params.roughness;
    let variance = compute_params.height_variance;

    var height = 0.0;

    // Pattern type selection
    switch compute_params.pattern_type {
        case 0u: {
            // Standard terrain with domain warping
            let warped_pos = domain_warp(noise_pos * 0.5, compute_params.warp_strength);
            height = fbm_rough(warped_pos * 0.3, octaves, roughness) * compute_params.height_scale;

            // Add smaller detail
            let detail_octaves = max(octaves - 2, 2);
            height += fbm_rough(noise_pos * 2.0, detail_octaves, roughness) * compute_params.height_scale * 0.15 * variance;

            // Add ridges
            let ridge = 1.0 - abs(fbm_rough(noise_pos * 0.8, detail_octaves, roughness));
            height += ridge * ridge * compute_params.height_scale * 0.3 * variance;
        }
        case 1u: {
            // Ridged/mountainous - dramatic sharp mountain ranges
            let warped_pos = domain_warp(noise_pos * 0.5, compute_params.warp_strength * 1.5);

            // Lower, flatter base terrain (emphasizes the ridges more)
            height = fbm_rough(warped_pos * 0.2, octaves, roughness) * compute_params.height_scale * 0.3;

            // Sharp primary ridges - higher frequency, stronger effect
            let ridge1 = 1.0 - abs(fbm_rough(warped_pos * 0.6, octaves, roughness));
            let ridge2 = 1.0 - abs(fbm_rough(warped_pos * 1.0, octaves - 1, roughness));

            // Stronger ridge contributions with sharper peaks (higher power)
            height += pow(ridge1, 2.5) * compute_params.height_scale * 0.6 * variance;
            height += pow(ridge2, 2.0) * compute_params.height_scale * 0.3 * variance;

            // Fine erosion detail
            height += fbm_rough(noise_pos * 3.0, 4, roughness) * compute_params.height_scale * 0.08;
        }
        case 2u: {
            // Islands/archipelago
            let warped_pos = domain_warp(noise_pos * 0.5, compute_params.warp_strength);
            let base_height = fbm_rough(warped_pos * 0.3, octaves, roughness);

            // Create island mask using noise
            let island_noise = fbm_rough(noise_pos * 0.15, 4, 0.5);
            let island_mask = smoothstep(-0.1, 0.3, island_noise);

            // Apply mask - creates ocean at low values
            height = (base_height * island_mask - 0.3) * compute_params.height_scale;
            height += fbm_rough(noise_pos * 2.0, 4, roughness) * compute_params.height_scale * 0.1 * variance * island_mask;
        }
        case 3u: {
            // Valleys/canyons - inverted ridges
            let warped_pos = domain_warp(noise_pos * 0.5, compute_params.warp_strength);

            // Base terrain
            height = fbm_rough(warped_pos * 0.3, octaves, roughness) * compute_params.height_scale * 0.5;

            // Carve valleys (inverted ridges)
            let valley1 = abs(fbm_rough(warped_pos * 0.5, octaves - 1, roughness));
            let valley2 = abs(fbm_rough(warped_pos * 0.8 + vec2f(100.0, 100.0), octaves - 2, roughness));

            height -= pow(1.0 - valley1, 3.0) * compute_params.height_scale * 0.6 * variance;
            height -= pow(1.0 - valley2, 2.0) * compute_params.height_scale * 0.3 * variance;

            // Add some texture
            height += fbm_rough(noise_pos * 3.0, 3, roughness) * compute_params.height_scale * 0.05;
        }
        case 4u: {
            // Terraced/plateaus
            let warped_pos = domain_warp(noise_pos * 0.5, compute_params.warp_strength);
            let raw_height = fbm_rough(warped_pos * 0.3, octaves, roughness) * compute_params.height_scale;

            // Quantize height to create terraces
            let terrace_count = 6.0 + variance * 6.0;
            let step_size = compute_params.height_scale / terrace_count;
            height = floor(raw_height / step_size + 0.5) * step_size;

            // Add slight variation within terraces
            height += fbm_rough(noise_pos * 4.0, 3, roughness) * step_size * 0.2;
        }
        case 5u: {
            // Blocky/plateau terrain - uses distant noise region for unique character
            let blocky_offset = vec2f(1000000.0, 1337000.0);
            let blocky_pos = noise_pos + blocky_offset;
            let warped_pos = domain_warp(blocky_pos * 0.5, compute_params.warp_strength);
            height = fbm_rough(warped_pos * 0.3, octaves, roughness) * compute_params.height_scale;

            // Add detail with the offset
            let detail_octaves = max(octaves - 2, 2);
            height += fbm_rough(blocky_pos * 2.0, detail_octaves, roughness) * compute_params.height_scale * 0.15 * variance;

            // Add characteristic blocky ridges
            let ridge = 1.0 - abs(fbm_rough(blocky_pos * 0.8, detail_octaves, roughness));
            height += ridge * ridge * compute_params.height_scale * 0.3 * variance;
        }
        default: {
            // Fallback to standard
            let warped_pos = domain_warp(noise_pos * 0.5, compute_params.warp_strength);
            height = fbm_rough(warped_pos * 0.3, octaves, roughness) * compute_params.height_scale;
        }
    }

    height_buffer[index] = height;
}

// ============================================
// VERTEX SHADER - TERRAIN RENDERING
// ============================================

struct CameraUniforms {
    view_proj: mat4x4f,
    camera_pos: vec3f,
    _padding: f32,
}

struct ChunkUniforms {
    chunk_offset: vec2f,
    _padding: vec2f,
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(1) @binding(0) var<uniform> chunk: ChunkUniforms;
@group(1) @binding(1) var<storage, read> heights: array<f32>;

struct VertexInput {
    @location(0) local_uv: vec2f,  // 0..1 range within chunk
    @builtin(vertex_index) vertex_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) world_pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) height: f32,
}

fn get_height(x: i32, y: i32) -> f32 {
    let cx = clamp(x, 0, i32(CHUNK_SIZE) - 1);
    let cy = clamp(y, 0, i32(CHUNK_SIZE) - 1);
    return heights[u32(cy) * CHUNK_SIZE + u32(cx)];
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Grid indices from UV
    let grid_x = i32(in.local_uv.x * f32(CHUNK_SIZE - 1u) + 0.5);
    let grid_y = i32(in.local_uv.y * f32(CHUNK_SIZE - 1u) + 0.5);

    // Get height for this vertex
    let height = get_height(grid_x, grid_y);

    // World position
    let world_xz = chunk.chunk_offset + in.local_uv * CHUNK_WORLD_SIZE;
    out.world_pos = vec3f(world_xz.x, height, world_xz.y);

    // Compute normal from neighboring heights
    let step = CHUNK_WORLD_SIZE / f32(CHUNK_SIZE - 1u);
    let h_left = get_height(grid_x - 1, grid_y);
    let h_right = get_height(grid_x + 1, grid_y);
    let h_down = get_height(grid_x, grid_y - 1);
    let h_up = get_height(grid_x, grid_y + 1);

    let dx = (h_right - h_left) / (2.0 * step);
    let dz = (h_up - h_down) / (2.0 * step);
    out.normal = normalize(vec3f(-dx, 1.0, -dz));

    out.height = height;
    out.clip_position = camera.view_proj * vec4f(out.world_pos, 1.0);

    return out;
}

// ============================================
// FRAGMENT SHADER - TERRAIN COLORING
// ============================================

struct ColorParams {
    color_abyss: vec4f,
    color_deep_water: vec4f,
    color_shallow_water: vec4f,
    color_sand: vec4f,
    color_grass: vec4f,
    color_rock: vec4f,
    color_snow: vec4f,
    color_sky: vec4f,
    color_sky_top: vec4f,
    color_sky_horizon: vec4f,
    ambient: f32,
    fog_start: f32,
    fog_distance: f32,
    _padding: f32,
}

@group(2) @binding(0) var<uniform> colors: ColorParams;

const SUN_DIR: vec3f = vec3f(0.4, 0.7, 0.5);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let normal = normalize(in.normal);
    let sun_dir = normalize(SUN_DIR);

    // Calculate slope (0 = flat, 1 = vertical)
    let slope = 1.0 - normal.y;

    // Color palette from uniforms
    let abyss = colors.color_abyss.rgb;
    let deep_water = colors.color_deep_water.rgb;
    let shallow_water = colors.color_shallow_water.rgb;
    let sand = colors.color_sand.rgb;
    let grass = colors.color_grass.rgb;
    let dark_grass = grass * 0.7; // Derive from grass
    let rock = colors.color_rock.rgb;
    let dark_rock = rock * 0.7; // Derive from rock
    let snow = colors.color_snow.rgb;

    // Height-based terrain coloring
    var base_color: vec3f;
    let h = in.height;

    if (h < -100.0) {
        // Abyss - deepest water
        base_color = abyss;
    } else if (h < -5.0) {
        // Deep water - blend from abyss to deep water
        let t = (h + 100.0) / 95.0;
        base_color = mix(abyss, deep_water, t);
    } else if (h < 0.0) {
        // Shallow water - blend from deep to shallow
        let t = (h + 5.0) / 5.0;
        base_color = mix(deep_water, shallow_water, t);
    } else if (h < 5.0) {
        // Beach/sand
        let t = h / 5.0;
        base_color = mix(sand, grass, t);
    } else if (h < 40.0) {
        // Grassland
        let t = (h - 5.0) / 35.0;
        base_color = mix(grass, dark_grass, t);
    } else if (h < 70.0) {
        // Transition to rock
        let t = (h - 40.0) / 30.0;
        base_color = mix(dark_grass, rock, t);
    } else if (h < 100.0) {
        // Rocky mountains
        let t = (h - 70.0) / 30.0;
        base_color = mix(rock, dark_rock, t);
    } else {
        // Snow caps
        let t = clamp((h - 100.0) / 20.0, 0.0, 1.0);
        base_color = mix(dark_rock, snow, t);
    }

    // Slope-based rock blending
    if (h > 5.0) {  // Above water/beach
        let rock_blend = smoothstep(0.35, 0.55, slope);
        base_color = mix(base_color, rock, rock_blend);
    }

    // Snow on high slopes is less likely
    if (h > 90.0 && slope < 0.35) {
        let snow_factor = (h - 90.0) / 30.0 * (1.0 - slope / 0.35);
        base_color = mix(base_color, snow, clamp(snow_factor, 0.0, 1.0));
    }

    // Lambert diffuse lighting
    let ndotl = max(dot(normal, sun_dir), 0.0);

    // Add some wrap lighting for softer shadows
    let wrap_light = (ndotl + 0.3) / 1.3;

    // Final lighting
    let lighting = colors.ambient + (1.0 - colors.ambient) * wrap_light;

    // Apply fog based on distance from camera, attenuated by camera height
    let dist = length(in.world_pos - camera.camera_pos);
    let camera_height = camera.camera_pos.y;
    let height_factor = 1.0 - clamp((camera_height - 500.0) / 300.0, 0.0, 1.0);
    let fog_factor = clamp((dist - colors.fog_start) / colors.fog_distance, 0.0, 0.8) * height_factor;

    // Compute sky gradient based on view direction
    let view_dir = normalize(in.world_pos - camera.camera_pos);
    let sky_blend = smoothstep(-0.1, 0.5, view_dir.y);
    let fog_color = mix(colors.color_sky_horizon.rgb, colors.color_sky_top.rgb, sky_blend);

    var final_color = base_color * lighting;
    final_color = mix(final_color, fog_color, fog_factor);

    return vec4f(final_color, 1.0);
}
