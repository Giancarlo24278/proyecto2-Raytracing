//! Escena: primitivas + BVH, planos analíticos (fondo y superficie), burbujas
//! animadas y los datos de iluminación precalculados.

use crate::{
    bvh::{Aabb, Bvh},
    fauna::{Creature, Glow, Species, Swimmer},
    lighting::{AoVolume, ShadowMap},
    material::Library,
    math::{Ray, Vec3, fract},
    primitives::Prim,
    texture::{CubeMap, Gray},
};

/// Distancia a partir de la cual el agua ya es completamente opaca.
pub const FAR: f32 = 90.0;

/// Columna de burbujas que sale de un coral cerebro o de una grieta.
#[derive(Clone, Copy)]
pub struct Vent {
    pub base: Vec3,
    pub height: f32,
    pub count: usize,
    pub radius: f32,
    pub speed: f32,
    pub phase: f32,
}

#[derive(Clone, Copy)]
pub struct Bubble {
    pub center: Vec3,
    pub radius: f32,
}

/// Estado animado de un cuadro (posiciones de burbujas, tiempo).
pub struct Frame {
    pub time: f32,
    pub night: f32,
    pub bubbles: Vec<Bubble>,
    pub columns: Vec<(Aabb, usize, usize)>,
    pub creatures: Vec<Creature>,
    /// Luces que se mueven (ojos de los peepers).
    pub glows: Vec<Glow>,
}

#[derive(Clone, Copy, Debug)]
pub enum HitKind {
    Prim(u32),
    Floor,
    Water,
    Bubble(u32),
    /// Animal (índice en el cuadro, índice de la primitiva en su plantilla).
    Creature(u32, u32),
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub kind: HitKind,
}

pub struct Scene {
    pub prims: Vec<Prim>,
    pub bvh: Bvh,
    pub lib: Library,
    pub sky: CubeMap,
    pub haze: CubeMap,
    pub caustics: Gray,
    pub shafts: Gray,
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub water_y: f32,
    pub floor_extent: f32,
    pub vents: Vec<Vent>,
    pub shadow: ShadowMap,
    pub ao: AoVolume,
    pub focus: Vec3,
    pub species: Vec<Species>,
    pub swimmers: Vec<Swimmer>,
    /// Luces fijas (placas venosas, hongos ácidos) y su rejilla de búsqueda.
    pub glows: Vec<Glow>,
    pub glow_grid: GlowGrid,
}

/// Rejilla 2D (x, z) que guarda qué luces fijas alcanzan cada celda.
pub struct GlowGrid {
    min: f32,
    cell: f32,
    n: usize,
    cells: Vec<Vec<u16>>,
}

impl GlowGrid {
    pub fn build(glows: &[Glow], min: f32, max: f32, cell: f32) -> Self {
        let n = ((max - min) / cell).ceil() as usize;
        let mut cells = vec![Vec::new(); n * n];
        for (i, g) in glows.iter().enumerate() {
            let lo_x = (((g.pos.x - g.radius - min) / cell).floor().max(0.0) as usize).min(n - 1);
            let hi_x = (((g.pos.x + g.radius - min) / cell).floor().max(0.0) as usize).min(n - 1);
            let lo_z = (((g.pos.z - g.radius - min) / cell).floor().max(0.0) as usize).min(n - 1);
            let hi_z = (((g.pos.z + g.radius - min) / cell).floor().max(0.0) as usize).min(n - 1);
            for z in lo_z..=hi_z {
                for x in lo_x..=hi_x {
                    cells[z * n + x].push(i as u16);
                }
            }
        }
        Self { min, cell, n, cells }
    }

    #[inline]
    pub fn near(&self, p: Vec3) -> &[u16] {
        let x = ((p.x - self.min) / self.cell).floor();
        let z = ((p.z - self.min) / self.cell).floor();
        if x < 0.0 || z < 0.0 || x >= self.n as f32 || z >= self.n as f32 {
            return &[];
        }
        &self.cells[z as usize * self.n + x as usize]
    }
}

impl Scene {
    /// `night`: 0 = día, 1 = noche.
    /// `event`: segundos desde que empezó el evento del leviatán (None = no hay evento).
    pub fn frame(&self, time: f32, night: f32, event: Option<f32>) -> Frame {
        let mut bubbles = Vec::new();
        let mut columns = Vec::new();
        for (vi, v) in self.vents.iter().enumerate() {
            let start = bubbles.len();
            let mut bounds = Aabb::EMPTY;
            for i in 0..v.count {
                let fi = i as f32;
                let s = fract(time * v.speed / v.height + fi / v.count as f32 + v.phase);
                let wobble = (time * 1.3 + fi * 1.7 + vi as f32).sin() * 0.25 * s;
                let wobble2 = (time * 1.1 + fi * 2.9 + vi as f32 * 0.5).cos() * 0.25 * s;
                let grow = 0.55 + 0.75 * s;
                let pop = 1.0 - ((s - 0.92) / 0.08).clamp(0.0, 1.0);
                let r = v.radius * grow * (0.75 + 0.5 * fract(fi * 0.618)) * pop;
                if r < 0.005 {
                    continue;
                }
                let c = v.base + Vec3::new(wobble, s * v.height, wobble2);
                bounds.grow(&Aabb {
                    min: c - Vec3::splat(r),
                    max: c + Vec3::splat(r),
                });
                bubbles.push(Bubble { center: c, radius: r });
            }
            if bubbles.len() > start {
                columns.push((bounds, start, bubbles.len()));
            }
        }
        let mut creatures: Vec<Creature> = self.swimmers.iter().map(|sw| sw.place(&self.species, time)).collect();
        if let Some(c) = event.and_then(crate::fauna::leviathan_pose) {
            creatures.push(c);
        }
        let mut glows = Vec::new();
        for c in &creatures {
            for (lp, color, radius, halo) in &self.species[c.species as usize].lights {
                glows.push(Glow {
                    pos: c.pos + c.rot.mul(*lp),
                    color: *color,
                    radius: *radius,
                    halo: *halo,
                });
            }
        }
        Frame {
            time,
            night,
            bubbles,
            columns,
            creatures,
            glows,
        }
    }

    /// Impacto más cercano contra todo lo que hay en la escena.
    pub fn intersect(&self, ray: &Ray, tmin: f32, tmax: f32, frame: &Frame) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        let mut tmax = tmax;

        // Fondo de arena (plano y = 0 limitado).
        if ray.dir.y < -1e-6 && ray.origin.y > 0.0 {
            let t = -ray.origin.y / ray.dir.y;
            if t > tmin && t < tmax {
                let p = ray.at(t);
                if p.x.abs() < self.floor_extent && p.z.abs() < self.floor_extent {
                    tmax = t;
                    best = Some(Hit { t, kind: HitKind::Floor });
                }
            }
        }
        // Superficie del agua vista desde abajo.
        if ray.dir.y > 1e-6 && ray.origin.y < self.water_y {
            let t = (self.water_y - ray.origin.y) / ray.dir.y;
            if t > tmin && t < tmax {
                tmax = t;
                best = Some(Hit { t, kind: HitKind::Water });
            }
        }
        if let Some((prim, t)) = self.bvh.closest(ray, tmin, tmax, |i, tm| {
            self.prims[i as usize].intersect(ray, tmin, tm, frame.time)
        }) {
            tmax = t;
            best = Some(Hit {
                t,
                kind: HitKind::Prim(prim),
            });
        }
        // Animales: esfera envolvente y luego el rayo en su espacio local.
        for (ci, c) in frame.creatures.iter().enumerate() {
            let sp = &self.species[c.species as usize];
            let oc = ray.origin - c.pos;
            let b = oc.dot(ray.dir);
            let disc = b * b - (oc.dot(oc) - sp.radius * sp.radius);
            if disc < 0.0 || -b - disc.sqrt() > tmax || -b + disc.sqrt() < tmin {
                continue;
            }
            let local = Ray::new(c.rot.tmul(ray.origin - c.pos), c.rot.tmul(ray.dir));
            for (pi, prim) in sp.prims.iter().enumerate() {
                if let Some(t) = prim.intersect(&local, tmin, tmax, c.time) {
                    tmax = t;
                    best = Some(Hit {
                        t,
                        kind: HitKind::Creature(ci as u32, pi as u32),
                    });
                }
            }
        }
        // Burbujas animadas (fuera del BVH porque se mueven).
        let inv = Vec3::new(1.0 / ray.dir.x, 1.0 / ray.dir.y, 1.0 / ray.dir.z);
        for (bounds, a, b) in &frame.columns {
            if bounds.hit(ray.origin, inv, tmin, tmax).is_infinite() {
                continue;
            }
            for i in *a..*b {
                let bubble = &frame.bubbles[i];
                if let Some(t) = sphere_hit(ray, bubble.center, bubble.radius, tmin, tmax) {
                    tmax = t;
                    best = Some(Hit {
                        t,
                        kind: HitKind::Bubble(i as u32),
                    });
                }
            }
        }
        best
    }

    /// Columnas de burbujas que soltaron una burbuja nueva entre `t0` y `t1`.
    pub fn bubble_spawns(&self, t0: f32, t1: f32) -> Vec<Vec3> {
        let mut out = Vec::new();
        if t1 <= t0 {
            return out;
        }
        for v in &self.vents {
            let k = |t: f32| ((t * v.speed / v.height + v.phase) * v.count as f32).floor();
            if k(t1) > k(t0) {
                out.push(v.base);
            }
        }
        out
    }

    /// ¿Hay algo opaco entre el origen del rayo y `tmax`? (Solo geometría estática.)
    pub fn occluded(&self, ray: &Ray, tmax: f32) -> bool {
        self.bvh.any(ray, 1e-3, tmax, |i| {
            let p = &self.prims[i as usize];
            self.lib.materials[p.material as usize].casts_shadow
                && p.intersect(ray, 1e-3, tmax, 0.0).is_some()
        })
    }

    /// ¿El punto está dentro (o muy cerca) de una roca u otro objeto sólido?
    pub fn inside_solid(&self, p: Vec3, margin: f32) -> bool {
        self.bvh.any_at(p, |i| {
            let prim = &self.prims[i as usize];
            prim.cutout == crate::primitives::Cutout::None
                && self.lib.materials[prim.material as usize].casts_shadow
                && prim.contains(p, margin)
        })
    }

    /// Distancia al primer objeto opaco (para el mapa de sombras y la cámara).
    pub fn first_opaque(&self, ray: &Ray, tmax: f32) -> Option<f32> {
        self.bvh
            .closest(ray, 1e-3, tmax, |i, tm| {
                let p = &self.prims[i as usize];
                if self.lib.materials[p.material as usize].casts_shadow {
                    p.intersect(ray, 1e-3, tm, 0.0)
                } else {
                    None
                }
            })
            .map(|(_, t)| t)
    }
}

#[inline(always)]
pub fn sphere_hit(ray: &Ray, c: Vec3, r: f32, tmin: f32, tmax: f32) -> Option<f32> {
    let oc = ray.origin - c;
    let b = oc.dot(ray.dir);
    let cc = oc.dot(oc) - r * r;
    let disc = b * b - cc;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t0 = -b - s;
    if t0 > tmin && t0 < tmax {
        return Some(t0);
    }
    let t1 = -b + s;
    if t1 > tmin && t1 < tmax {
        return Some(t1);
    }
    None
}
