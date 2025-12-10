// Particle compute and render shaders

// Particle structure - matches Rust Particle struct
struct Particle {
    position: vec3f,
    velocity: vec3f,
    life: f32,
    size: f32,
}

// Simulation parameters - must match Rust SimParams struct layout
struct SimParams {
    delta_time: f32,
    time: f32,
    camera_pos: vec3f,
    wind_x: f32,
    wind_z: f32,
    spawn_height: f32,
    spawn_radius: f32,
    despawn_height: f32,
    particle_type: u32,
    speed: f32,
    particle_count: u32,
    _padding: f32,
}

// Bind group layout must match Rust
@group(0) @binding(0) var<uniform> sim: SimParams;
@group(0) @binding(1) var<storage, read> particles_in: array<Particle>;
@group(0) @binding(2) var<storage, read_write> particles_out: array<Particle>;

// Simple hash function for randomness
fn hash(p: vec3f) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash2(p: vec2f) -> vec2f {
    let k = vec2f(0.3183099, 0.3678794);
    var p2 = p * k + k.yx;
    return fract(16.0 * k * fract(p2.x * p2.y * (p2.x + p2.y)));
}

// Compute shader - simulate particles
@compute @workgroup_size(256)
fn simulate(@builtin(global_invocation_id) global_id: vec3u) {
    let idx = global_id.x;
    if (idx >= sim.particle_count) {
        return;
    }

    var p = particles_in[idx];
    let camera_pos = sim.camera_pos;

    // Update position
    p.position += p.velocity * sim.delta_time;

    // Decrease life
    p.life -= sim.delta_time;

    // Check if particle needs respawning
    let horizontal_dist = length(p.position.xz - camera_pos.xz);
    // NaN check: NaN != NaN is true, so this detects corrupted particles
    let has_nan = p.position.x != p.position.x ||
                  p.position.y != p.position.y ||
                  p.position.z != p.position.z ||
                  p.velocity.x != p.velocity.x ||
                  p.velocity.y != p.velocity.y ||
                  p.velocity.z != p.velocity.z;
    let needs_respawn = p.life <= 0.0 ||
                        p.position.y < sim.despawn_height ||
                        horizontal_dist > sim.spawn_radius * 1.5 ||
                        has_nan;

    if (needs_respawn) {
        // Respawn particle near camera
        // Use stable time value to prevent hash clustering with large time values
        let stable_time = sim.time - floor(sim.time / 1000.0) * 1000.0;
        let seed = vec3f(f32(idx), stable_time, f32(idx) * 0.7);
        let rand1 = hash(seed);
        let rand2 = hash(seed + vec3f(1.0, 2.0, 3.0));
        let rand3 = hash(seed + vec3f(4.0, 5.0, 6.0));

        // Random angle and distance
        let angle = rand1 * 6.28318;
        let dist = sqrt(rand2) * sim.spawn_radius;

        p.position.x = camera_pos.x + cos(angle) * dist;
        p.position.z = camera_pos.z + sin(angle) * dist;
        // Spread particles throughout the spawn height range (not just at top)
        p.position.y = camera_pos.y + sim.spawn_height * rand3;

        // Reset velocity based on type
        if (sim.particle_type == 0u) {
            // Rain - fast, mostly vertical
            p.velocity = vec3f(
                sim.wind_x * 0.5,
                -sim.speed,
                sim.wind_z * 0.5
            );
        } else {
            // Snow - slow, drifting
            let drift = hash2(vec2f(f32(idx) + sim.time, rand1));
            p.velocity = vec3f(
                sim.wind_x * 0.3 + (drift.x - 0.5) * 2.0,
                -sim.speed * 0.3,
                sim.wind_z * 0.3 + (drift.y - 0.5) * 2.0
            );
        }

        // Random lifetime
        p.life = 3.0 + rand3 * 5.0;
    } else {
        // Apply simple physics - constant fall with mild wind
        if (sim.particle_type == 0u) {
            // Rain - mild wind influence
            p.velocity.x += sim.wind_x * sim.delta_time * 0.1;
            p.velocity.z += sim.wind_z * sim.delta_time * 0.1;
        } else {
            // Snow - gentle wind influence only (no erratic drift)
            p.velocity.x += sim.wind_x * sim.delta_time * 0.05;
            p.velocity.z += sim.wind_z * sim.delta_time * 0.05;

            // Light damping
            p.velocity.x *= 0.995;
            p.velocity.z *= 0.995;
        }
    }

    particles_out[idx] = p;
}

// ============================================
// Render shaders
// ============================================

// Render params - must match Rust RenderParams struct layout
struct RenderParams {
    view_proj: mat4x4f,
    camera_pos: vec3f,
    particle_size: f32,
    particle_color: vec4f,
    particle_type: u32,
    time: f32,
    _padding: vec2f,
}

@group(0) @binding(0) var<uniform> render: RenderParams;
@group(0) @binding(1) var<storage, read> render_particles: array<Particle>;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
    @location(1) alpha: f32,
}

// Billboard quad vertices
const QUAD_VERTICES = array<vec2f, 6>(
    vec2f(-0.5, -0.5),
    vec2f( 0.5, -0.5),
    vec2f( 0.5,  0.5),
    vec2f(-0.5, -0.5),
    vec2f( 0.5,  0.5),
    vec2f(-0.5,  0.5),
);

@vertex
fn vs_particle(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32
) -> VertexOutput {
    let p = render_particles[instance_idx];
    let quad_pos = QUAD_VERTICES[vertex_idx];

    // Base size - much smaller particles
    let base_size = render.particle_size * p.size * 0.3;

    // Billboard - face camera (with degenerate case handling)
    let to_camera = normalize(render.camera_pos - p.position);
    let world_up = vec3f(0.0, 1.0, 0.0);
    var right: vec3f;
    // When to_camera is nearly vertical, cross(world_up, to_camera) produces near-zero
    // vector, and normalize() produces NaN. Use Z axis as fallback reference.
    if (abs(dot(to_camera, world_up)) > 0.99) {
        right = normalize(cross(vec3f(0.0, 0.0, 1.0), to_camera));
    } else {
        right = normalize(cross(world_up, to_camera));
    }
    let up = cross(to_camera, right);

    var world_offset: vec3f;
    if (render.particle_type == 0u) {
        // Rain - thin vertical line
        world_offset = right * quad_pos.x * base_size * 0.08 + up * quad_pos.y * base_size * 1.5;
    } else {
        // Snow - small dot
        world_offset = right * quad_pos.x * base_size * 0.4 + up * quad_pos.y * base_size * 0.4;
    }

    let world_pos = p.position + world_offset;

    var out: VertexOutput;
    out.position = render.view_proj * vec4f(world_pos, 1.0);
    out.uv = quad_pos + 0.5;

    // Fade based on life
    let life_fade = smoothstep(0.0, 0.5, p.life);

    // Distance fade - generous range so particles near camera are visible
    let dist = length(p.position - render.camera_pos);
    let dist_fade = 1.0 - smoothstep(200.0, 400.0, dist);

    out.alpha = life_fade * dist_fade;

    return out;
}

@fragment
fn fs_particle(in: VertexOutput) -> @location(0) vec4f {
    var color = render.particle_color;

    if (render.particle_type == 0u) {
        // Rain - thin vertical line
        let center_dist = abs(in.uv.x - 0.5) * 2.0;
        let alpha = (1.0 - center_dist * center_dist) * in.alpha * 0.7;
        return vec4f(color.rgb, alpha);
    } else {
        // Snow - simple small dot (no sparkle)
        let center = in.uv - 0.5;
        let dist = length(center) * 2.0;
        let circle = 1.0 - smoothstep(0.0, 0.9, dist);
        let alpha = circle * in.alpha * 0.85;
        return vec4f(color.rgb, alpha);
    }
}
