//! Cámara de agujero (pinhole) que genera los rayos primarios.

use crate::math::{Ray, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: Vec3,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    tan_half: f32,
    aspect: f32,
}

impl Camera {
    pub fn look_at(position: Vec3, target: Vec3, vertical_fov_deg: f32, aspect: f32) -> Self {
        let forward = (target - position).normalized();
        let right = forward.cross(Vec3::UP).normalized();
        let up = right.cross(forward).normalized();
        Self {
            position,
            forward,
            right,
            up,
            tan_half: (vertical_fov_deg.to_radians() * 0.5).tan(),
            aspect,
        }
    }

    /// `sx`, `sy` en [0,1]; `sy = 0` es la parte superior de la imagen.
    #[inline(always)]
    pub fn ray(&self, sx: f32, sy: f32) -> Ray {
        let x = (2.0 * sx - 1.0) * self.tan_half * self.aspect;
        let y = (1.0 - 2.0 * sy) * self.tan_half;
        Ray::new(
            self.position,
            (self.forward + self.right * x + self.up * y).normalized(),
        )
    }

    /// Proyecta un punto a coordenadas de pantalla [0,1]² y devuelve también la profundidad.
    pub fn project(&self, p: Vec3) -> (f32, f32, f32) {
        let v = p - self.position;
        let depth = v.dot(self.forward);
        let x = v.dot(self.right) / (depth.max(1e-4) * self.tan_half * self.aspect);
        let y = v.dot(self.up) / (depth.max(1e-4) * self.tan_half);
        (x * 0.5 + 0.5, 0.5 - y * 0.5, depth)
    }

    /// Tamaño (en fracción de la altura de pantalla) de un objeto de `size` metros a `depth`.
    pub fn screen_size(&self, size: f32, depth: f32) -> f32 {
        size / (depth.max(1e-4) * self.tan_half * 2.0)
    }

    pub fn forward(&self) -> Vec3 {
        self.forward
    }

    pub fn same_as(&self, other: &Camera) -> bool {
        (self.position - other.position).length_sq() < 1e-10
            && (self.forward - other.forward).length_sq() < 1e-10
            && (self.aspect - other.aspect).abs() < 1e-6
    }
}
