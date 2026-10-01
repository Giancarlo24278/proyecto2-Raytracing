//! Jerarquía de volúmenes envolventes (BVH) construida con SAH por cubetas.
//! Es lo que permite tener miles de figuras sin que cada rayo las pruebe todas.

use crate::math::{Ray, Vec3, fmax, fmin};

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const EMPTY: Self = Self {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    pub fn grow(&mut self, o: &Aabb) {
        self.min = self.min.min(o.min);
        self.max = self.max.max(o.max);
    }

    pub fn grow_point(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn centroid(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_area(&self) -> f32 {
        let e = self.max - self.min;
        if e.x < 0.0 { 0.0 } else { e.x * e.y + e.y * e.z + e.z * e.x }
    }

    /// Distancia de entrada del rayo a la caja, o infinito si no la toca.
    #[inline(always)]
    pub fn hit(&self, o: Vec3, inv: Vec3, tmin: f32, tmax: f32) -> f32 {
        let t1 = (self.min - o) * inv;
        let t2 = (self.max - o) * inv;
        let near = fmax(t1.min(t2).max_elem(), tmin);
        let far = fmin(t1.max(t2).min_elem(), tmax);
        if near <= far { near } else { f32::INFINITY }
    }
}

#[derive(Clone, Copy)]
struct Node {
    bounds: Aabb,
    /// Hoja: primer índice en `order`. Nodo interno: índice del hijo izquierdo.
    first: u32,
    /// Cantidad de primitivas (0 = nodo interno).
    count: u32,
}

pub struct Bvh {
    nodes: Vec<Node>,
    order: Vec<u32>,
}

const BINS: usize = 12;
const MAX_LEAF: usize = 4;

impl Bvh {
    pub fn build(boxes: &[Aabb]) -> Self {
        let mut order: Vec<u32> = (0..boxes.len() as u32).collect();
        let centroids: Vec<Vec3> = boxes.iter().map(|b| b.centroid()).collect();
        let mut nodes = Vec::with_capacity(boxes.len() * 2);
        nodes.push(Node {
            bounds: Aabb::EMPTY,
            first: 0,
            count: boxes.len() as u32,
        });
        if !boxes.is_empty() {
            Self::subdivide(0, 0, boxes.len(), boxes, &centroids, &mut order, &mut nodes);
        }
        Self { nodes, order }
    }

    fn subdivide(
        node_index: usize,
        start: usize,
        end: usize,
        boxes: &[Aabb],
        centroids: &[Vec3],
        order: &mut [u32],
        nodes: &mut Vec<Node>,
    ) {
        let mut bounds = Aabb::EMPTY;
        let mut cbounds = Aabb::EMPTY;
        for &i in &order[start..end] {
            bounds.grow(&boxes[i as usize]);
            cbounds.grow_point(centroids[i as usize]);
        }
        nodes[node_index].bounds = bounds;
        nodes[node_index].first = start as u32;
        nodes[node_index].count = (end - start) as u32;
        let count = end - start;
        if count <= MAX_LEAF {
            return;
        }

        // SAH por cubetas en los tres ejes.
        let mut best_cost = f32::INFINITY;
        let mut best_axis = 0;
        let mut best_split = 0;
        for axis in 0..3 {
            let lo = cbounds.min.get(axis);
            let hi = cbounds.max.get(axis);
            if hi - lo < 1e-6 {
                continue;
            }
            let scale = BINS as f32 / (hi - lo);
            let mut bin_bounds = [Aabb::EMPTY; BINS];
            let mut bin_count = [0usize; BINS];
            for &i in &order[start..end] {
                let b = (((centroids[i as usize].get(axis) - lo) * scale) as usize).min(BINS - 1);
                bin_count[b] += 1;
                bin_bounds[b].grow(&boxes[i as usize]);
            }
            let mut left_area = [0.0f32; BINS];
            let mut left_count = [0usize; BINS];
            let mut acc = Aabb::EMPTY;
            let mut n = 0;
            for b in 0..BINS - 1 {
                acc.grow(&bin_bounds[b]);
                n += bin_count[b];
                left_area[b] = acc.half_area();
                left_count[b] = n;
            }
            let mut acc = Aabb::EMPTY;
            let mut n = 0;
            for b in (1..BINS).rev() {
                acc.grow(&bin_bounds[b]);
                n += bin_count[b];
                let cost = left_count[b - 1] as f32 * left_area[b - 1] + n as f32 * acc.half_area();
                if cost < best_cost {
                    best_cost = cost;
                    best_axis = axis;
                    best_split = b;
                }
            }
        }

        let leaf_cost = count as f32 * bounds.half_area();
        let mid = if best_cost.is_finite() && best_cost < leaf_cost {
            let lo = cbounds.min.get(best_axis);
            let scale = BINS as f32 / (cbounds.max.get(best_axis) - lo);
            let slice = &mut order[start..end];
            let mut left = 0;
            for i in 0..slice.len() {
                let b = (((centroids[slice[i] as usize].get(best_axis) - lo) * scale) as usize).min(BINS - 1);
                if b < best_split {
                    slice.swap(i, left);
                    left += 1;
                }
            }
            start + left
        } else if count > MAX_LEAF * 4 {
            // Respaldo: mediana en el eje más largo.
            let e = cbounds.max - cbounds.min;
            let axis = if e.x > e.y && e.x > e.z { 0 } else if e.y > e.z { 1 } else { 2 };
            order[start..end].sort_by(|a, b| {
                centroids[*a as usize]
                    .get(axis)
                    .total_cmp(&centroids[*b as usize].get(axis))
            });
            start + count / 2
        } else {
            return;
        };
        if mid == start || mid == end {
            return;
        }

        let left_index = nodes.len();
        nodes.push(Node {
            bounds: Aabb::EMPTY,
            first: 0,
            count: 0,
        });
        nodes.push(Node {
            bounds: Aabb::EMPTY,
            first: 0,
            count: 0,
        });
        nodes[node_index].first = left_index as u32;
        nodes[node_index].count = 0;
        Self::subdivide(left_index, start, mid, boxes, centroids, order, nodes);
        Self::subdivide(left_index + 1, mid, end, boxes, centroids, order, nodes);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Impacto más cercano. `test(prim, tmax)` devuelve la distancia si la primitiva es tocada.
    #[inline]
    pub fn closest(
        &self,
        ray: &Ray,
        tmin: f32,
        mut tmax: f32,
        mut test: impl FnMut(u32, f32) -> Option<f32>,
    ) -> Option<(u32, f32)> {
        if self.nodes.is_empty() || self.order.is_empty() {
            return None;
        }
        let inv = Vec3::new(1.0 / ray.dir.x, 1.0 / ray.dir.y, 1.0 / ray.dir.z);
        let o = ray.origin;
        if self.nodes[0].bounds.hit(o, inv, tmin, tmax).is_infinite() {
            return None;
        }
        let mut best = None;
        let mut stack = [(0u32, 0.0f32); 64];
        let mut sp = 0;
        let mut node = &self.nodes[0];
        loop {
            if node.count > 0 {
                let first = node.first as usize;
                for &prim in &self.order[first..first + node.count as usize] {
                    if let Some(t) = test(prim, tmax) {
                        tmax = t;
                        best = Some((prim, t));
                    }
                }
            } else {
                let li = node.first as usize;
                let left = &self.nodes[li];
                let right = &self.nodes[li + 1];
                let mut dl = left.bounds.hit(o, inv, tmin, tmax);
                let mut dr = right.bounds.hit(o, inv, tmin, tmax);
                let (mut near, mut far) = (li as u32, li as u32 + 1);
                if dr < dl {
                    std::mem::swap(&mut dl, &mut dr);
                    std::mem::swap(&mut near, &mut far);
                }
                if dl.is_finite() {
                    if dr.is_finite() && sp < stack.len() {
                        stack[sp] = (far, dr);
                        sp += 1;
                    }
                    node = &self.nodes[near as usize];
                    continue;
                }
            }
            // Sacar de la pila el siguiente nodo que aún pueda estar más cerca.
            loop {
                if sp == 0 {
                    return best;
                }
                sp -= 1;
                let (index, dist) = stack[sp];
                if dist <= tmax {
                    node = &self.nodes[index as usize];
                    break;
                }
            }
        }
    }

    /// Primitivas cuya caja contiene el punto `p`; se detiene si `visit` devuelve `true`.
    pub fn any_at(&self, p: Vec3, mut visit: impl FnMut(u32) -> bool) -> bool {
        if self.nodes.is_empty() || self.order.is_empty() {
            return false;
        }
        let mut stack = [0u32; 64];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            let b = &node.bounds;
            if p.x < b.min.x || p.y < b.min.y || p.z < b.min.z || p.x > b.max.x || p.y > b.max.y || p.z > b.max.z {
                continue;
            }
            if node.count > 0 {
                let first = node.first as usize;
                for &prim in &self.order[first..first + node.count as usize] {
                    if visit(prim) {
                        return true;
                    }
                }
            } else if sp + 2 <= stack.len() {
                stack[sp] = node.first;
                stack[sp + 1] = node.first + 1;
                sp += 2;
            }
        }
        false
    }

    /// Recorre todas las primitivas que el rayo podría tocar; se detiene si `visit` devuelve `true`.
    #[inline]
    pub fn any(&self, ray: &Ray, tmin: f32, tmax: f32, mut visit: impl FnMut(u32) -> bool) -> bool {
        if self.nodes.is_empty() || self.order.is_empty() {
            return false;
        }
        let inv = Vec3::new(1.0 / ray.dir.x, 1.0 / ray.dir.y, 1.0 / ray.dir.z);
        let o = ray.origin;
        let mut stack = [0u32; 64];
        let mut sp = 0;
        stack[sp] = 0;
        sp += 1;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if node.bounds.hit(o, inv, tmin, tmax).is_infinite() {
                continue;
            }
            if node.count > 0 {
                let first = node.first as usize;
                for &prim in &self.order[first..first + node.count as usize] {
                    if visit(prim) {
                        return true;
                    }
                }
            } else if sp + 2 <= stack.len() {
                stack[sp] = node.first;
                stack[sp + 1] = node.first + 1;
                sp += 2;
            }
        }
        false
    }
}
