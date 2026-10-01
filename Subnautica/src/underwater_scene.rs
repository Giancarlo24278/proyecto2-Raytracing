//! Composición del arrecife (inspirado en las zonas poco profundas de Subnautica).
//!
//! Todo se arma con figuras básicas: cubos rotados (rocas, cristales, cápsula),
//! cilindros (esponjas, placas venosas, tallos), elipsoides (corales de mesa y
//! cerebro, hongos ácidos, dunas) y planos (fondo y superficie). Las plantas se colocan lanzando rayos
//! hacia abajo para que queden apoyadas sobre la roca o la arena.

use crate::{
    bvh::Bvh,
    fauna::{self, Glow},
    lighting::{AoVolume, ShadowMap},
    material::{
        self, BRAIN, BRANCH, CRYSTAL, FAN, GRASS, METAL, MUSHROOM, RED_GRASS, RED_PLANT, ROCK, SAND, SHELL_PLATE, SPONGE,
        TABLE_CORAL,
    },
    math::{Mat3, Ray, Vec3, lerp, smoothstep},
    noise::voronoi2,
    primitives::{Cutout, Prim, Shape},
    rng::Rng,
    scene::{GlowGrid, Scene, Vent},
    skybox,
    texture::Gray,
};
use std::f32::consts::{PI, TAU};

const WATER_Y: f32 = 10.0;

/// BVH temporal para "dejar caer" objetos sobre lo que ya está construido.
struct Placer {
    prims: Vec<Prim>,
    bvh: Bvh,
}

impl Placer {
    fn new(prims: &[Prim]) -> Self {
        let boxes: Vec<_> = prims.iter().map(|p| p.aabb()).collect();
        Self {
            prims: prims.to_vec(),
            bvh: Bvh::build(&boxes),
        }
    }

    fn cast(&self, origin: Vec3, dir: Vec3, tmax: f32) -> Option<(Vec3, Vec3, u16)> {
        let ray = Ray::new(origin, dir);
        self.bvh
            .closest(&ray, 1e-3, tmax, |i, tm| self.prims[i as usize].intersect(&ray, 1e-3, tm, 0.0))
            .map(|(i, t)| {
                let prim = &self.prims[i as usize];
                let mut n = prim.surface(&ray, t, 0.0).normal;
                if n.dot(dir) > 0.0 {
                    n = -n;
                }
                (ray.at(t), n, prim.material)
            })
    }

    /// Superficie debajo de (x, z): devuelve punto, normal y material (None = arena del fondo).
    fn drop(&self, x: f32, z: f32) -> (Vec3, Vec3, Option<u16>) {
        match self.cast(Vec3::new(x, 14.0, z), Vec3::new(0.0, -1.0, 0.0), 14.0) {
            Some((p, n, m)) => (p, n, Some(m)),
            None => (Vec3::new(x, 0.0, z), Vec3::UP, None),
        }
    }

    fn is_sand(m: Option<u16>) -> bool {
        matches!(m, None | Some(SAND))
    }
}

struct Builder {
    prims: Vec<Prim>,
    rng: Rng,
    vents: Vec<Vent>,
    glows: Vec<Glow>,
}

impl Builder {
    fn push(&mut self, p: Prim) {
        self.prims.push(p);
    }

    fn rand_rot(&mut self, tilt: f32) -> Mat3 {
        let yaw = self.rng.range(-PI, PI);
        let pitch = self.rng.signed() * tilt;
        let roll = self.rng.signed() * tilt;
        Mat3::from_euler(yaw, pitch, roll)
    }

    fn rock_tint(&mut self, warm: f32) -> Vec3 {
        let t = self.rng.tint(0.07);
        t * Vec3::new(0.96, 1.0, 1.02).lerp(Vec3::new(1.08, 1.0, 0.92), warm)
    }

    fn rock_tint_rand(&mut self) -> Vec3 {
        let warm = self.rng.f();
        self.rock_tint(warm)
    }

    fn rock(&mut self, c: Vec3, half: Vec3, rot: Mat3, tint: Vec3) {
        self.push(Prim::new(Shape::Cube, c, half, rot, ROCK).with_color(tint));
    }

    /// Piedra pequeña sin musgo (param[1] = cantidad de musgo).
    fn pebble(&mut self, c: Vec3, half: Vec3, rot: Mat3, tint: Vec3, round: bool) {
        let shape = if round { Shape::Sphere } else { Shape::Cube };
        let mut p = Prim::new(shape, c, half, rot, ROCK).with_color(tint);
        p.param = [0.0, 0.0];
        self.push(p);
    }

    // -----------------------------------------------------------------------
    // Rocas
    // -----------------------------------------------------------------------

    /// Arco principal: bloques rotados siguiendo una curva elíptica.
    fn arch(&mut self) -> Vec<(Vec3, Vec3, Vec3, f32, f32)> {
        let mut stations = Vec::new();
        let n = 30;
        let (cx, rx, ry) = (-0.1, 4.5, 5.3);
        for i in 0..=n {
            let s = i as f32 / n as f32;
            let th = PI * (1.0 - s);
            let x = cx + rx * th.cos();
            let y = ry * th.sin() * (1.0 - 0.05 * s) + 0.15;
            let z = 0.35 * (th * 2.0).sin();
            let top = th.sin();
            let r = lerp(1.75, 1.10, top);
            let dz = lerp(1.95, 1.30, top);
            let tangent = Vec3::new(rx * th.sin(), -ry * th.cos(), 0.0).normalized();
            let out = Vec3::new(ry * th.cos(), rx * th.sin(), 0.0).normalized();
            let base = Mat3 {
                c0: tangent,
                c1: out,
                c2: tangent.cross(out).normalized(),
            };
            let center = Vec3::new(x, y, z);
            stations.push((center, tangent, out, r, dz));
            let jitter = Mat3::from_euler(self.rng.signed() * 0.2, self.rng.signed() * 0.2, self.rng.signed() * 0.2);
            let tint = self.rock_tint(0.3 + 0.4 * top);
            let half = Vec3::new(
                0.45 + self.rng.range(0.0, 0.3),
                r * self.rng.range(0.70, 0.92),
                dz * self.rng.range(0.80, 1.0),
            );
            self.rock(center, half, base.mul_mat(jitter), tint);
            for k in 0..3 {
                let off = out * (r * self.rng.range(-0.6, 0.6))
                    + Vec3::new(0.0, 0.0, dz * self.rng.range(-0.7, 0.7))
                    + tangent * self.rng.range(-0.3, 0.3);
                // Bloques "gruesos" (proporciones parecidas) para que no parezcan astillas.
                let size = r * self.rng.range(0.45, 0.75);
                let half = Vec3::new(
                    size * self.rng.range(0.85, 1.25),
                    size * self.rng.range(0.6, 0.95),
                    size * self.rng.range(0.85, 1.25),
                );
                let jitter = Mat3::from_euler(self.rng.signed() * 0.5, self.rng.signed() * 0.3, self.rng.signed() * 0.3);
                let rot = base.mul_mat(jitter);
                let tint = self.rock_tint_rand();
                if k == 2 && self.rng.chance(0.6) {
                    // Bultos aplastados que suavizan la silueta sin volverla redonda.
                    let lump = Vec3::new(half.x * 1.3, half.y * 0.75, half.z * 1.15);
                    self.push(Prim::new(Shape::Sphere, center + off, lump, rot, ROCK).with_color(tint));
                } else {
                    self.rock(center + off, half, rot, tint);
                }
            }
        }
        // Bases macizas del arco, parcialmente enterradas.
        for (fx, side) in [(cx - rx, -1.0f32), (cx + rx, 1.0)] {
            for _ in 0..7 {
                let c = Vec3::new(
                    fx + side * self.rng.range(-0.8, 1.6),
                    self.rng.range(0.1, 1.3),
                    self.rng.range(-2.2, 2.2),
                );
                let half = Vec3::new(self.rng.range(0.7, 1.5), self.rng.range(0.5, 1.2), self.rng.range(0.7, 1.4));
                let rot = self.rand_rot(0.45);
                let tint = self.rock_tint(0.2);
                self.rock(c, half, rot, tint);
            }
        }
        stations
    }

    /// Acantilado estratificado: capas de bloques casi horizontales.
    fn layered_cliff(&mut self, path: &[Vec3], layers: usize, layer_h: f32, width: f32) {
        let segs = path.len() - 1;
        for layer in 0..layers {
            let lf = layer as f32 / layers as f32;
            let y = layer as f32 * layer_h + layer_h * 0.45;
            let shrink = 1.0 - lf * 0.55;
            for s in 0..segs {
                let steps = 3;
                for k in 0..steps {
                    let t = (k as f32 + self.rng.f() * 0.6) / steps as f32;
                    let a = path[s];
                    let b = path[s + 1];
                    // Las capas altas solo cubren la parte central del acantilado.
                    let along = (s as f32 + t) / segs as f32;
                    if (along - 0.5).abs() > 0.5 * shrink + 0.08 {
                        continue;
                    }
                    if layer > 0 && self.rng.chance(0.18 + lf * 0.2) {
                        continue;
                    }
                    let p = a.lerp(b, t);
                    let dir = (b - a).normalized();
                    let side = Vec3::new(-dir.z, 0.0, dir.x);
                    let c = p + side * (self.rng.signed() * width * 0.25 * shrink)
                        + Vec3::new(0.0, y + self.rng.signed() * 0.15, 0.0);
                    let half = Vec3::new(
                        self.rng.range(1.0, 1.9),
                        layer_h * self.rng.range(0.45, 0.62),
                        width * shrink * self.rng.range(0.35, 0.55),
                    );
                    let yaw = dir.x.atan2(dir.z) + PI * 0.5 + self.rng.signed() * 0.35;
                    let rot = Mat3::from_euler(yaw, self.rng.signed() * 0.10, self.rng.signed() * 0.10);
                    let tint = self.rock_tint(0.5 + lf * 0.4);
                    self.rock(c, half, rot, tint);
                }
            }
        }
    }

    /// Aguja rocosa: bloques apilados que se afinan hacia arriba.
    fn spire(&mut self, base: Vec3, height: f32, radius: f32) {
        let mut y = 0.0;
        let mut r = radius;
        let mut offset = Vec3::ZERO;
        while y < height {
            let h = r * self.rng.range(0.7, 1.1);
            let half = Vec3::new(r * self.rng.range(0.8, 1.1), h * 0.5 + 0.1, r * self.rng.range(0.8, 1.1));
            let c = base + offset + Vec3::new(0.0, y + h * 0.5, 0.0);
            let rot = self.rand_rot(0.18);
            let tint = self.rock_tint(0.3);
            self.rock(c, half, rot, tint);
            y += h * 0.85;
            r *= self.rng.range(0.78, 0.92);
            offset += Vec3::new(self.rng.signed() * 0.25, 0.0, self.rng.signed() * 0.25);
        }
    }

    /// Bloque grande e irregular (varios cubos) para formaciones y rocas sueltas.
    fn boulder(&mut self, c: Vec3, size: f32) {
        let tint = self.rock_tint_rand();
        for k in 0..4 {
            let off = if k == 0 {
                Vec3::ZERO
            } else {
                Vec3::new(self.rng.signed(), self.rng.range(-0.3, 0.4), self.rng.signed()) * (size * 0.55)
            };
            let half = Vec3::new(
                size * self.rng.range(0.45, 0.8),
                size * self.rng.range(0.35, 0.6),
                size * self.rng.range(0.45, 0.8),
            );
            let rot = self.rand_rot(0.5);
            let t = tint * self.rng.tint(0.04);
            self.rock(c + off, half, rot, t);
        }
    }

    /// Pequeños salientes rocosos sobre una superficie existente (rugosidad).
    fn rubble(&mut self, placer: &Placer, centers: &[(Vec3, f32)], count: usize) {
        for _ in 0..count {
            let (c, reach) = centers[self.rng.index(centers.len())];
            let dir = Vec3::new(self.rng.signed(), self.rng.signed() * 0.8, self.rng.signed()).normalized();
            let origin = c + dir * (reach + 2.0);
            if let Some((p, n, ROCK)) = placer.cast(origin, -dir, reach + 3.0) {
                // Lajas aplanadas apoyadas sobre la superficie (no púas).
                let s = self.rng.range(0.18, 0.45);
                let half = Vec3::new(s, s * self.rng.range(0.4, 0.7), s * self.rng.range(0.7, 1.0));
                let tilt = (n + Vec3::new(self.rng.signed(), self.rng.signed(), self.rng.signed()) * 0.25).normalized();
                let rot = Mat3::from_y_axis(tilt, self.rng.range(0.0, TAU));
                let tint = self.rock_tint_rand();
                self.rock(p - n * (half.y * 0.4), half, rot, tint);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Vida: corales, esponjas, plantas
    // -----------------------------------------------------------------------

    fn coral_tint(&mut self) -> Vec3 {
        // Colores del coral de mesa del juego: verde, azul, morado y rojo.
        let palettes = [
            Vec3::new(0.38, 0.85, 0.30),
            Vec3::new(0.25, 0.48, 0.95),
            Vec3::new(0.58, 0.32, 0.88),
            Vec3::new(0.90, 0.28, 0.22),
            Vec3::new(0.95, 0.55, 0.20),
        ];
        palettes[self.rng.index(palettes.len())] * self.rng.tint(0.06)
    }

    /// Coral de mesa (como en Subnautica): "almohadas" redondeadas y escalonadas
    /// que salen de la pared, cada grupo de un color.
    fn table_coral(&mut self, p: Vec3, outward: Vec3, scale: f32, tint: Vec3) {
        let out = Vec3::new(outward.x, 0.0, outward.z).normalized();
        let side = Vec3::new(-out.z, 0.0, out.x);
        let tiers = 2 + self.rng.index(3);
        for k in 0..tiers {
            let kf = k as f32;
            let r = scale * self.rng.range(0.45, 0.7) * (1.0 - kf * 0.15);
            let c = p
                + out * (r * 0.55)
                + side * ((kf - tiers as f32 * 0.5) * r * 0.55 + self.rng.signed() * 0.06)
                + Vec3::new(0.0, kf * scale * 0.11 - 0.05, 0.0);
            let half = Vec3::new(r * 0.75, r * 0.17, r);
            let rot = Mat3 { c0: out, c1: Vec3::UP, c2: side }
                .mul_mat(Mat3::from_euler(self.rng.signed() * 0.3, self.rng.signed() * 0.15, -0.15));
            let t = tint * self.rng.tint(0.05);
            self.push(Prim::new(Shape::Sphere, c, half, rot, TABLE_CORAL).with_color(t));
        }
    }

    /// Placa venosa: disco ovalado casi vertical con venas amarillas que brillan.
    fn shell_plate(&mut self, p: Vec3, outward: Vec3, scale: f32) {
        let out = Vec3::new(outward.x, 0.0, outward.z).normalized();
        let tilt = (out + Vec3::UP * self.rng.range(0.15, 0.6)).normalized();
        let rot = Mat3::from_y_axis(tilt, 0.0);
        // Orientar el eje largo del óvalo hacia arriba.
        let up_local = rot.tmul(Vec3::UP);
        let spin = up_local.x.atan2(up_local.z);
        let rot = Mat3::from_y_axis(tilt, -spin);
        let r = scale * self.rng.range(0.28, 0.42);
        let c = p + out * 0.05 + Vec3::new(0.0, r * 0.6, 0.0);
        self.push(Prim::new(Shape::Cylinder, c, Vec3::new(r * 0.8, 0.025, r), rot, SHELL_PLATE));
        self.glows.push(Glow {
            pos: c + out * 0.15,
            color: Vec3::new(1.0, 0.78, 0.28) * 0.45,
            radius: 1.6,
            halo: 0.22,
        });
    }

    /// Hongos ácidos (como en el juego): copas moradas con un hueco arriba que
    /// brilla en naranja/rosado, sobre un tallo corto.
    fn mushrooms(&mut self, base: Vec3, scale: f32) {
        let n = 4 + self.rng.index(4);
        for i in 0..n {
            let a = self.rng.range(0.0, TAU);
            let d = if i == 0 { 0.0 } else { self.rng.range(0.25, 0.6) * scale };
            let r = scale * if i == 0 { 0.30 } else { self.rng.range(0.13, 0.22) };
            let pos = base + Vec3::new(a.cos() * d, 0.0, a.sin() * d);
            let stem_h = r * self.rng.range(0.9, 1.4);
            let lean = Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * 0.15;
            let axis = (Vec3::UP + lean).normalized();
            // Tallo que se ensancha hacia arriba (dos cilindros).
            let stem_rot = Mat3::from_y_axis(axis, 0.0);
            self.push(
                Prim::new(Shape::Cylinder, pos + axis * (stem_h * 0.35), Vec3::new(r * 0.28, stem_h * 0.35, r * 0.28), stem_rot, TABLE_CORAL)
                    .with_color(Vec3::new(0.38, 0.48, 0.95)),
            );
            self.push(
                Prim::new(Shape::Cylinder, pos + axis * (stem_h * 0.75), Vec3::new(r * 0.42, stem_h * 0.12, r * 0.42), stem_rot, TABLE_CORAL)
                    .with_color(Vec3::new(0.38, 0.48, 0.95)),
            );
            // Copa: elipsoide achatado; el hueco de arriba está en la textura (polo superior).
            let tint = self.rng.tint(0.06);
            let spin = self.rng.range(0.0, TAU);
            self.push(
                Prim::new(Shape::Sphere, pos + axis * (stem_h + r * 0.35), Vec3::new(r, r * 0.62, r), Mat3::from_y_axis(axis, spin), MUSHROOM)
                    .with_color(tint),
            );
        }
        self.glows.push(Glow {
            pos: base + Vec3::new(0.0, 0.55 * scale, 0.0),
            color: Vec3::new(1.0, 0.45, 0.65) * 0.35,
            radius: 1.8,
            halo: 0.35 * scale,
        });
    }

    /// Mancha de pasto rojo (como en el bioma "Grassy Plateaus"): muchas hojitas finas.
    fn red_grass_patch(&mut self, placer: &Placer, center: Vec3, radius: f32, tufts: usize) {
        for _ in 0..tufts {
            let a = self.rng.range(0.0, TAU);
            let d = radius * self.rng.f().sqrt();
            let (p, n, m) = placer.drop(center.x + a.cos() * d, center.z + a.sin() * d);
            if !(Placer::is_sand(m) || n.y > 0.5) {
                continue;
            }
            let s = self.rng.range(0.55, 0.9);
            let blades = 8 + self.rng.index(5);
            self.tuft(p, s, RED_GRASS, blades, 0.03, 0.55);
        }
    }

    /// Grupo de esponjas tubo amarillas.
    fn sponges(&mut self, base: Vec3, scale: f32) {
        let n = 3 + self.rng.index(5);
        let tint = self.rng.tint(0.08) * if self.rng.chance(0.25) { Vec3::new(1.0, 0.8, 0.7) } else { Vec3::ONE };
        for _ in 0..n {
            let off = Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * (0.35 * scale);
            let r = scale * self.rng.range(0.11, 0.26);
            let h = scale * self.rng.range(0.45, 1.5);
            let lean = Vec3::new(off.x, 0.0, off.z) * 0.6 + Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * 0.12;
            let axis = (Vec3::UP + lean).normalized();
            let c = base + off + axis * (h * 0.5 - 0.08);
            let rot = Mat3::from_y_axis(axis, self.rng.range(0.0, TAU));
            self.push(Prim::new(Shape::Cylinder, c, Vec3::new(r, h * 0.5, r), rot, SPONGE).with_color(tint));
        }
    }

    /// Coral cerebro semienterrado (opcionalmente con una columna de burbujas).
    fn brain(&mut self, base: Vec3, r: f32, bubbles: bool) {
        let tint = self.rng.tint(0.06);
        let half = Vec3::new(r, r * self.rng.range(0.62, 0.78), r * self.rng.range(0.85, 1.0));
        let rot = Mat3::from_euler(self.rng.range(0.0, TAU), self.rng.signed() * 0.08, self.rng.signed() * 0.08);
        let c = base + Vec3::new(0.0, -half.y * 0.15, 0.0);
        self.push(Prim::new(Shape::Sphere, c, half, rot, BRAIN).with_color(tint));
        if bubbles {
            // Una burbuja grande (casi la mitad del coral) cada ~10 segundos.
            let top = c + Vec3::new(0.0, half.y * 0.9, 0.0);
            let height = (WATER_Y - top.y - 0.8).max(2.0);
            let period = self.rng.range(9.0, 11.0);
            self.vents.push(Vent {
                base: top,
                height,
                count: 1,
                radius: r * 0.32,
                speed: height / period,
                phase: self.rng.f(),
            });
        }
    }

    /// Coral ramificado recursivo (cilindros + esferas en las uniones).
    fn branch(&mut self, start: Vec3, dir: Vec3, len: f32, radius: f32, depth: u32, tint: Vec3) {
        let end = start + dir * len;
        let rot = Mat3::from_y_axis(dir, 0.0);
        self.push(
            Prim::new(Shape::Cylinder, (start + end) * 0.5, Vec3::new(radius, len * 0.5, radius), rot, BRANCH)
                .with_color(tint),
        );
        if depth == 0 {
            let tip = Vec3::splat(radius * 1.45);
            self.push(Prim::new(Shape::Sphere, end, tip, Mat3::IDENTITY, BRANCH).with_color(tint * 1.25));
            return;
        }
        self.push(Prim::new(Shape::Sphere, end, Vec3::splat(radius * 1.02), Mat3::IDENTITY, BRANCH).with_color(tint));
        let children = 2 + self.rng.index(2);
        for _ in 0..children {
            let d = (dir * 0.9
                + Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * 0.75
                + Vec3::UP * 0.35)
                .normalized();
            let l = len * self.rng.range(0.65, 0.85);
            self.branch(end, d, l, radius * 0.74, depth - 1, tint);
        }
    }

    fn branch_coral(&mut self, base: Vec3, scale: f32) {
        let tints = [
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.75, 0.75, 1.35),
            Vec3::new(1.15, 1.05, 0.55),
            Vec3::new(0.65, 1.0, 1.25),
        ];
        let tint = tints[self.rng.index(tints.len())] * self.rng.tint(0.05);
        let trunks = 1 + self.rng.index(3);
        for _ in 0..trunks {
            let dir = (Vec3::UP + Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * 0.35).normalized();
            let off = Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * (0.15 * scale);
            self.branch(base + off - Vec3::new(0.0, 0.05, 0.0), dir, 0.38 * scale, 0.075 * scale, 3, tint);
        }
    }

    /// Abanico de mar: plano delgado con recorte en forma de red.
    fn sea_fan(&mut self, base: Vec3, height: f32) {
        let w = height * self.rng.range(0.55, 0.8);
        let rot = Mat3::from_euler(self.rng.range(0.0, TAU), self.rng.signed() * 0.15, self.rng.signed() * 0.12);
        let c = base + rot.c1 * (height * 0.5 - 0.03);
        let tint = if self.rng.chance(0.35) { Vec3::new(1.5, 0.7, 0.45) } else { self.rng.tint(0.08) * 1.15 };
        self.push(
            Prim::new(Shape::Cube, c, Vec3::new(w * 0.5, height * 0.5, 0.012), rot, FAN)
                .with_color(tint)
                .with_cutout(Cutout::Fan, [0.0, 1.0])
                .with_sway(Vec3::new(0.8, 0.0, 0.6), height * 0.06, base.x * 0.35 + base.z * 0.25),
        );
    }

    /// Mata de hojas curvas: cada hoja son 3 segmentos con recorte en forma de hoja.
    fn tuft(&mut self, base: Vec3, scale: f32, material: u16, blades: usize, width: f32, spread: f32) {
        let tint = self.rng.tint(0.10);
        // Todas las hojas de la mata se mecen con la misma corriente (fase parecida).
        let current = Vec3::new(0.8, 0.0, 0.6);
        let tuft_phase = base.x * 0.35 + base.z * 0.25 + self.rng.f() * 0.6;
        for _ in 0..blades {
            let az = self.rng.range(0.0, TAU);
            let lean_dir = Vec3::new(az.cos(), 0.0, az.sin());
            let width_axis = Vec3::new(-az.sin(), 0.0, az.cos());
            let length = scale * self.rng.range(0.55, 1.0);
            let half_w = width * self.rng.range(0.8, 1.2);
            let mut angle = self.rng.range(0.05, spread);
            let bend = self.rng.range(0.15, 0.45);
            let mut p = base
                + Vec3::new(self.rng.signed(), 0.0, self.rng.signed()) * (0.06 * scale)
                - Vec3::new(0.0, 0.03, 0.0);
            let segs = 3;
            let seg_len = length / segs as f32;
            for k in 0..segs {
                let dir = (Vec3::UP * angle.cos() + lean_dir * angle.sin()).normalized();
                let z = width_axis.cross(dir).normalized();
                let rot = Mat3 {
                    c0: width_axis,
                    c1: dir,
                    c2: z,
                };
                let c = p + dir * (seg_len * 0.5);
                let v0 = k as f32 / segs as f32;
                let v1 = (k + 1) as f32 / segs as f32;
                let phase = tuft_phase + az * 0.15;
                self.push(
                    Prim::new(Shape::Cube, c, Vec3::new(half_w, seg_len * 0.52, 0.006), rot, material)
                        .with_color(tint)
                        .with_cutout(Cutout::Leaf, [v0, v1])
                        .with_sway(current, length * 0.16, phase),
                );
                p += dir * seg_len;
                angle += bend;
            }
        }
    }

    fn grass(&mut self, base: Vec3, scale: f32) {
        let blades = 5 + self.rng.index(5);
        self.tuft(base, scale, GRASS, blades, 0.032, 0.45);
    }

    fn red_plant(&mut self, base: Vec3, scale: f32) {
        let blades = 5 + self.rng.index(4);
        self.tuft(base, scale * 1.4, RED_PLANT, blades, 0.10, 0.65);
    }

    /// Cúmulo de cristales de cuarzo (prismas alargados que salen de la arena).
    fn crystals(&mut self, base: Vec3, scale: f32) {
        let n = 5 + self.rng.index(4);
        let tint = if self.rng.chance(0.5) { Vec3::new(1.0, 1.0, 1.0) } else { Vec3::new(1.15, 0.85, 1.1) };
        for i in 0..n {
            let main = i == 0;
            let h = scale * if main { 1.5 } else { self.rng.range(0.5, 1.1) };
            let r = scale * if main { 0.17 } else { self.rng.range(0.07, 0.13) };
            let lean = if main { 0.08 } else { self.rng.range(0.25, 0.7) };
            let az = self.rng.range(0.0, TAU);
            let axis = (Vec3::UP * lean.cos() + Vec3::new(az.cos(), 0.0, az.sin()) * lean.sin()).normalized();
            let c = base + axis * (h * 0.5 - 0.12 * scale);
            let rot = Mat3::from_y_axis(axis, self.rng.range(0.0, TAU));
            self.push(Prim::new(Shape::Cube, c, Vec3::new(r, h * 0.5, r * 0.8), rot, CRYSTAL).with_color(tint));
        }
    }

    /// Cápsula de suministros metálica, semienterrada y con la tapa entreabierta.
    fn supply_crate(&mut self, base: Vec3, yaw: f32) {
        let rot = Mat3::from_euler(yaw, 0.06, 0.16);
        let half = Vec3::new(0.80, 0.45, 0.52);
        let c = base + Vec3::new(0.0, 0.25, 0.0);
        self.push(Prim::new(Shape::Cube, c, half, rot, METAL));
        // Tapa girada sobre una bisagra.
        let hinge = c + rot.mul(Vec3::new(0.0, half.y, -half.z));
        let lid_rot = rot.mul_mat(Mat3::from_euler(0.0, -0.45, 0.0));
        let lid_c = hinge + lid_rot.mul(Vec3::new(0.0, 0.05, half.z));
        self.push(Prim::new(Shape::Cube, lid_c, Vec3::new(half.x + 0.03, 0.05, half.z + 0.02), lid_rot, METAL));
        // Asas laterales.
        for side in [-1.0f32, 1.0] {
            let hc = c + rot.mul(Vec3::new(side * (half.x + 0.04), 0.05, 0.0));
            self.push(
                Prim::new(Shape::Cube, hc, Vec3::new(0.04, 0.06, 0.22), rot, METAL)
                    .with_color(Vec3::splat(0.7)),
            );
        }
    }
}

pub fn build() -> Scene {
    let mut b = Builder {
        prims: Vec::new(),
        rng: Rng::new(20_260_930),
        vents: Vec::new(),
        glows: Vec::new(),
    };

    // --- Dunas suaves de arena -------------------------------------------------
    for (c, r) in [
        (Vec3::new(-7.0, -0.55, 5.5), Vec3::new(6.0, 0.95, 4.0)),
        (Vec3::new(7.5, -0.6, 6.5), Vec3::new(5.0, 0.9, 4.5)),
        (Vec3::new(2.0, -0.7, -9.0), Vec3::new(7.0, 1.0, 4.0)),
        (Vec3::new(-9.0, -0.7, -8.5), Vec3::new(5.0, 1.0, 5.0)),
        (Vec3::new(12.0, -0.8, -4.0), Vec3::new(5.0, 1.1, 6.0)),
    ] {
        let rot = Mat3::from_euler(b.rng.range(0.0, TAU), 0.0, 0.0);
        b.push(Prim::new(Shape::Sphere, c, r, rot, SAND));
    }

    // --- Rocas ------------------------------------------------------------------
    let stations = b.arch();
    // Acantilado estratificado a la izquierda.
    b.layered_cliff(
        &[
            Vec3::new(-13.5, 0.0, -9.0),
            Vec3::new(-15.0, 0.0, -3.5),
            Vec3::new(-14.5, 0.0, 2.0),
            Vec3::new(-15.5, 0.0, 7.5),
        ],
        5,
        1.25,
        3.8,
    );
    // Cordillera del fondo (se ve borrosa a través del arco).
    b.layered_cliff(
        &[
            Vec3::new(-14.0, 0.0, -17.0),
            Vec3::new(-5.0, 0.0, -19.0),
            Vec3::new(4.0, 0.0, -17.5),
            Vec3::new(14.0, 0.0, -19.0),
        ],
        6,
        1.4,
        6.0,
    );
    // Cordillera baja detrás de la cámara inicial.
    b.layered_cliff(
        &[
            Vec3::new(-12.0, 0.0, 18.0),
            Vec3::new(-2.0, 0.0, 19.5),
            Vec3::new(9.0, 0.0, 17.5),
        ],
        3,
        1.1,
        5.0,
    );
    // Agujas y bloques a la derecha.
    b.spire(Vec3::new(14.0, 0.0, -3.0), 6.5, 1.5);
    b.spire(Vec3::new(15.5, 0.0, 3.5), 4.5, 1.2);
    b.spire(Vec3::new(10.0, 0.0, -11.5), 5.5, 1.3);
    b.spire(Vec3::new(-4.0, 0.0, -14.5), 4.0, 1.1);
    for (c, s) in [
        (Vec3::new(8.2, 0.3, 1.0), 1.1),
        (Vec3::new(6.5, 0.3, -5.5), 1.0),
        (Vec3::new(-8.0, 0.3, -5.5), 1.1),
        (Vec3::new(-7.2, 0.2, 8.5), 1.0),
        (Vec3::new(4.5, 0.1, 9.5), 0.9),
        (Vec3::new(12.5, 0.4, 9.0), 1.6),
        (Vec3::new(-13.0, 0.5, 12.0), 1.7),
        (Vec3::new(15.0, 0.5, -11.0), 1.8),
    ] {
        b.boulder(c, s);
    }

    // Rugosidad extra sobre el arco y las formaciones.
    let placer = Placer::new(&b.prims);
    let arch_centers: Vec<(Vec3, f32)> = stations.iter().map(|s| (s.0, s.3.max(s.4))).collect();
    b.rubble(&placer, &arch_centers, 70);
    b.rubble(
        &placer,
        &[
            (Vec3::new(-14.5, 2.5, 0.0), 3.5),
            (Vec3::new(14.0, 3.0, -3.0), 2.0),
            (Vec3::new(-14.0, 2.0, -5.0), 3.5),
        ],
        60,
    );

    // --- Vida sobre la roca --------------------------------------------------
    let placer = Placer::new(&b.prims);
    // Corales de mesa pegados a las paredes del arco (frente y atrás).
    for _ in 0..70 {
        let (center, _tangent, out, r, dz) = stations[2 + b.rng.index(stations.len() - 4)];
        let side = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
        let origin = center + out * (r * b.rng.range(-0.5, 0.9)) + Vec3::new(0.0, 0.0, side * (dz + 3.0));
        let target = center + out * (r * 0.2);
        let dir = (target - origin).normalized();
        let hit = placer.cast(origin, dir, 8.0).filter(|h| h.2 == ROCK && h.1.y.abs() < 0.75);
        if let Some((p, n, _)) = hit {
            if b.rng.chance(0.3) {
                let s = b.rng.range(0.8, 1.3);
                b.shell_plate(p - n * 0.03, n, s);
            } else {
                let tint = b.coral_tint();
                let scale = b.rng.range(0.45, 0.85);
                b.table_coral(p - n * 0.05, n, scale, tint);
            }
        }
    }
    // Corales de mesa en el acantilado izquierdo y en las agujas.
    for _ in 0..40 {
        let (cx, cz, reach) = if b.rng.chance(0.6) {
            (-14.5, b.rng.range(-7.0, 6.0), 5.0)
        } else {
            (14.0, -3.0, 3.0)
        };
        let az = b.rng.range(0.0, TAU);
        let y = b.rng.range(0.6, 5.5);
        let origin = Vec3::new(cx + az.cos() * (reach + 2.0), y, cz + az.sin() * (reach + 2.0));
        let dir = Vec3::new(-az.cos(), 0.0, -az.sin());
        let hit = placer.cast(origin, dir, reach + 4.0).filter(|h| h.2 == ROCK && h.1.y.abs() < 0.7);
        if let Some((p, n, _)) = hit {
            if b.rng.chance(0.3) {
                let s = b.rng.range(0.9, 1.4);
                b.shell_plate(p - n * 0.03, n, s);
            } else {
                let tint = b.coral_tint();
                let scale = b.rng.range(0.5, 1.0);
                b.table_coral(p - n * 0.05, n, scale, tint);
            }
        }
    }

    // Plantas, abanicos y esponjas sobre las cimas (lanzando rayos hacia abajo).
    for _ in 0..900 {
        let x = b.rng.range(-17.0, 17.0);
        let z = b.rng.range(-20.0, 20.0);
        let (p, n, m) = placer.drop(x, z);
        if m != Some(ROCK) || n.y < 0.55 {
            continue;
        }
        let roll = b.rng.f();
        if roll < 0.45 {
            let s = b.rng.range(0.5, 1.1);
            b.grass(p, s);
        } else if roll < 0.62 {
            let s = b.rng.range(0.4, 0.75);
            b.red_plant(p, s);
        } else if roll < 0.66 {
            let h = b.rng.range(0.8, 1.5);
            b.sea_fan(p, h);
        } else if roll < 0.74 {
            let s = b.rng.range(0.5, 0.9);
            b.sponges(p, s);
        }
    }

    // --- Vida sobre la arena -------------------------------------------------
    let near_center = |x: f32, z: f32| x.abs() < 2.6 && z.abs() < 2.2;
    // Esponjas tubo (como en la primera imagen de referencia).
    for (x, z, s) in [
        (-3.3, 3.6, 1.0),
        (-2.6, 4.4, 0.7),
        (3.0, 4.6, 0.9),
        (-7.6, 3.0, 1.1),
        (6.0, -3.8, 1.0),
        (-5.4, -3.5, 0.9),
        (1.2, -5.2, 0.8),
        (8.4, 5.5, 1.2),
        (-1.0, 8.0, 0.8),
        (4.8, -10.5, 1.1),
    ] {
        let (p, n, m) = placer.drop(x, z);
        if Placer::is_sand(m) || n.y > 0.6 {
            b.sponges(p, s);
        }
    }
    // Corales cerebro: pocos, grandes y separados. Son los únicos que sueltan burbujas.
    for (x, z, r) in [(2.4, 3.4, 1.1), (-6.4, 2.2, 1.25), (6.6, -3.4, 1.05), (-3.8, -6.0, 1.2), (0.8, 9.0, 1.0)] {
        let (p, _, _) = placer.drop(x, z);
        b.brain(Vec3::new(p.x, 0.0, p.z), r, true);
    }
    // Corales ramificados.
    for (x, z, s) in [
        (2.5, -3.6, 0.9),
        (-4.0, 7.2, 1.0),
        (0.0, -7.5, 1.2),
    ] {
        let (p, _, m) = placer.drop(x, z);
        if Placer::is_sand(m) {
            b.branch_coral(p, s);
        }
    }
    // Abanicos de mar cerca de las rocas.
    for (x, z, h) in [
        (-8.6, 1.5, 1.7),
        (7.8, -3.0, 1.5),
        (-9.5, -3.0, 1.8),
        (-6.0, 6.5, 1.5),
    ] {
        let (p, _, m) = placer.drop(x, z);
        if Placer::is_sand(m) {
            b.sea_fan(p, h);
        }
    }
    // Hongos ácidos: pocos grupos sobre la arena.
    for (x, z, sc) in [
        (-4.0, 3.2, 1.0),
        (4.2, 1.6, 0.9),
        (-2.4, -3.6, 1.0),
        (2.2, -5.0, 0.9),
        (-8.0, 5.6, 1.1),
        (7.8, 7.4, 1.0),
        (-10.4, -1.6, 1.0),
        (10.4, 2.2, 1.0),
    ] {
        let (p, _, m) = placer.drop(x, z);
        if Placer::is_sand(m) {
            b.mushrooms(p, sc);
        }
    }
    // Dos manchas de pasto rojo que se mecen con la corriente.
    b.red_grass_patch(&placer, Vec3::new(-7.5, 0.0, -8.5), 2.2, 65);
    b.red_grass_patch(&placer, Vec3::new(8.5, 0.0, 9.5), 2.0, 60);
    // Algunas placas venosas sueltas apoyadas en la arena.
    for (x, z, a) in [(-2.4, 6.4, 0.3), (4.4, 5.0, 2.4), (-7.8, -4.6, 1.2), (6.2, -7.6, 4.0), (1.0, 8.6, 5.2)] {
        let (p, _, m) = placer.drop(x, z);
        if Placer::is_sand(m) {
            let dir = Vec3::new(f32::cos(a), 0.0, f32::sin(a));
            b.shell_plate(p - Vec3::new(0.0, 0.12, 0.0), dir, 1.3);
        }
    }
    // Cristales de cuarzo (refracción) y la cápsula de suministros (reflexión).
    for (x, z, s) in [(-2.9, 2.0, 0.9), (3.3, -1.8, 0.75), (6.6, 3.8, 0.6), (-6.6, -7.0, 0.8)] {
        let (p, _, _) = placer.drop(x, z);
        b.crystals(p, s);
    }
    {
        // Restos de metal que el stalker "vigila" (detrás y a la izquierda del arco).
        let home = fauna::STALKER_HOME;
        b.supply_crate(Vec3::new(home.x + 0.4, 0.0, home.z + 0.3), 2.1);
        let plate = Mat3::from_euler(0.6, 0.35, -0.25);
        b.push(Prim::new(Shape::Cube, Vec3::new(home.x - 1.0, 0.15, home.z - 0.6), Vec3::new(0.9, 0.04, 0.6), plate, METAL));
        b.push(
            Prim::new(Shape::Cube, Vec3::new(home.x + 1.3, 0.25, home.z - 0.9), Vec3::new(0.08, 0.35, 0.5), Mat3::from_euler(1.9, 0.1, 0.4), METAL)
                .with_color(Vec3::splat(0.8)),
        );
    }
    // Pasto marino y plantas venosas: más densos cerca de las rocas (ruido de densidad).
    let mut planted = 0;
    let mut tries = 0;
    while planted < 260 && tries < 6000 {
        tries += 1;
        let x = b.rng.range(-16.0, 16.0);
        let z = b.rng.range(-16.0, 16.0);
        if near_center(x, z) && b.rng.chance(0.85) {
            continue;
        }
        let (cell, _, _) = voronoi2(x * 0.18 + 40.0, z * 0.18 + 40.0, 0);
        let density = 1.0 - smoothstep(0.15, 0.55, cell);
        if !b.rng.chance(0.15 + density * 0.85) {
            continue;
        }
        let (p, n, m) = placer.drop(x, z);
        if !(Placer::is_sand(m) || (m == Some(ROCK) && n.y > 0.6)) {
            continue;
        }
        if b.rng.chance(0.72) {
            let s = b.rng.range(0.45, 1.05);
            b.grass(p, s);
        } else {
            let s = b.rng.range(0.35, 0.7);
            b.red_plant(p, s);
        }
        planted += 1;
    }
    // Piedritas sueltas.
    for _ in 0..140 {
        let x = b.rng.range(-15.0, 15.0);
        let z = b.rng.range(-15.0, 15.0);
        let (p, _, m) = placer.drop(x, z);
        if !Placer::is_sand(m) {
            continue;
        }
        let s = b.rng.range(0.04, 0.16);
        let half = Vec3::new(s, s * b.rng.range(0.35, 0.6), s * b.rng.range(0.6, 1.0));
        let rot = b.rand_rot(0.35);
        let tint = b.rock_tint_rand() * Vec3::new(1.1, 1.0, 0.95);
        let round = b.rng.chance(0.5);
        b.pebble(p + Vec3::new(0.0, s * 0.1, 0.0), half, rot, tint, round);
    }

    // --- Escena final ------------------------------------------------------------
    let Builder { prims, vents, glows, .. } = b;
    let boxes: Vec<_> = prims.iter().map(|p| p.aabb()).collect();
    let bvh = Bvh::build(&boxes);
    let lib = material::build_library();
    let (species, swimmers) = fauna::build();
    let sky = skybox::build_sky(256);
    let haze = skybox::build_haze(128);
    let caustics = Gray::generate(256, caustic_texture).with_mean(0.5);
    let shafts = caustics.blurred(9).with_mean(0.5);

    let sun_air = skybox::sun_air_direction();
    let sun_dir = -(-sun_air)
        .refract(Vec3::UP, 1.0 / 1.333)
        .unwrap_or(Vec3::new(0.0, -1.0, 0.0));

    let mut scene = Scene {
        prims,
        bvh,
        lib,
        sky,
        haze,
        caustics,
        shafts,
        sun_dir,
        sun_color: Vec3::new(1.45, 1.25, 1.0),
        water_y: WATER_Y,
        floor_extent: 120.0,
        vents,
        shadow: ShadowMap::empty(),
        ao: AoVolume::empty(),
        focus: Vec3::new(0.0, 2.6, 0.0),
        species,
        swimmers,
        glow_grid: GlowGrid::build(&glows, -26.0, 26.0, 4.0),
        glows,
    };
    scene.shadow = ShadowMap::build(&scene, Vec3::new(0.0, 3.0, 0.0), 25.0, 1024);
    scene.ao = AoVolume::build(
        &scene,
        Vec3::new(-22.0, 0.0, -22.0),
        Vec3::new(22.0, 9.0, 22.0),
        0.5,
        20,
        3.0,
    );
    scene
}

/// Red de cáusticas periódica (bordes de celdas de Voronoi deformadas).
fn caustic_texture(u: f32, v: f32) -> f32 {
    let warp_x = crate::noise::fbm2(u * 3.0, v * 3.0, 3, 3) - 0.5;
    let warp_y = crate::noise::fbm2(u * 3.0 + 7.0, v * 3.0 + 3.0, 3, 3) - 0.5;
    let (f1, f2, _) = voronoi2(u * 6.0 + warp_x * 1.1, v * 6.0 + warp_y * 1.1, 6);
    let edge = f2 - f1;
    let line = 1.0 - smoothstep(0.0, 0.30, edge);
    0.15 + line * line
}
