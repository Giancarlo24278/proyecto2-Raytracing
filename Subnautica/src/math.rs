//! Vectores, matrices de rotación y utilidades numéricas (f32 para velocidad).

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0, 1.0);
    pub const UP: Self = Self::new(0.0, 1.0, 0.0);

    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v, v)
    }

    #[inline(always)]
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline(always)]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    #[inline(always)]
    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }

    #[inline(always)]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    #[inline(always)]
    pub fn normalized(self) -> Self {
        let l2 = self.dot(self);
        if l2 > 0.0 { self * (1.0 / l2.sqrt()) } else { self }
    }

    #[inline(always)]
    pub fn reflect(self, n: Self) -> Self {
        self - n * (2.0 * self.dot(n))
    }

    /// Refracción de Snell. `n` apunta hacia el lado de donde viene el rayo,
    /// `eta` = n_origen / n_destino. Devuelve `None` si hay reflexión interna total.
    #[inline(always)]
    pub fn refract(self, n: Self, eta: f32) -> Option<Self> {
        let cos_i = (-self.dot(n)).min(1.0);
        let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
        if k < 0.0 {
            None
        } else {
            Some((self * eta + n * (eta * cos_i - k.sqrt())).normalized())
        }
    }

    /// Mínimo por componente (sin el manejo especial de NaN de `f32::min`: más rápido).
    #[inline(always)]
    pub fn min(self, o: Self) -> Self {
        Self::new(fmin(self.x, o.x), fmin(self.y, o.y), fmin(self.z, o.z))
    }

    #[inline(always)]
    pub fn max(self, o: Self) -> Self {
        Self::new(fmax(self.x, o.x), fmax(self.y, o.y), fmax(self.z, o.z))
    }

    #[inline(always)]
    pub fn abs(self) -> Self {
        Self::new(self.x.abs(), self.y.abs(), self.z.abs())
    }

    #[inline(always)]
    pub fn max_elem(self) -> f32 {
        fmax(fmax(self.x, self.y), self.z)
    }

    #[inline(always)]
    pub fn min_elem(self) -> f32 {
        fmin(fmin(self.x, self.y), self.z)
    }

    #[inline(always)]
    pub fn luminance(self) -> f32 {
        self.x * 0.2126 + self.y * 0.7152 + self.z * 0.0722
    }

    #[inline(always)]
    pub fn lerp(self, o: Self, t: f32) -> Self {
        self + (o - self) * t
    }

    #[inline(always)]
    pub fn exp(self) -> Self {
        Self::new(self.x.exp(), self.y.exp(), self.z.exp())
    }

    #[inline(always)]
    pub fn get(self, axis: usize) -> f32 {
        match axis {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl AddAssign for Vec3 {
    #[inline(always)]
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}
impl Sub for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl SubAssign for Vec3 {
    #[inline(always)]
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}
impl Mul<f32> for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Mul for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}
impl MulAssign<f32> for Vec3 {
    #[inline(always)]
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl MulAssign for Vec3 {
    #[inline(always)]
    fn mul_assign(&mut self, o: Self) {
        *self = *self * o;
    }
}
impl Div<f32> for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn div(self, s: f32) -> Self {
        self * (1.0 / s)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// Matriz de rotación guardada por columnas: cada columna es un eje local en el mundo.
#[derive(Clone, Copy, Debug)]
pub struct Mat3 {
    pub c0: Vec3,
    pub c1: Vec3,
    pub c2: Vec3,
}

impl Mat3 {
    pub const IDENTITY: Self = Self {
        c0: Vec3::new(1.0, 0.0, 0.0),
        c1: Vec3::new(0.0, 1.0, 0.0),
        c2: Vec3::new(0.0, 0.0, 1.0),
    };

    /// Rotación Y (yaw) * X (pitch) * Z (roll), ángulos en radianes.
    pub fn from_euler(yaw: f32, pitch: f32, roll: f32) -> Self {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sr, cr) = roll.sin_cos();
        let ry = Self {
            c0: Vec3::new(cy, 0.0, -sy),
            c1: Vec3::new(0.0, 1.0, 0.0),
            c2: Vec3::new(sy, 0.0, cy),
        };
        let rx = Self {
            c0: Vec3::new(1.0, 0.0, 0.0),
            c1: Vec3::new(0.0, cp, sp),
            c2: Vec3::new(0.0, -sp, cp),
        };
        let rz = Self {
            c0: Vec3::new(cr, sr, 0.0),
            c1: Vec3::new(-sr, cr, 0.0),
            c2: Vec3::new(0.0, 0.0, 1.0),
        };
        ry.mul_mat(rx).mul_mat(rz)
    }

    /// Base cuyo eje Y local apunta en `dir`, girada `spin` radianes alrededor de él.
    pub fn from_y_axis(dir: Vec3, spin: f32) -> Self {
        let y = dir.normalized();
        let helper = if y.y.abs() < 0.95 { Vec3::UP } else { Vec3::new(1.0, 0.0, 0.0) };
        let x0 = helper.cross(y).normalized();
        let z0 = x0.cross(y).normalized();
        let (s, c) = spin.sin_cos();
        let x = x0 * c + z0 * s;
        let z = x.cross(y).normalized();
        Self { c0: x, c1: y, c2: z }
    }

    #[inline(always)]
    pub fn mul(&self, v: Vec3) -> Vec3 {
        self.c0 * v.x + self.c1 * v.y + self.c2 * v.z
    }

    /// Multiplica por la transpuesta (mundo -> local para rotaciones puras).
    #[inline(always)]
    pub fn tmul(&self, v: Vec3) -> Vec3 {
        Vec3::new(self.c0.dot(v), self.c1.dot(v), self.c2.dot(v))
    }

    pub fn mul_mat(&self, o: Self) -> Self {
        Self {
            c0: self.mul(o.c0),
            c1: self.mul(o.c1),
            c2: self.mul(o.c2),
        }
    }

    /// Fila `i` de la matriz.
    pub fn row(&self, i: usize) -> Vec3 {
        Vec3::new(self.c0.get(i), self.c1.get(i), self.c2.get(i))
    }
}

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

impl Ray {
    #[inline(always)]
    pub fn new(origin: Vec3, dir: Vec3) -> Self {
        Self { origin, dir }
    }

    #[inline(always)]
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.dir * t
    }
}

#[inline(always)]
pub fn fmin(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

#[inline(always)]
pub fn fmax(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

#[inline(always)]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline(always)]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
pub fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// Aproximación de Schlick para el término de Fresnel.
#[inline(always)]
pub fn schlick(cos_theta: f32, n1: f32, n2: f32) -> f32 {
    let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
    let m = (1.0 - cos_theta).clamp(0.0, 1.0);
    r0 + (1.0 - r0) * m * m * m * m * m
}
