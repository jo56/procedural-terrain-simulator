// ============================================
// SKY OBJECTS SHADER
// Renders stars, suns, and moons as billboards
// ============================================

struct SkyUniforms {
    view_proj: mat4x4f,
    camera_pos: vec3f,
    time: f32,
}

struct SkyObject {
    position: vec3f,
    size: f32,
    color: vec3f,
    object_type: u32,   // 0=star, 1=sun, 2=moon
    seed: f32,
    parallax_factor: f32,
    _padding: vec2f,
}

@group(0) @binding(0) var<uniform> uniforms: SkyUniforms;
@group(0) @binding(1) var<storage, read> objects: array<SkyObject>;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec3f,
    @location(2) @interpolate(flat) object_type: u32,
    @location(3) seed: f32,
}

// Billboard quad vertices (2 triangles)
const QUAD_VERTS: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(-1.0, -1.0),
    vec2f(1.0, -1.0),
    vec2f(-1.0, 1.0),
    vec2f(-1.0, 1.0),
    vec2f(1.0, -1.0),
    vec2f(1.0, 1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_id: u32,
    @builtin(instance_index) instance_id: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let obj = objects[instance_id];
    let quad_vert = QUAD_VERTS[vertex_id];

    // Apply parallax offset based on camera position
    let parallax_offset = uniforms.camera_pos.xz * obj.parallax_factor * 0.001;
    var base_pos = obj.position + vec3f(parallax_offset.x, 0.0, parallax_offset.y);

    // Keep objects in upper hemisphere relative to camera
    base_pos.y = max(base_pos.y, uniforms.camera_pos.y + 5.0);

    // Calculate billboard orientation (camera-facing)
    let to_cam = normalize(uniforms.camera_pos - base_pos);
    let world_up = vec3f(0.0, 1.0, 0.0);
    let right = normalize(cross(world_up, to_cam));
    let up = cross(to_cam, right);

    // Scale and position the quad
    let offset = quad_vert * obj.size;
    let world_pos = base_pos + right * offset.x + up * offset.y;

    out.position = uniforms.view_proj * vec4f(world_pos, 1.0);
    out.uv = quad_vert * 0.5 + 0.5;
    out.color = obj.color;
    out.object_type = obj.object_type;
    out.seed = obj.seed;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let center = vec2f(0.5, 0.5);
    let dist = length(in.uv - center);

    switch in.object_type {
        case 0u: {
            // Star - point with twinkle
            let twinkle = sin(uniforms.time * 3.0 + in.seed) * 0.3 + 0.7;
            let alpha = smoothstep(0.5, 0.0, dist) * twinkle;

            // Add glow
            let glow = smoothstep(0.5, 0.1, dist) * 0.5;

            return vec4f(in.color * (1.0 + glow * 0.5), alpha);
        }
        case 1u: {
            // Sun - glowing disc with corona
            let core = smoothstep(0.3, 0.1, dist);
            let glow = smoothstep(0.5, 0.0, dist);

            // Corona effect
            let corona = smoothstep(0.5, 0.3, dist) * 0.5;

            let brightness = core + corona * 0.5;
            let alpha = glow;

            return vec4f(in.color * (0.8 + brightness * 0.4), alpha);
        }
        case 2u: {
            // Moon - solid disc with subtle gradient
            let alpha = smoothstep(0.5, 0.45, dist);

            // Slight shading gradient (left to right)
            let shade = 0.85 + 0.15 * (1.0 - in.uv.x);

            return vec4f(in.color * shade, alpha);
        }
        default: {
            return vec4f(0.0);
        }
    }
}
