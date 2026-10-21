use openxr_sys as xr;

#[derive(Debug, Default, Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn to_openxr(self) -> xr::Vector3f {
        xr::Vector3f {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

pub fn quat_yaw_pitch(yaw: f32, pitch: f32) -> xr::Quaternionf {
    let (sy, cy) = (yaw * 0.5).sin_cos();
    let (sp, cp) = (pitch * 0.5).sin_cos();
    xr::Quaternionf {
        x: sp * cy,
        y: cp * sy,
        z: -sp * sy,
        w: cp * cy,
    }
}

pub fn rotate_xz(yaw: f32, vector: Vec3) -> Vec3 {
    let (sin, cos) = yaw.sin_cos();
    Vec3::new(
        cos * vector.x + sin * vector.z,
        vector.y,
        -sin * vector.x + cos * vector.z,
    )
}

pub fn head_pose(yaw: f32, pitch: f32, position: Vec3) -> xr::Posef {
    xr::Posef {
        orientation: quat_yaw_pitch(yaw, pitch),
        position: position.to_openxr(),
    }
}

pub fn eye_pose(eye: usize, yaw: f32, pitch: f32, head: Vec3, ipd_meters: f32) -> xr::Posef {
    let lateral = if eye == 0 {
        -ipd_meters * 0.5
    } else {
        ipd_meters * 0.5
    };
    let offset = rotate_xz(yaw, Vec3::new(lateral, 0.0, 0.0));
    head_pose(
        yaw,
        pitch,
        Vec3::new(head.x + offset.x, head.y, head.z + offset.z),
    )
}

pub fn symmetric_fov(horizontal_degrees: f32, vertical_degrees: f32) -> xr::Fovf {
    let half_h = horizontal_degrees.to_radians() * 0.5;
    let half_v = vertical_degrees.to_radians() * 0.5;
    xr::Fovf {
        angle_left: -half_h,
        angle_right: half_h,
        angle_up: half_v,
        angle_down: -half_v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_quaternion_at_zero() {
        let q = quat_yaw_pitch(0.0, 0.0);
        assert!((q.w - 1.0).abs() < 1e-6);
    }

    #[test]
    fn eye_offsets_follow_yaw() {
        let left = eye_pose(
            0,
            -std::f32::consts::FRAC_PI_2,
            0.0,
            Vec3::new(0.0, 1.7, 0.0),
            0.06,
        );
        let right = eye_pose(
            1,
            -std::f32::consts::FRAC_PI_2,
            0.0,
            Vec3::new(0.0, 1.7, 0.0),
            0.06,
        );
        assert!(left.position.z < right.position.z);
    }
}
