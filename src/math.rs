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

pub fn eye_offset(eye: usize, ipd_meters: f32) -> xr::Posef {
    let lateral = if eye == 0 {
        -ipd_meters * 0.5
    } else {
        ipd_meters * 0.5
    };
    xr::Posef {
        orientation: xr::Quaternionf::IDENTITY,
        position: xr::Vector3f {
            x: lateral,
            y: 0.0,
            z: 0.0,
        },
    }
}

pub fn pose_compose(a: xr::Posef, b: xr::Posef) -> xr::Posef {
    let rotated = quat_rotate(a.orientation, b.position);
    xr::Posef {
        orientation: quat_normalize(quat_mul(a.orientation, b.orientation)),
        position: xr::Vector3f {
            x: a.position.x + rotated.x,
            y: a.position.y + rotated.y,
            z: a.position.z + rotated.z,
        },
    }
}

pub fn pose_inverse(pose: xr::Posef) -> xr::Posef {
    let orientation = xr::Quaternionf {
        x: -pose.orientation.x,
        y: -pose.orientation.y,
        z: -pose.orientation.z,
        w: pose.orientation.w,
    };
    let position = xr::Vector3f {
        x: -pose.position.x,
        y: -pose.position.y,
        z: -pose.position.z,
    };
    xr::Posef {
        orientation,
        position: quat_rotate(orientation, position),
    }
}

pub fn pose_relative(pose: xr::Posef, base: xr::Posef) -> xr::Posef {
    pose_compose(pose_inverse(base), pose)
}

fn quat_mul(a: xr::Quaternionf, b: xr::Quaternionf) -> xr::Quaternionf {
    xr::Quaternionf {
        x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    }
}

fn quat_rotate(q: xr::Quaternionf, v: xr::Vector3f) -> xr::Vector3f {
    let tx = 2.0 * (q.y * v.z - q.z * v.y);
    let ty = 2.0 * (q.z * v.x - q.x * v.z);
    let tz = 2.0 * (q.x * v.y - q.y * v.x);
    xr::Vector3f {
        x: v.x + q.w * tx + q.y * tz - q.z * ty,
        y: v.y + q.w * ty + q.z * tx - q.x * tz,
        z: v.z + q.w * tz + q.x * ty - q.y * tx,
    }
}

fn quat_normalize(q: xr::Quaternionf) -> xr::Quaternionf {
    let length = (q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w).sqrt();
    if length <= f32::EPSILON {
        return xr::Quaternionf::IDENTITY;
    }
    xr::Quaternionf {
        x: q.x / length,
        y: q.y / length,
        z: q.z / length,
        w: q.w / length,
    }
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
        let head = head_pose(-std::f32::consts::FRAC_PI_2, 0.0, Vec3::new(0.0, 1.7, 0.0));
        let left = pose_compose(head, eye_offset(0, 0.06));
        let right = pose_compose(head, eye_offset(1, 0.06));
        assert!(left.position.z < right.position.z);
    }

    #[test]
    fn inverse_composition_restores_the_original_pose() {
        let head = head_pose(0.6, -0.2, Vec3::new(0.4, 1.7, -0.3));
        let eye = pose_compose(head, eye_offset(1, 0.064));
        let relative = pose_relative(eye, head);
        assert!((relative.position.x - 0.032).abs() < 1e-5);
        assert!(relative.position.y.abs() < 1e-5);
        assert!(relative.position.z.abs() < 1e-5);
        assert!((relative.orientation.w - 1.0).abs() < 1e-5);
    }
}
