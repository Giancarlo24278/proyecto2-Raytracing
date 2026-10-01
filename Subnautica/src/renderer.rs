//! Trazado de rayos: sombreado, reflexión, refracción, niebla submarina,
//! haces de luz volumétricos y render paralelo con hilos de la librería estándar.

use crate::{
    camera::Camera,
    fauna::Glow,
    lighting::caustic,
    material::{self, Kind, Mapping, Material},
    math::{Ray, Vec3, fract, schlick, smoothstep},
    noise::{value2, value3, value3_grad},
    primitives::{Prim, Surface},
    scene::{FAR, Frame, Hit, HitKind, Scene},
    texture::Texture,
};
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

/// Coeficiente de extinción del agua por metro (el rojo se pierde primero).
pub const SIGMA: Vec3 = Vec3::new(0.050, 0.030, 0.027);
const AMB_UP: Vec3 = Vec3::new(0.11, 0.21, 0.25);
const AMB_DOWN: Vec3 = Vec3::new(0.03, 0.07, 0.085);
const EPS: f32 = 1.5e-3;

#[derive(Clone, Copy, Debug)]
pub struct Quality {
    pub soft_shadows: bool,
    pub max_depth: u32,
    pub god_rays: bool,
    pub god_steps: u32,
}

impl Quality {
    pub const INTERACTIVE: Self = Self {
        soft_shadows: false,
        max_depth: 3,
        god_rays: true,
        god_steps: 7,
    };
    pub const HIGH: Self = Self {
        soft_shadows: true,
        max_depth: 6,
        god_rays: true,
        god_steps: 14,
    };
}

struct Tracer<'a> {
    scene: &'a Scene,
    frame: &'a Frame,
    q: Quality,
    /// Iluminación según la hora (día/noche).
    env: Ambience,
}

/// Factores de luz que cambian entre el día y la noche.
#[derive(Clone, Copy)]
pub struct Ambience {
    /// Color del sol (de día) o de la luna (de noche).
    pub sun: Vec3,
    pub ambient: Vec3,
    pub water: Vec3,
    pub sky: Vec3,
    pub glow: f32,
    /// Intensidad de la luz que las fuentes proyectan sobre lo que las rodea.
    pub lights: f32,
    pub shafts: Vec3,
}

impl Ambience {
    /// `night` = 0 de día, 1 de noche (con transición suave entre ambos).
    pub fn new(scene: &Scene, night: f32) -> Self {
        let n = night.clamp(0.0, 1.0);
        let n = n * n * (3.0 - 2.0 * n);
        // Luz de luna: azulada y mucho más débil, pero suficiente para ver la escena.
        let moon = Vec3::new(0.30, 0.42, 0.75) * 0.32;
        Self {
            sun: scene.sun_color.lerp(moon, n),
            ambient: Vec3::ONE.lerp(Vec3::new(0.30, 0.40, 0.65), n),
            water: Vec3::ONE.lerp(Vec3::new(0.10, 0.16, 0.30), n),
            sky: Vec3::ONE.lerp(Vec3::new(0.025, 0.04, 0.09), n),
            glow: 1.0 + 1.0 * n,
            lights: 1.0 + 0.35 * n,
            shafts: SHAFT_COLOR.lerp(Vec3::new(0.05, 0.09, 0.20), n),
        }
    }
}

impl Tracer<'_> {
    /// Color visto por un rayo y distancia recorrida en el agua.
    fn trace_dist(&self, ray: &Ray, depth: u32, weight: f32) -> (Vec3, f32) {
        let hit = self.scene.intersect(ray, EPS, FAR, self.frame);
        let dist = hit.map_or(FAR, |h| h.t);
        let trans = (SIGMA * -dist).exp();
        let fog = self.water(ray.dir);
        let mut color = match hit {
            // Si el agua ya tapa casi todo, no vale la pena sombrear.
            Some(h) if weight * trans.max_elem() > 0.01 => {
                self.shade(ray, h, depth, weight * trans.max_elem()) * trans
            }
            _ => Vec3::ZERO,
        };
        color += fog * (Vec3::ONE - trans);
        (color, dist)
    }

    #[inline]
    fn trace(&self, ray: &Ray, depth: u32, weight: f32) -> Vec3 {
        self.trace_dist(ray, depth, weight).0
    }

    fn shade(&self, ray: &Ray, hit: Hit, depth: u32, weight: f32) -> Vec3 {
        let p = ray.at(hit.t);
        let view = -ray.dir;
        let lib = &self.scene.lib;
        match hit.kind {
            HitKind::Floor => {
                let mat = &lib.materials[material::SAND as usize];
                let n = sand_normal(p, Vec3::UP, hit.t, mat.bump);
                let albedo = sample_texture(&lib.textures[mat.texture], mat.mapping, p, n, None);
                self.light(p, n, view, mat, albedo)
            }
            HitKind::Water => self.shade_water(ray, p, hit.t, depth, weight),
            HitKind::Bubble(i) => {
                let b = &self.frame.bubbles[i as usize];
                let n = (p - b.center) * (1.0 / b.radius);
                let mat = &lib.materials[material::BUBBLE as usize];
                let tint = lib.textures[mat.texture].sample(n.x * 0.5 + 0.5, n.y * 0.5 + 0.5);
                self.shade_dielectric(ray, p, n, mat, tint, depth, weight)
            }
            HitKind::Prim(_) | HitKind::Creature(..) => {
                let (prim, s) = match hit.kind {
                    HitKind::Creature(ci, pi) => {
                        // Animal: el rayo se lleva a su espacio local y la normal vuelve al mundo.
                        let c = &self.frame.creatures[ci as usize];
                        let prim = &self.scene.species[c.species as usize].prims[pi as usize];
                        let local = Ray::new(c.rot.tmul(ray.origin - c.pos), c.rot.tmul(ray.dir));
                        let mut s = prim.surface(&local, hit.t, c.time);
                        s.normal = c.rot.mul(s.normal);
                        (prim, s)
                    }
                    HitKind::Prim(i) => {
                        let prim = &self.scene.prims[i as usize];
                        (prim, prim.surface(ray, hit.t, self.frame.time))
                    }
                    _ => unreachable!(),
                };
                let mat = &lib.materials[prim.material as usize];
                if mat.kind == Kind::Glass {
                    let tint = albedo(lib.textures.as_slice(), mat, prim, &s, p, s.normal);
                    return self.shade_dielectric(ray, p, s.normal, mat, tint, depth, weight);
                }
                let mut n = s.normal;
                if ray.dir.dot(n) > 0.0 {
                    n = -n;
                }
                let mut col = albedo(lib.textures.as_slice(), mat, prim, &s, p, n);
                match mat.kind {
                    Kind::Sand => {
                        // Dunas: la normal se mezcla con la del suelo cerca de la base.
                        let blend = smoothstep(0.0, 0.35, p.y);
                        n = Vec3::UP.lerp(n, blend).normalized();
                        n = sand_normal(p, n, hit.t, mat.bump);
                    }
                    Kind::Rock => {
                        n = rock_normal(p, n, hit.t, mat.bump);
                        let moss_tex = &lib.textures[mat.overlay.unwrap_or(mat.texture)];
                        let noise = value3(p * 0.9) - 0.5;
                        let moss = smoothstep(0.40, 0.85, n.y + noise * 0.75) * prim.param[1];
                        if moss > 0.0 {
                            let m = moss_tex.sample(p.x * 0.45, p.z * 0.45);
                            col = col.lerp(m * prim.color.lerp(Vec3::ONE, 0.6), moss);
                        }
                    }
                    Kind::Sponge => {
                        if s.face == 1 {
                            let r = (s.local.x * s.local.x + s.local.z * s.local.z).sqrt();
                            if r < 0.74 {
                                // Interior oscuro del tubo.
                                let k = r / 0.74;
                                let inner = Vec3::new(0.10, 0.055, 0.015) * (0.35 + 0.65 * k * k);
                                return inner * (self.ambient(p, n) * 0.8 + Vec3::splat(0.05));
                            }
                            col = col * 1.25;
                        }
                    }
                    Kind::Brain => {
                        n = bumpy_normal(p * 2.2, n, 0.6);
                    }
                    Kind::ShellPlate => {
                        if s.face == 0 {
                            // Canto de la placa.
                            col = Vec3::new(0.12, 0.13, 0.06);
                        }
                    }
                    _ => {}
                }
                let mut c = self.light(p, n, view, mat, col);
                if mat.glow > 0.0 {
                    // Las partes claras de la textura emiten luz propia.
                    let mask = smoothstep(0.25, 0.6, col.luminance());
                    c += col * (mat.glow * mask * self.env.glow);
                }
                if mat.kind != Kind::Metal && mat.reflectivity > 0.0 {
                    // Brillo "mojado": reflejo barato del entorno (sin lanzar otro rayo).
                    let cos = view.dot(n).clamp(0.0, 1.0);
                    let f = mat.reflectivity + (1.0 - mat.reflectivity) * (1.0 - cos).powi(5) * 0.25;
                    let env = self.water(ray.dir.reflect(n));
                    c = c * (1.0 - f) + env * f * self.ambient(p, n).max_elem().min(1.0) * 2.0;
                }
                if mat.kind == Kind::Metal {
                    let cos = view.dot(n).clamp(0.0, 1.0);
                    let f = mat.reflectivity + (1.0 - mat.reflectivity) * (1.0 - cos).powi(5);
                    let rd = ray.dir.reflect(n);
                    let reflected = if depth < self.q.max_depth && weight * f > 0.03 {
                        self.trace(&Ray::new(p + n * EPS * 2.0, rd), depth + 1, weight * f)
                    } else {
                        self.water(rd)
                    };
                    let metal_tint = col.lerp(Vec3::ONE, 0.35) * 1.15;
                    c = c * (1.0 - f) + reflected * metal_tint * f;
                }
                c
            }
        }
    }

    #[inline]
    fn sun_visibility(&self, p: Vec3) -> f32 {
        if self.q.soft_shadows {
            self.scene.shadow.visibility(p)
        } else {
            self.scene.shadow.visibility_bilinear(p)
        }
    }

    /// Color del agua lejana / skybox submarino (oscurecido de noche).
    #[inline]
    fn water(&self, dir: Vec3) -> Vec3 {
        self.scene.haze.sample(dir) * self.env.water
    }

    /// Luz ambiental (hemisférica + oclusión precalculada).
    #[inline]
    fn ambient(&self, p: Vec3, n: Vec3) -> Vec3 {
        let ao = self.scene.ao.sample(p + n * 0.3);
        let ao = ao * ao * (3.0 - 2.0 * ao);
        AMB_DOWN.lerp(AMB_UP, n.y * 0.5 + 0.5) * self.env.ambient * (0.18 + 0.82 * ao)
    }

    /// Iluminación local: sol con sombras suaves y cáusticas, ambiente, especular.
    fn light(&self, p: Vec3, n: Vec3, view: Vec3, mat: &Material, albedo: Vec3) -> Vec3 {
        let scene = self.scene;
        let l = scene.sun_dir;
        let ndl = n.dot(l);
        let diffuse = ndl.max(0.0) + mat.translucency * (-ndl).max(0.0) * 0.7;
        let mut sun = 0.0;
        if diffuse > 0.0 || mat.specular > 0.0 {
            let vis = self.sun_visibility(p + n * 0.05);
            if vis > 0.0 {
                // Las cáusticas se notan más en superficies que miran al sol.
                let c = caustic(scene, p, self.frame.time);
                sun = vis * (1.0 + (c - 1.0) * ndl.max(0.0).powf(1.5));
            }
        }
        // La luz del sol pierde rojo al atravesar la columna de agua.
        let column = (scene.water_y - p.y).max(0.0) / l.y;
        let sun_color = self.env.sun * (SIGMA * (-column * 0.35)).exp();
        let glow = self.glow_light(p, n);
        let mut c = albedo * (sun_color * (diffuse * sun) + self.ambient(p, n) + glow) + mat.emission;
        if sun > 0.0 && ndl > 0.0 {
            let h = (l + view).normalized();
            let spec = n.dot(h).max(0.0).powf(mat.shininess) * mat.specular;
            c += sun_color * (spec * sun);
        }
        c
    }

    /// Luz de las fuentes luminosas cercanas (corales, hongos, ojos de peeper).
    #[inline]
    fn glow_light(&self, p: Vec3, n: Vec3) -> Vec3 {
        let mut acc = Vec3::ZERO;
        let mut add = |g: &Glow| {
            let v = g.pos - p;
            let d2 = v.length_sq();
            let r2 = g.radius * g.radius;
            // Muy cerca de la fuente (el propio animal o coral) no se ilumina:
            // ya brilla por su textura.
            if d2 < r2 && d2 > 0.09 {
                let wrap = n.dot(v) / d2.sqrt() * 0.5 + 0.5;
                let fall = 1.0 - d2 / r2;
                acc += g.color * (fall * fall * wrap * self.env.lights);
            }
        };
        for &i in self.scene.glow_grid.near(p) {
            add(&self.scene.glows[i as usize]);
        }
        for g in &self.frame.glows {
            add(g);
        }
        acc
    }

    /// Halo visible en el agua alrededor de cada fuente de luz (solo rayos primarios).
    /// `list` son solo las luces cuyo halo cae en el bloque de pantalla de este píxel.
    fn halos(&self, ray: &Ray, dist: f32, glows: &[Glow], list: &[u16]) -> Vec3 {
        let mut acc = Vec3::ZERO;
        for &i in list {
            let g = &glows[i as usize];
            let oc = g.pos - ray.origin;
            let t = oc.dot(ray.dir);
            if t < -g.halo * 3.0 {
                continue;
            }
            let t = t.clamp(0.0, dist);
            let q = oc - ray.dir * t;
            let d2 = q.length_sq();
            let s2 = g.halo * g.halo;
            if d2 < s2 * 9.0 {
                acc += g.color * ((-d2 / s2).exp() * (-0.05 * t).exp() * 0.35 * self.env.glow);
            }
        }
        acc
    }

    /// Superficie del agua vista desde abajo: ventana de Snell y reflexión interna total.
    fn shade_water(&self, ray: &Ray, p: Vec3, dist: f32, depth: u32, weight: f32) -> Vec3 {
        let scene = self.scene;
        let lib = &scene.lib;
        let mat = &lib.materials[material::WATER as usize];
        let n = -wave_normal(p.x, p.z, self.frame.time, dist);
        let cos_i = (-ray.dir.dot(n)).clamp(0.0, 1.0);
        let refracted = ray.dir.refract(n, mat.ior);
        let mut f = schlick(cos_i, mat.ior, 1.0).max(mat.reflectivity);
        let mut c = Vec3::ZERO;
        if let Some(r) = refracted {
            let tint = lib.textures[mat.texture].sample(p.x * 0.08 + self.frame.time * 0.01, p.z * 0.08);
            c += scene.sky.sample(r) * self.env.sky * tint * (1.0 - f) * mat.transparency;
        } else {
            f = 1.0;
        }
        let rd = ray.dir.reflect(n);
        let reflected = if depth < self.q.max_depth.min(2) && weight * f > 0.05 {
            self.trace(&Ray::new(p + n * 0.01, rd), depth + 1, weight * f)
        } else {
            self.water(rd)
        };
        c += reflected * f;
        // Destello especular del sol sobre las olas.
        let h = (scene.sun_dir - ray.dir).normalized();
        c += self.env.sun * (-n).dot(h).max(0.0).powf(mat.shininess) * mat.specular;
        c
    }

    /// Materiales dieléctricos: cristal (refracción) y burbujas de aire (ior < 1).
    fn shade_dielectric(
        &self,
        ray: &Ray,
        p: Vec3,
        n_out: Vec3,
        mat: &Material,
        tint: Vec3,
        depth: u32,
        weight: f32,
    ) -> Vec3 {
        let front = ray.dir.dot(n_out) < 0.0;
        let n = if front { n_out } else { -n_out };
        let (n1, n2) = if front { (1.0, mat.ior) } else { (mat.ior, 1.0) };
        let cos_i = (-ray.dir.dot(n)).clamp(0.0, 1.0);
        let refracted = ray.dir.refract(n, n1 / n2);
        let mut f = schlick(cos_i, n1, n2).max(mat.reflectivity);
        if refracted.is_none() {
            f = 1.0;
        }
        let can_recurse = depth < self.q.max_depth;
        let mut c = mat.emission;
        if let Some(r) = refracted {
            let k = (1.0 - f) * mat.transparency;
            let transmitted = if can_recurse && weight * k > 0.02 {
                self.trace(&Ray::new(p - n * EPS * 2.0, r), depth + 1, weight * k)
            } else {
                self.water(r)
            };
            // El color del cristal se aplica al entrar (no en cada cara).
            let t = if front { tint } else { Vec3::ONE };
            c += transmitted * t * k;
        }
        let rd = ray.dir.reflect(n);
        let reflected = if can_recurse && weight * f > 0.06 {
            self.trace(&Ray::new(p + n * EPS * 2.0, rd), depth + 1, weight * f)
        } else {
            self.water(rd) * 1.2
        };
        c += reflected * f;
        // Parte "opaca" (difusa) y brillo especular del sol.
        let opaque = 1.0 - mat.transparency;
        let l = self.scene.sun_dir;
        let vis = self.sun_visibility(p + n * 0.05);
        if opaque > 0.0 {
            let diffuse = n.dot(l).max(0.0) * vis;
            c += tint * (self.env.sun * diffuse + self.ambient(p, n)) * opaque;
        }
        let h = (l - ray.dir).normalized();
        c += self.env.sun * (n.dot(h).max(0.0).powf(mat.shininess) * mat.specular * vis);
        c
    }

    /// Intensidad de los haces de luz (dispersión del sol) a lo largo del rayo primario.
    fn god_rays(&self, ray: &Ray, dist: f32, jitter: f32) -> f32 {
        let scene = self.scene;
        let steps = self.q.god_steps.max(1);
        let (o, d) = (ray.origin, ray.dir);
        let mut max_d = dist.min(32.0);
        if d.y > 1e-4 {
            max_d = max_d.min((scene.water_y - o.y) / d.y);
        }
        if max_d <= 0.0 {
            return 0.0;
        }
        let dt = max_d / steps as f32;
        let s0 = jitter * dt;
        let l = scene.sun_dir;
        let time = self.frame.time;
        // Todo lo que se consulta es lineal en la distancia `s`, así que se avanza
        // sumando incrementos en vez de recalcular proyecciones en cada paso.
        let kx = l.x / l.y;
        let kz = l.z / l.y;
        let w = scene.water_y - o.y;
        let (qx0, qx1) = (o.x + kx * w, d.x - kx * d.y);
        let (qz0, qz1) = (o.z + kz * w, d.z - kz * d.y);
        let mut u = (qx0 + qx1 * s0) * SHAFT_SCALE + time * 0.006;
        let mut v = (qz0 + qz1 * s0) * SHAFT_SCALE - time * 0.004;
        let du = qx1 * dt * SHAFT_SCALE;
        let dv = qz1 * dt * SHAFT_SCALE;
        let (mut sm, dsm) = scene.shadow.ray_coords(o, d, s0, dt);
        // Atenuación exp(-0.045 * (s + 0.6 * columna)) como progresión geométrica.
        let a = -0.045 * (s0 + 0.6 * (w - d.y * s0) / l.y);
        let b = -0.045 * (1.0 - 0.6 * d.y / l.y);
        let mut att = a.exp();
        let att_step = (b * dt).exp();
        let mut acc = 0.0;
        for _ in 0..steps {
            let sh = scene.shafts.sample(u, v);
            let pattern = ((sh - 0.42) * (1.0 / 0.36)).clamp(0.0, 1.0);
            if pattern > 0.0 {
                let pattern = pattern * pattern * (3.0 - 2.0 * pattern);
                acc += pattern * scene.shadow.lit_at(sm[0], sm[1], sm[2]) * att;
            }
            u += du;
            v += dv;
            sm[0] += dsm[0];
            sm[1] += dsm[1];
            sm[2] += dsm[2];
            att *= att_step;
        }
        let cos_t = d.dot(l);
        let g = 0.55;
        let phase = (1.0 - g * g) / (1.0 + g * g - 2.0 * g * cos_t).powf(1.5) * 0.3 + 0.35;
        acc * dt * phase * 0.055
    }
}

const SHAFT_SCALE: f32 = 0.07;
const HALO_TILE: usize = 16;
const SHAFT_COLOR: Vec3 = Vec3::new(0.30, 0.66, 0.66);

/// Muestreo del albedo de un material según su tipo de mapeo.
fn albedo(textures: &[Texture], mat: &Material, prim: &Prim, s: &Surface, p: Vec3, n: Vec3) -> Vec3 {
    let tex = &textures[mat.texture];
    let base = match mat.mapping {
        Mapping::ObjectTriplanar(k) => triplanar(tex, s.local * prim.half * k, s.local_normal),
        m => sample_texture(tex, m, p, n, Some(s)),
    };
    base * mat.albedo * prim.color
}

fn sample_texture(tex: &Texture, mapping: Mapping, p: Vec3, n: Vec3, s: Option<&Surface>) -> Vec3 {
    match mapping {
        Mapping::Uv { su, sv } => {
            let uv = s.map_or([0.0, 0.0], |s| s.uv);
            tex.sample(uv[0] * su, uv[1] * sv)
        }
        Mapping::WorldTop(k) => tex.sample(p.x * k, p.z * k),
        Mapping::WorldTriplanar(k) | Mapping::ObjectTriplanar(k) => triplanar(tex, p * k, n),
    }
}

#[inline]
fn triplanar(tex: &Texture, q: Vec3, n: Vec3) -> Vec3 {
    let a = n.abs();
    let w = Vec3::new(a.x.powi(4), a.y.powi(4), a.z.powi(4));
    let threshold = (w.x + w.y + w.z) * 0.03;
    let mut c = Vec3::ZERO;
    let mut used = 0.0;
    if w.x > threshold {
        c += tex.sample(q.z, -q.y) * w.x;
        used += w.x;
    }
    if w.y > threshold {
        c += tex.sample(q.x, q.z) * w.y;
        used += w.y;
    }
    if w.z > threshold {
        c += tex.sample(q.x, -q.y) * w.z;
        used += w.z;
    }
    c * (1.0 / used)
}

/// Relieve de la arena: ondulaciones grandes y rizos pequeños (bump mapping analítico).
fn sand_normal(p: Vec3, n: Vec3, dist: f32, strength: f32) -> Vec3 {
    let warp = value2(p.x * 0.18, p.z * 0.18, 0) * 5.0;
    let ph1 = p.x * 1.15 + p.z * 0.45 + warp;
    let g1 = (ph1.cos() + 0.45 * (2.0 * ph1).cos()) * 0.10;
    let ph2 = p.x * 6.3 + p.z * 2.4 + warp * 2.3 + value2(p.x * 0.9, p.z * 0.9, 0) * 2.0;
    let fine = 1.0 / (1.0 + dist * 0.12);
    let g2 = (ph2.cos() + 0.4 * (2.0 * ph2).cos()) * 0.06 * fine;
    let gx = (g1 * 1.15 + g2 * 6.3 * 0.18) * strength;
    let gz = (g1 * 0.45 + g2 * 2.4 * 0.18) * strength;
    let bumped = Vec3::new(n.x - gx, n.y, n.z - gz);
    bumped.normalized()
}

/// Relieve genérico con ruido 3D.
fn bumpy_normal(q: Vec3, n: Vec3, strength: f32) -> Vec3 {
    let (_, g) = value3_grad(q);
    let tangential = g - n * g.dot(n);
    (n - tangential * strength).normalized()
}

/// Relieve rocoso con dos escalas de ruido 3D.
fn rock_normal(p: Vec3, n: Vec3, dist: f32, strength: f32) -> Vec3 {
    let (_, g1) = value3_grad(p * 1.6);
    let (_, g2) = value3_grad(p * 5.5 + Vec3::splat(17.0));
    let fine = 1.0 / (1.0 + dist * 0.08);
    let g = (g1 * 0.85 + g2 * (0.45 * fine)) * strength;
    let tangential = g - n * g.dot(n);
    (n - tangential).normalized()
}

/// Normal de la superficie del agua (apunta hacia arriba): suma de ondas.
pub fn wave_normal(x: f32, z: f32, t: f32, dist: f32) -> Vec3 {
    const WAVES: [(f32, f32, f32, f32, f32); 7] = [
        (0.80, 0.60, 0.55, 0.30, 0.9),
        (-0.40, 0.92, 0.90, 0.16, 1.3),
        (0.95, -0.31, 1.60, 0.07, 1.9),
        (-0.70, -0.71, 2.70, 0.035, 2.6),
        (0.20, 0.98, 4.10, 0.018, 3.4),
        (0.99, 0.14, 6.20, 0.010, 4.3),
        (-0.55, 0.83, 9.00, 0.006, 5.1),
    ];
    let mut gx = 0.0;
    let mut gz = 0.0;
    for (dx, dz, k, a, w) in WAVES {
        // Las ondas finas se apagan con la distancia (evita parpadeo/aliasing).
        let lod = 1.0 / (1.0 + dist * k * 0.04);
        let c = ((dx * x + dz * z) * k + w * t).cos() * a * k * 0.35 * lod;
        gx += c * dx;
        gz += c * dz;
    }
    Vec3::new(-gx, 1.0, -gz).normalized()
}

// ---------------------------------------------------------------------------
// Render paralelo y postproceso.
// ---------------------------------------------------------------------------

pub struct Target {
    pub w: usize,
    pub h: usize,
    pub accum: Vec<Vec3>,
    hdr: Vec<Vec3>,
    scatter: Vec<f32>,
    pub rgba: Vec<u8>,
}

impl Target {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            accum: vec![Vec3::ZERO; w * h],
            hdr: vec![Vec3::ZERO; w * h],
            scatter: vec![0.0; w * h],
            rgba: vec![255; w * h * 4],
        }
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        if w != self.w || h != self.h {
            *self = Self::new(w, h);
        }
    }

    /// Cambia de tamaño reescalando la imagen actual (para que no aparezca negro
    /// mientras se refina por franjas).
    pub fn resize_preserving(&mut self, w: usize, h: usize) {
        if w == self.w && h == self.h {
            return;
        }
        let mut next = Self::new(w, h);
        for y in 0..h {
            let sy = y * self.h / h;
            for x in 0..w {
                let sx = x * self.w / w;
                let s = (sy * self.w + sx) * 4;
                let d = (y * w + x) * 4;
                next.rgba[d..d + 4].copy_from_slice(&self.rgba[s..s + 4]);
            }
        }
        *self = next;
    }
}

struct ToneMap {
    lut: Vec<u8>,
}

impl ToneMap {
    fn new() -> Self {
        let lut = (0..4096)
            .map(|i| {
                let x = i as f32 / 4095.0;
                (x.powf(1.0 / 2.2) * 255.0 + 0.5) as u8
            })
            .collect();
        Self { lut }
    }

    #[inline(always)]
    fn map(&self, c: Vec3, vignette: f32) -> [u8; 4] {
        let aces = |x: f32| {
            let x = x * 1.05;
            ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
        };
        let c = c * vignette;
        // Un poco más de saturación para colores vivos.
        let l = c.luminance();
        let c = (Vec3::splat(l) + (c - Vec3::splat(l)) * 1.25).max(Vec3::ZERO);
        let f = |v: f32| self.lut[(aces(v) * 4095.0) as usize];
        [f(c.x), f(c.y), f(c.z), 255]
    }
}

fn halton(mut i: u32, base: u32) -> f32 {
    let mut f = 1.0;
    let mut r = 0.0;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// Renderiza las filas `rows` de un cuadro.
/// `pass = 0` sobrescribe; `pass > 0` acumula otra muestra con jitter (antialiasing progresivo).
pub fn render(
    scene: &Scene,
    frame: &Frame,
    camera: &Camera,
    quality: Quality,
    target: &mut Target,
    pass: u32,
    rows: std::ops::Range<usize>,
) {
    let (w, h) = (target.w, target.h);
    let rows = rows.start.min(h)..rows.end.min(h);
    if rows.is_empty() {
        return;
    }
    let accumulate = pass > 0;
    let inv_samples = 1.0 / (pass + 1) as f32;
    let (jx, jy) = if accumulate {
        (halton(pass + 1, 2), halton(pass + 1, 3))
    } else {
        (0.5, 0.5)
    };
    let env = Ambience::new(scene, frame.night);
    let shaft_color = env.shafts;
    let tracer = Tracer {
        scene,
        frame,
        q: quality,
        env,
    };
    static TONE: std::sync::OnceLock<ToneMap> = std::sync::OnceLock::new();
    let tone = TONE.get_or_init(ToneMap::new);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    const ROWS: usize = 2;
    let row0 = rows.start;
    let span = row0 * w..rows.end * w;

    // Qué halos tocan cada bloque de 16x16 píxeles (así cada píxel revisa pocas luces).
    let all_glows: Vec<Glow> = scene.glows.iter().chain(frame.glows.iter()).copied().collect();
    let tiles_x = w.div_ceil(HALO_TILE);
    let tiles_y = h.div_ceil(HALO_TILE);
    let mut halo_tiles: Vec<Vec<u16>> = vec![Vec::new(); tiles_x * tiles_y];
    for (i, g) in all_glows.iter().enumerate() {
        let reach = g.halo * 3.0;
        let (sx, sy, depth) = camera.project(g.pos);
        let (x0, x1, y0, y1) = if depth < reach * 1.5 {
            (0, tiles_x - 1, 0, tiles_y - 1)
        } else {
            let r = camera.screen_size(reach, depth) * 1.2;
            let r_px = r * h as f32;
            let px = sx * w as f32;
            let py = sy * h as f32;
            if px + r_px < 0.0 || py + r_px < 0.0 || px - r_px > w as f32 || py - r_px > h as f32 {
                continue;
            }
            let t = HALO_TILE as f32;
            (
                ((px - r_px) / t).max(0.0) as usize,
                (((px + r_px) / t) as usize).min(tiles_x - 1),
                ((py - r_px) / t).max(0.0) as usize,
                (((py + r_px) / t) as usize).min(tiles_y - 1),
            )
        };
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                halo_tiles[ty * tiles_x + tx].push(i as u16);
            }
        }
    }
    let all_glows = &all_glows;
    let halo_tiles = &halo_tiles;

    // Fase 1: trazar rayos (color sin haces de luz + intensidad de los haces).
    {
        let mut work: Vec<Vec<(usize, &mut [Vec3], &mut [f32])>> = (0..threads).map(|_| Vec::new()).collect();
        for (i, (hdr, sc)) in target.hdr[span.clone()]
            .chunks_mut(w * ROWS)
            .zip(target.scatter[span.clone()].chunks_mut(w * ROWS))
            .enumerate()
        {
            work[i % threads].push((i, hdr, sc));
        }
        std::thread::scope(|s| {
            for list in work {
                let tracer = &tracer;
                s.spawn(move || {
                    for (chunk, hdr, sc) in list {
                        for k in 0..hdr.len() {
                            let x = k % w;
                            let y = row0 + chunk * ROWS + k / w;
                            let ray = camera.ray((x as f32 + jx) / w as f32, (y as f32 + jy) / h as f32);
                            let (c, dist) = tracer.trace_dist(&ray, 0, 1.0);
                            let tile = (y / HALO_TILE) * tiles_x + x / HALO_TILE;
                            let c = c + tracer.halos(&ray, dist, &all_glows, &halo_tiles[tile]);
                            hdr[k] = if c.x.is_finite() && c.y.is_finite() && c.z.is_finite() { c } else { Vec3::ZERO };
                            sc[k] = if tracer.q.god_rays {
                                // Ruido de gradiente intercalado: el filtro 3x3 de la fase 2 lo elimina.
                                let ign = fract(52.982_918 * fract(0.067_110_56 * x as f32 + 0.005_837_15 * y as f32));
                                tracer.god_rays(&ray, dist, fract(ign + pass as f32 * 0.618_034))
                            } else {
                                0.0
                            };
                        }
                    }
                });
            }
        });
    }

    // Fase 2: suavizar los haces (3x3), acumular muestras y aplicar tonemapping.
    let hdr = &target.hdr;
    let scatter = &target.scatter;
    let mut work: Vec<Vec<(usize, &mut [Vec3], &mut [u8])>> = (0..threads).map(|_| Vec::new()).collect();
    for (i, (acc, rgba)) in target.accum[span.clone()]
        .chunks_mut(w * ROWS)
        .zip(target.rgba[row0 * w * 4..rows.end * w * 4].chunks_mut(w * ROWS * 4))
        .enumerate()
    {
        work[i % threads].push((i, acc, rgba));
    }
    let aspect_fix = w as f32 / h as f32;
    let (ylo, yhi) = (rows.start, rows.end - 1);
    std::thread::scope(|s| {
        for list in work {
            s.spawn(move || {
                for (chunk, acc, rgba) in list {
                    for k in 0..acc.len() {
                        let x = k % w;
                        let y = row0 + chunk * ROWS + k / w;
                        let mut sum = 0.0;
                        for yy in [y.saturating_sub(1).max(ylo), y, (y + 1).min(yhi)] {
                            let row = yy * w;
                            sum += scatter[row + x.saturating_sub(1)] + scatter[row + x] + scatter[row + (x + 1).min(w - 1)];
                        }
                        let c = hdr[y * w + x] + shaft_color * (sum * (1.0 / 9.0));
                        let out = if accumulate {
                            acc[k] += c;
                            acc[k] * inv_samples
                        } else {
                            acc[k] = c;
                            c
                        };
                        let dx = ((x as f32 + 0.5) / w as f32 - 0.5) * 2.0;
                        let dy = ((y as f32 + 0.5) / h as f32 - 0.5) * 2.0 / aspect_fix * 1.6;
                        let vignette = 1.0 - 0.22 * (dx * dx + dy * dy);
                        rgba[k * 4..k * 4 + 4].copy_from_slice(&tone.map(out, vignette));
                    }
                }
            });
        }
    });
}

/// Guarda un buffer RGBA como BMP de 24 bits (sin librerías).
pub fn save_bmp(path: impl AsRef<Path>, rgba: &[u8], w: usize, h: usize) -> io::Result<()> {
    let row = (w * 3 + 3) & !3;
    let size = 54 + row * h;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        for x in 0..w {
            let i = (y * w + x) * 4;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i]]);
        }
        out.extend(std::iter::repeat_n(0u8, row - w * 3));
    }
    File::create(path)?.write_all(&out)
}
