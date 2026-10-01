//! Primitivas geométricas orientadas (cubo, esfera/elipsoide y cilindro).
//!
//! Cada primitiva guarda su centro, una rotación y sus medias dimensiones. Para
//! intersectar se lleva el rayo al espacio local, donde la figura es la unitaria
//! (cubo [-1,1]³, esfera de radio 1 o cilindro de radio 1 y alto [-1,1]).

use crate::{
    bvh::Aabb,
    math::{Mat3, Ray, Vec3},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Cube,
    Sphere,
    Cylinder,
}

/// Recortes alfa evaluados durante la intersección (hojas, abanicos...).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cutout {
    None,
    /// Hoja que se afina hacia la punta. Usa `param` = (v inicial, v final).
    Leaf,
    /// Abanico de mar en forma de red.
    Fan,
    /// Cola de pez: se abre hacia la punta y termina en dos lóbulos.
    Tail,
}

#[derive(Clone, Copy, Debug)]
pub struct Prim {
    pub shape: Shape,
    pub cutout: Cutout,
    pub material: u16,
    pub center: Vec3,
    pub rot: Mat3,
    pub half: Vec3,
    pub inv_half: Vec3,
    pub color: Vec3,
    pub param: [f32; 2],
    /// Radio de la esfera envolvente (descarte rápido antes de transformar el rayo).
    pub radius: f32,
    /// Vaivén con la corriente (plantas): dirección, amplitud en metros y fase.
    pub sway_dir: Vec3,
    pub sway_amp: f32,
    pub sway_phase: f32,
    /// > 0: onda de serpiente (número de onda a lo largo del cuerpo).
    pub sway_wave: f32,
}

/// Datos de superficie calculados solo para el impacto más cercano.
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub normal: Vec3,
    pub local: Vec3,
    pub local_normal: Vec3,
    pub uv: [f32; 2],
    /// Cubo: eje*2 + signo. Cilindro: 0 lateral, 1 tapa superior, 2 tapa inferior.
    pub face: u8,
}

impl Prim {
    pub fn new(shape: Shape, center: Vec3, half: Vec3, rot: Mat3, material: u16) -> Self {
        Self {
            shape,
            cutout: Cutout::None,
            material,
            center,
            rot,
            half,
            inv_half: Vec3::new(1.0 / half.x, 1.0 / half.y, 1.0 / half.z),
            color: Vec3::ONE,
            param: [0.0, 1.0],
            radius: match shape {
                Shape::Sphere => half.x.max(half.y).max(half.z),
                _ => half.length(),
            },
            sway_dir: Vec3::ZERO,
            sway_amp: 0.0,
            sway_phase: 0.0,
            sway_wave: 0.0,
        }
    }

    /// Escala la primitiva respecto al origen de su plantilla.
    pub fn scaled(mut self, k: f32) -> Self {
        self.center = self.center * k;
        self.half = self.half * k;
        self.inv_half = Vec3::new(1.0 / self.half.x, 1.0 / self.half.y, 1.0 / self.half.z);
        self.radius *= k;
        self.sway_amp *= k;
        self
    }

    /// Convierte el vaivén en una onda de serpiente con `k` radianes a lo largo del cuerpo.
    pub fn with_wave(mut self, k: f32) -> Self {
        self.radius -= self.max_sway();
        self.sway_wave = k;
        self.radius += self.max_sway();
        self
    }

    /// Activa el vaivén. `param` = (altura inicial, altura final) del segmento en la planta.
    pub fn with_sway(mut self, dir: Vec3, amp: f32, phase: f32) -> Self {
        self.sway_dir = Vec3::new(dir.x, 0.0, dir.z).normalized();
        self.sway_amp = amp;
        self.sway_phase = phase;
        self.radius += self.max_sway();
        self
    }

    fn max_sway(&self) -> f32 {
        if self.sway_wave > 0.0 {
            self.sway_amp * self.param[0].max(self.param[1]) * 1.05
        } else {
            self.sway_amp * 1.25 * self.param[0].max(self.param[1]).powi(2)
        }
    }

    /// Desplazamiento medio y pendiente del segmento en el instante `time`.
    #[inline(always)]
    fn sway_ag(&self, time: f32) -> (f32, f32) {
        let wave = |h: f32| {
            if self.sway_wave > 0.0 {
                // Onda que viaja de la cabeza a la cola (nado de serpiente).
                h * (time * 1.3 - self.sway_wave * h + self.sway_phase).sin()
            } else {
                h * h
                    * ((time * 1.3 + self.sway_phase + h * 1.4).sin()
                        + 0.25 * (time * 2.9 + self.sway_phase * 1.7).sin())
            }
        };
        let d0 = wave(self.param[0]) * self.sway_amp;
        let d1 = wave(self.param[1]) * self.sway_amp;
        ((d0 + d1) * 0.5, (d1 - d0) * 0.5)
    }

    pub fn with_color(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    pub fn with_cutout(mut self, cutout: Cutout, param: [f32; 2]) -> Self {
        self.cutout = cutout;
        self.param = param;
        self
    }

    pub fn aabb(&self) -> Aabb {
        let b = self.rest_aabb();
        let m = self.max_sway();
        Aabb {
            min: b.min - Vec3::new(m, 0.0, m),
            max: b.max + Vec3::new(m, 0.0, m),
        }
    }

    fn rest_aabb(&self) -> Aabb {
        let ext = match self.shape {
            Shape::Sphere => {
                // Caja exacta de un elipsoide rotado.
                let f = |i: usize| {
                    let r = self.rot.row(i);
                    ((r.x * self.half.x).powi(2) + (r.y * self.half.y).powi(2) + (r.z * self.half.z).powi(2)).sqrt()
                };
                Vec3::new(f(0), f(1), f(2))
            }
            _ => {
                let f = |i: usize| {
                    let r = self.rot.row(i).abs();
                    r.x * self.half.x + r.y * self.half.y + r.z * self.half.z
                };
                Vec3::new(f(0), f(1), f(2))
            }
        };
        Aabb {
            min: self.center - ext,
            max: self.center + ext,
        }
    }

    /// Lleva el rayo al espacio local unitario. Si la primitiva se mece, primero
    /// deshace el desplazamiento (una transformación afín: traslación + cizalla).
    #[inline(always)]
    fn to_local(&self, ray: &Ray, time: f32) -> (Vec3, Vec3) {
        let mut o = ray.origin - self.center;
        let mut d = ray.dir;
        if self.sway_amp > 0.0 {
            let (a, g) = self.sway_ag(time);
            let s = self.sway_dir;
            let e = self.rot.c1 * self.inv_half.y;
            let k = g / (1.0 + g * e.dot(s));
            o -= s * a;
            o -= s * (k * e.dot(o));
            d -= s * (k * e.dot(d));
        }
        (self.rot.tmul(o) * self.inv_half, self.rot.tmul(d) * self.inv_half)
    }

    /// Distancia al impacto (o `None`). `t` está en las mismas unidades que el rayo.
    #[inline]
    pub fn intersect(&self, ray: &Ray, tmin: f32, tmax: f32, time: f32) -> Option<f32> {
        // Descarte con la esfera envolvente (el rayo está normalizado).
        let oc = self.center - ray.origin;
        let b = oc.dot(ray.dir);
        let c = oc.dot(oc) - self.radius * self.radius;
        let disc = b * b - c;
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        if b - s > tmax || b + s < tmin {
            return None;
        }
        let (o, d) = self.to_local(ray, time);
        match self.shape {
            Shape::Cube => {
                let inv = Vec3::new(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
                let t1 = (Vec3::splat(-1.0) - o) * inv;
                let t2 = (Vec3::ONE - o) * inv;
                let near = t1.min(t2).max_elem();
                let far = t1.max(t2).min_elem();
                if near > far || far < tmin || near > tmax {
                    return None;
                }
                if near >= tmin && self.accept(o + d * near) {
                    return Some(near);
                }
                if far <= tmax && self.accept(o + d * far) {
                    return Some(far);
                }
                None
            }
            Shape::Sphere => {
                let a = d.dot(d);
                let b = o.dot(d);
                let c = o.dot(o) - 1.0;
                let disc = b * b - a * c;
                if disc < 0.0 {
                    return None;
                }
                let s = disc.sqrt();
                let t0 = (-b - s) / a;
                if t0 >= tmin && t0 <= tmax {
                    return Some(t0);
                }
                let t1 = (-b + s) / a;
                if t1 >= tmin && t1 <= tmax {
                    return Some(t1);
                }
                None
            }
            Shape::Cylinder => {
                let mut best = f32::INFINITY;
                let a = d.x * d.x + d.z * d.z;
                if a > 1e-12 {
                    let b = o.x * d.x + o.z * d.z;
                    let c = o.x * o.x + o.z * o.z - 1.0;
                    let disc = b * b - a * c;
                    if disc >= 0.0 {
                        let s = disc.sqrt();
                        for t in [(-b - s) / a, (-b + s) / a] {
                            if t >= tmin && t <= tmax && t < best {
                                let y = o.y + d.y * t;
                                if y.abs() <= 1.0 {
                                    best = t;
                                    break;
                                }
                            }
                        }
                    }
                }
                if d.y.abs() > 1e-12 {
                    for cap in [1.0f32, -1.0] {
                        let t = (cap - o.y) / d.y;
                        if t >= tmin && t <= tmax && t < best {
                            let x = o.x + d.x * t;
                            let z = o.z + d.z * t;
                            if x * x + z * z <= 1.0 {
                                best = t;
                            }
                        }
                    }
                }
                if best.is_finite() { Some(best) } else { None }
            }
        }
    }

    /// ¿El punto está dentro del volumen (en reposo)?
    pub fn contains(&self, p: Vec3, margin: f32) -> bool {
        let q = self.rot.tmul(p - self.center);
        let l = Vec3::new(
            q.x / (self.half.x + margin),
            q.y / (self.half.y + margin),
            q.z / (self.half.z + margin),
        );
        match self.shape {
            Shape::Cube => l.x.abs() <= 1.0 && l.y.abs() <= 1.0 && l.z.abs() <= 1.0,
            Shape::Sphere => l.dot(l) <= 1.0,
            Shape::Cylinder => l.x * l.x + l.z * l.z <= 1.0 && l.y.abs() <= 1.0,
        }
    }

    /// Prueba de recorte alfa en coordenadas locales.
    #[inline(always)]
    fn accept(&self, p: Vec3) -> bool {
        match self.cutout {
            Cutout::None => true,
            Cutout::Leaf => {
                let v = self.param[0] + (self.param[1] - self.param[0]) * (p.y * 0.5 + 0.5);
                // Base angosta, más ancha a un cuarto del largo y punta afilada.
                let width = ((1.0 - v).max(0.0).powf(1.1) * (0.35 + 2.6 * v)).min(1.0);
                p.x.abs() <= width
            }
            Cutout::Tail => {
                let v = p.y * 0.5 + 0.5;
                let ax = p.x.abs();
                ax <= 0.22 + 0.78 * v.powf(0.8) && v <= 0.78 + 0.22 * ax
            }
            Cutout::Fan => {
                // Abanico: contorno elíptico y una red de ramas radiales y arcos.
                let x = p.x;
                let y = p.y * 0.5 + 0.5;
                let shape = x * x + (y - 0.45).powi(2) * 3.2;
                if shape > 1.0 || y < 0.0 {
                    return false;
                }
                let r = (x * x + (y + 0.25).powi(2)).sqrt();
                // Ramas radiales onduladas (se van dividiendo) y una red fina irregular.
                let ang = x.atan2(y + 0.25) + (r * 9.0).sin() * 0.06;
                let branches = 5.0 + (r * 8.0).floor();
                let radial = (ang * branches).sin().abs() < 0.16 + 0.22 * (1.0 - r);
                let wobble = (ang * 13.0).sin() * 0.04 + (x * 21.0).sin() * 0.02;
                let mesh = ((r + wobble) * 30.0).sin().abs() < 0.16;
                let edge = shape > 0.86;
                let stem = x.abs() < 0.06 && y < 0.4;
                radial || mesh || edge || stem
            }
        }
    }

    /// Calcula normal, coordenadas locales y UV del punto `t` del rayo.
    pub fn surface(&self, ray: &Ray, t: f32, time: f32) -> Surface {
        let (o, d) = self.to_local(ray, time);
        let p = o + d * t;
        let (ln, uv, face) = match self.shape {
            Shape::Cube => {
                let a = p.abs();
                if a.x >= a.y && a.x >= a.z {
                    let s = p.x.signum();
                    (Vec3::new(s, 0.0, 0.0), [p.z * 0.5 + 0.5, p.y * 0.5 + 0.5], if s > 0.0 { 1 } else { 0 })
                } else if a.y >= a.z {
                    let s = p.y.signum();
                    (Vec3::new(0.0, s, 0.0), [p.x * 0.5 + 0.5, p.z * 0.5 + 0.5], if s > 0.0 { 3 } else { 2 })
                } else {
                    let s = p.z.signum();
                    (Vec3::new(0.0, 0.0, s), [p.x * 0.5 + 0.5, p.y * 0.5 + 0.5], if s > 0.0 { 5 } else { 4 })
                }
            }
            Shape::Sphere => {
                let u = p.z.atan2(p.x) * (0.5 / std::f32::consts::PI) + 0.5;
                (p, [u, p.y * 0.5 + 0.5], 0)
            }
            Shape::Cylinder => {
                let r2 = p.x * p.x + p.z * p.z;
                let to_cap = (1.0 - p.y.abs()).abs();
                let to_side = (1.0 - r2.sqrt()).abs();
                if to_cap < to_side {
                    let s = p.y.signum();
                    (Vec3::new(0.0, s, 0.0), [p.x * 0.5 + 0.5, p.z * 0.5 + 0.5], if s > 0.0 { 1 } else { 2 })
                } else {
                    let u = p.z.atan2(p.x) * (0.5 / std::f32::consts::PI) + 0.5;
                    (Vec3::new(p.x, 0.0, p.z), [u, p.y * 0.5 + 0.5], 0)
                }
            }
        };
        let normal = self.rot.mul(ln * self.inv_half).normalized();
        let uv = if self.cutout == Cutout::Leaf {
            [uv[0], self.param[0] + (self.param[1] - self.param[0]) * uv[1]]
        } else {
            uv
        };
        Surface {
            normal,
            local: p,
            local_normal: ln,
            uv,
            face,
        }
    }
}
