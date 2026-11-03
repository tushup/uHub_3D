//! Binned-SAH BVH2 over triangles, leaves of at most 4, iterative
//! traversal (spec §A1.6). Intersection is Möller–Trumbore, two-sided,
//! with smooth normals and UVs interpolated at the hit.

use super::{add, cross, dot, sub, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub p: [Vec3; 3],
    pub n: [Vec3; 3],
    pub uv: [[f32; 2]; 3],
    pub material: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Hit {
    pub t: f32,
    /// Barycentric weights for corners 0, 1, 2.
    pub bary: [f32; 3],
    pub tri: usize,
}

impl Triangle {
    fn bounds(&self) -> (Vec3, Vec3) {
        let mut min = self.p[0];
        let mut max = self.p[0];
        for v in &self.p[1..] {
            for i in 0..3 {
                min[i] = min[i].min(v[i]);
                max[i] = max[i].max(v[i]);
            }
        }
        (min, max)
    }

    fn centroid(&self) -> Vec3 {
        [
            (self.p[0][0] + self.p[1][0] + self.p[2][0]) / 3.0,
            (self.p[0][1] + self.p[1][1] + self.p[2][1]) / 3.0,
            (self.p[0][2] + self.p[1][2] + self.p[2][2]) / 3.0,
        ]
    }

    /// Interpolated smooth normal at barycentric weights.
    pub fn normal(&self, bary: [f32; 3]) -> Vec3 {
        let n = add(
            add(
                [
                    self.n[0][0] * bary[0],
                    self.n[0][1] * bary[0],
                    self.n[0][2] * bary[0],
                ],
                [
                    self.n[1][0] * bary[1],
                    self.n[1][1] * bary[1],
                    self.n[1][2] * bary[1],
                ],
            ),
            [
                self.n[2][0] * bary[2],
                self.n[2][1] * bary[2],
                self.n[2][2] * bary[2],
            ],
        );
        let len = dot(n, n).sqrt();
        if len > 0.0 {
            [n[0] / len, n[1] / len, n[2] / len]
        } else {
            [0.0, 0.0, 1.0]
        }
    }

    pub fn uv(&self, bary: [f32; 3]) -> [f32; 2] {
        [
            self.uv[0][0] * bary[0] + self.uv[1][0] * bary[1] + self.uv[2][0] * bary[2],
            self.uv[0][1] * bary[0] + self.uv[1][1] * bary[1] + self.uv[2][1] * bary[2],
        ]
    }
}

/// Möller–Trumbore, two-sided. Returns (t, u, v) with u along
/// p0->p1 and v along p0->p2, or None.
pub fn intersect_tri(tri: &Triangle, origin: Vec3, dir: Vec3, t_max: f32) -> Option<(f32, f32, f32)> {
    let e1 = sub(tri.p[1], tri.p[0]);
    let e2 = sub(tri.p[2], tri.p[0]);
    let pvec = cross(dir, e2);
    let det = dot(e1, pvec);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = sub(origin, tri.p[0]);
    let u = dot(tvec, pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = cross(tvec, e1);
    let v = dot(dir, qvec) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(e2, qvec) * inv_det;
    if t <= 1e-6 || t >= t_max {
        return None;
    }
    Some((t, u, v))
}

const MAX_LEAF: usize = 4;
const BINS: usize = 16;

#[derive(Debug)]
enum NodeKind {
    Leaf { start: usize, count: usize },
    Inner { left: usize, right: usize },
}

#[derive(Debug)]
struct BvhNode {
    min: Vec3,
    max: Vec3,
    kind: NodeKind,
}

#[derive(Debug)]
pub struct Bvh {
    nodes: Vec<BvhNode>,
    order: Vec<usize>,
}

fn box_area(min: Vec3, max: Vec3) -> f32 {
    let d = [
        (max[0] - min[0]).max(0.0),
        (max[1] - min[1]).max(0.0),
        (max[2] - min[2]).max(0.0),
    ];
    2.0 * (d[0] * d[1] + d[1] * d[2] + d[2] * d[0])
}

impl Bvh {
    pub fn build(triangles: &[Triangle]) -> Bvh {
        let mut order: Vec<usize> = (0..triangles.len()).collect();
        let mut nodes = Vec::new();
        if !order.is_empty() {
            Self::build_rec(triangles, &mut order, 0, triangles.len(), &mut nodes);
        }
        Bvh { nodes, order }
    }

    fn build_rec(
        triangles: &[Triangle],
        order: &mut [usize],
        start: usize,
        end: usize,
        nodes: &mut Vec<BvhNode>,
    ) -> usize {
        let (mut min, mut max) = triangles[order[start]].bounds();
        let (mut cmin, mut cmax) = (min, max);
        for &ti in &order[start + 1..end] {
            let (tmin, tmax) = triangles[ti].bounds();
            let c = triangles[ti].centroid();
            for i in 0..3 {
                min[i] = min[i].min(tmin[i]);
                max[i] = max[i].max(tmax[i]);
                cmin[i] = cmin[i].min(c[i]);
                cmax[i] = cmax[i].max(c[i]);
            }
        }
        let node_index = nodes.len();
        nodes.push(BvhNode {
            min,
            max,
            kind: NodeKind::Leaf { start, count: end - start },
        });
        let count = end - start;
        if count <= MAX_LEAF {
            return node_index;
        }
        // Binned SAH over the three axes.
        let parent_area = box_area(min, max);
        let mut best: Option<(f32, usize, usize)> = None; // (cost, axis, split bin)
        if parent_area > 0.0 {
            for axis in 0..3 {
                let extent = cmax[axis] - cmin[axis];
                if extent <= 0.0 {
                    continue;
                }
                let mut bin_count = [0usize; BINS];
                let mut bin_min = [[f32::INFINITY; 3]; BINS];
                let mut bin_max = [[f32::NEG_INFINITY; 3]; BINS];
                let bin_of = |c: f32| -> usize {
                    ((((c - cmin[axis]) / extent) * BINS as f32) as usize).min(BINS - 1)
                };
                for &ti in &order[start..end] {
                    let tri = &triangles[ti];
                    let b = bin_of(tri.centroid()[axis]);
                    bin_count[b] += 1;
                    let (tmin, tmax) = tri.bounds();
                    for i in 0..3 {
                        bin_min[b][i] = bin_min[b][i].min(tmin[i]);
                        bin_max[b][i] = bin_max[b][i].max(tmax[i]);
                    }
                }
                // Sweep split positions.
                for split in 1..BINS {
                    let (mut lcount, mut rcount) = (0usize, 0usize);
                    let (mut lmin, mut lmax) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                    let (mut rmin, mut rmax) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                    for b in 0..split {
                        lcount += bin_count[b];
                        for i in 0..3 {
                            lmin[i] = lmin[i].min(bin_min[b][i]);
                            lmax[i] = lmax[i].max(bin_max[b][i]);
                        }
                    }
                    for b in split..BINS {
                        rcount += bin_count[b];
                        for i in 0..3 {
                            rmin[i] = rmin[i].min(bin_min[b][i]);
                            rmax[i] = rmax[i].max(bin_max[b][i]);
                        }
                    }
                    if lcount == 0 || rcount == 0 {
                        continue;
                    }
                    let cost = (lcount as f32 * box_area(lmin, lmax)
                        + rcount as f32 * box_area(rmin, rmax))
                        / parent_area;
                    if best.map(|(c, _, _)| cost < c).unwrap_or(true) {
                        best = Some((cost, axis, split));
                    }
                }
            }
        }
        let Some((cost, axis, split)) = best else {
            return node_index;
        };
        if cost >= count as f32 {
            return node_index;
        }
        let extent = cmax[axis] - cmin[axis];
        let threshold = cmin[axis] + extent * split as f32 / BINS as f32;
        let mut mid = start;
        let mut i = start;
        while i < end {
            if triangles[order[i]].centroid()[axis] < threshold {
                order.swap(i, mid);
                mid += 1;
            }
            i += 1;
        }
        if mid == start || mid == end {
            mid = (start + end) / 2;
        }
        let left = Self::build_rec(triangles, order, start, mid, nodes);
        let right = Self::build_rec(triangles, order, mid, end, nodes);
        nodes[node_index].kind = NodeKind::Inner { left, right };
        node_index
    }

    fn ray_box(min: Vec3, max: Vec3, origin: Vec3, inv_dir: Vec3, t_max: f32) -> bool {
        let mut t_min = 0.0f32;
        let mut t_far = t_max;
        for i in 0..3 {
            let t1 = (min[i] - origin[i]) * inv_dir[i];
            let t2 = (max[i] - origin[i]) * inv_dir[i];
            t_min = t_min.max(t1.min(t2));
            t_far = t_far.min(t1.max(t2));
            if t_far < t_min {
                return false;
            }
        }
        true
    }

    pub fn intersect(&self, triangles: &[Triangle], origin: Vec3, dir: Vec3) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv_dir = [1.0 / dir[0], 1.0 / dir[1], 1.0 / dir[2]];
        let mut best: Option<Hit> = None;
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni];
            let t_max = best.map(|h| h.t).unwrap_or(f32::INFINITY);
            if !Self::ray_box(node.min, node.max, origin, inv_dir, t_max) {
                continue;
            }
            match node.kind {
                NodeKind::Leaf { start, count } => {
                    for &ti in &self.order[start..start + count] {
                        let limit = best.map(|h| h.t).unwrap_or(f32::INFINITY);
                        if let Some((t, u, v)) =
                            intersect_tri(&triangles[ti], origin, dir, limit)
                        {
                            best = Some(Hit { t, bary: [1.0 - u - v, u, v], tri: ti });
                        }
                    }
                }
                NodeKind::Inner { left, right } => {
                    stack.push(left);
                    stack.push(right);
                }
            }
        }
        best
    }

    /// Any-hit query for shadow rays: true when anything blocks the
    /// ray before `t_max`.
    pub fn occluded(&self, triangles: &[Triangle], origin: Vec3, dir: Vec3, t_max: f32) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let inv_dir = [1.0 / dir[0], 1.0 / dir[1], 1.0 / dir[2]];
        let mut stack = vec![0usize];
        while let Some(ni) = stack.pop() {
            let node = &self.nodes[ni];
            if !Self::ray_box(node.min, node.max, origin, inv_dir, t_max) {
                continue;
            }
            match node.kind {
                NodeKind::Leaf { start, count } => {
                    for &ti in &self.order[start..start + count] {
                        if intersect_tri(&triangles[ti], origin, dir, t_max).is_some() {
                            return true;
                        }
                    }
                }
                NodeKind::Inner { left, right } => {
                    stack.push(left);
                    stack.push(right);
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::super::sampler::Rng;
    use super::super::{add, normalize};
    use super::*;

    fn quad_tri() -> Triangle {
        Triangle {
            p: [[-3.0, -3.0, 0.0], [3.0, -3.0, 0.0], [3.0, 3.0, 0.0]],
            n: [[0.0, 0.0, 1.0]; 3],
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            material: 0,
        }
    }

    #[test]
    fn intersect_quad_from_outside() {
        let tri = quad_tri();
        let (t, u, v) = intersect_tri(&tri, [0.0, 0.0, 5.0], [0.0, 0.0, -1.0], f32::INFINITY)
            .expect("hits the quad");
        assert!((t - 5.0).abs() < 1e-5);
        // Centre of the quad: u along p0->p1, v along p0->p2.
        assert!(u.abs() < 1e-5 && (v - 0.5).abs() < 1e-5, "u={u} v={v}");
    }

    #[test]
    fn intersect_from_inside_hits_backface() {
        let tri = quad_tri();
        // Ray from behind the quad still hits (two-sided).
        assert!(intersect_tri(&tri, [0.0, 0.0, -5.0], [0.0, 0.0, 1.0], f32::INFINITY).is_some());
    }

    #[test]
    fn miss_returns_none() {
        let tris = vec![quad_tri()];
        assert!(intersect_tri(&tris[0], [5.0, 5.0, 5.0], [0.0, 0.0, -1.0], f32::INFINITY).is_none());
        let bvh = Bvh::build(&tris);
        assert!(bvh.intersect(&tris, [5.0, 5.0, 5.0], [0.0, 0.0, -1.0]).is_none());
        assert!(!bvh.occluded(&tris, [5.0, 5.0, 5.0], [0.0, 0.0, -1.0], 100.0));
    }

    #[test]
    fn bvh_matches_brute_force() {
        let mut rng = Rng::new(7);
        let mut tris = Vec::new();
        for i in 0..500 {
            let c = [
                rng.next_f32() * 4.0 - 2.0,
                rng.next_f32() * 4.0 - 2.0,
                rng.next_f32() * 4.0 - 2.0,
            ];
            tris.push(Triangle {
                p: [
                    c,
                    add(c, [rng.next_f32(), rng.next_f32() * 0.5, 0.0]),
                    add(c, [0.0, rng.next_f32(), rng.next_f32() * 0.5]),
                ],
                n: [[0.0, 0.0, 1.0]; 3],
                uv: [[0.0; 2]; 3],
                material: (i % 3) as u32,
            });
        }
        let bvh = Bvh::build(&tris);
        for _ in 0..2000 {
            let origin = [
                rng.next_f32() * 6.0 - 3.0,
                rng.next_f32() * 6.0 - 3.0,
                rng.next_f32() * 6.0 - 3.0,
            ];
            let dir = normalize([
                rng.next_f32() - 0.5,
                rng.next_f32() - 0.5,
                rng.next_f32() - 0.5,
            ]);
            let brute = tris
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    intersect_tri(t, origin, dir, f32::INFINITY).map(|(t, _, _)| (t, i))
                })
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let fast = bvh.intersect(&tris, origin, dir);
            match (brute, fast) {
                (None, None) => {}
                (Some((bt, bi)), Some(hit)) => {
                    assert_eq!(hit.tri, bi, "t brute {bt} bvh {}", hit.t);
                    assert!((hit.t - bt).abs() < 1e-5, "t {bt} vs {}", hit.t);
                }
                (b, f) => panic!("mismatch brute={:?} bvh={:?}", b.is_some(), f.is_some()),
            }
        }
    }
}
