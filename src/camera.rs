use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

use crate::input::InputState;

/// Maximum pitch angle in radians (~86 degrees) to prevent camera flipping
const PITCH_LIMIT: f32 = 1.5;

/// Keyboard rotation speed in radians per second
const ROTATION_SPEED: f32 = 0.8;

/// Scroll wheel zoom speed multiplier
const ZOOM_SPEED: f32 = 10.0;

/// Fly camera for exploring the terrain
pub struct FlyCamera {
    pub position: Vec3,
    pub yaw: f32,   // Horizontal rotation (radians)
    pub pitch: f32, // Vertical rotation (radians)

    pub aspect: f32,
    pub fov: f32,
    pub near: f32,
    pub far: f32,

    pub move_speed: f32,
    pub look_sensitivity: f32,
}

impl FlyCamera {
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 100.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,

            aspect,
            fov: 70.0_f32.to_radians(),
            near: 0.1,
            far: 5000.0,

            move_speed: 300.0,
            look_sensitivity: 0.002,
        }
    }

    /// Forward vector derived from yaw/pitch (not normalized)
    fn forward_vector(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// Normalized forward direction
    fn forward_direction(&self) -> Vec3 {
        self.forward_vector().normalize()
    }

    pub fn update(&mut self, input: &InputState, dt: f32) {
        // Mouse look (only when locked)
        if input.mouse_locked {
            self.yaw -= input.mouse_delta_x * self.look_sensitivity;
            self.pitch -= input.mouse_delta_y * self.look_sensitivity;
            // Clamp pitch to prevent flipping
            self.pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
        }

        // Keyboard rotation (Q/E or U/O)
        if input.is_key_down("q") || input.is_key_down("u") {
            self.yaw += ROTATION_SPEED * dt;
        }
        if input.is_key_down("e") || input.is_key_down("o") {
            self.yaw -= ROTATION_SPEED * dt;
        }

        // Calculate movement vectors
        let forward = self.forward_direction();

        let right = Vec3::new(-self.yaw.cos(), 0.0, self.yaw.sin()).normalize();
        let up = Vec3::Y;

        // Scroll zoom (move along forward direction)
        if input.scroll_delta.abs() > 0.001 {
            let zoom_amount = -input.scroll_delta * ZOOM_SPEED;
            self.position += forward * zoom_amount;
        }

        // Movement input (WASD or IJKL)
        let mut velocity = Vec3::ZERO;

        if input.is_key_down("w") || input.is_key_down("i") {
            velocity += forward;
        }
        if input.is_key_down("s") || input.is_key_down("k") {
            velocity -= forward;
        }
        if input.is_key_down("a") || input.is_key_down("j") {
            velocity -= right;
        }
        if input.is_key_down("d") || input.is_key_down("l") {
            velocity += right;
        }
        if input.is_key_down(" ") {
            // Space - up
            velocity += up;
        }
        if input.is_key_down("shift") {
            // Shift - down
            velocity -= up;
        }

        // Apply movement
        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * self.move_speed * dt;
            self.position += velocity;
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        let forward = self.forward_direction();
        let target = self.position + forward;
        Mat4::look_at_rh(self.position, target, Vec3::Y)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    pub fn uniform_data(&self) -> CameraUniform {
        CameraUniform {
            view_proj: self.view_projection_matrix().to_cols_array_2d(),
            camera_pos: self.position.to_array(),
            _padding: 0.0,
        }
    }

    /// Extract frustum planes from the view-projection matrix
    /// Returns 6 planes: [left, right, bottom, top, near, far]
    /// Each plane is (nx, ny, nz, d) where nx*x + ny*y + nz*z + d >= 0 means inside
    pub fn extract_frustum_planes(&self) -> [Vec4; 6] {
        let vp = self.view_projection_matrix();
        let cols = vp.to_cols_array_2d();

        // Extract rows from the transposed matrix for plane extraction
        let row0 = Vec4::new(cols[0][0], cols[1][0], cols[2][0], cols[3][0]);
        let row1 = Vec4::new(cols[0][1], cols[1][1], cols[2][1], cols[3][1]);
        let row2 = Vec4::new(cols[0][2], cols[1][2], cols[2][2], cols[3][2]);
        let row3 = Vec4::new(cols[0][3], cols[1][3], cols[2][3], cols[3][3]);

        // Extract and normalize planes
        let mut planes = [
            row3 + row0, // Left
            row3 - row0, // Right
            row3 + row1, // Bottom
            row3 - row1, // Top
            row3 + row2, // Near
            row3 - row2, // Far
        ];

        // Normalize each plane
        for plane in &mut planes {
            let len = (plane.x * plane.x + plane.y * plane.y + plane.z * plane.z).sqrt();
            if len > 0.0 {
                *plane /= len;
            }
        }

        planes
    }
}

/// Camera uniform data for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
}
