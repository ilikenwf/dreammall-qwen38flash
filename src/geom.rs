//! Geometry batching: the whole mall is built into a handful of merged meshes.
//! macroquad 0.4 has no model matrices — every vertex is baked into world space.

use macroquad::models::Vertex;
use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Batch {
    Flat, // vertex-colored, lit + fog
    Tex, // textured (floor/wall tiles), lit + fog
    Emissive, // unlit glow (lights, screens, neon)
    Glass, // translucent
    Water, // animated fountain water
    Ceiling,     // ceiling planes; culled while flying above the mall
    CeilingGlow, // ceiling lights/skylights; culled with the ceiling
}

pub const N_BATCHES: usize = 7;

pub fn batch_idx(b: Batch) -> usize {
    match b {
        Batch::Flat => 0,
        Batch::Tex => 1,
        Batch::Emissive => 2,
        Batch::Glass => 3,
        Batch::Water => 4,
        Batch::Ceiling => 5,
        Batch::CeilingGlow => 6,
    }
}

pub fn batch_kind(i: usize) -> Batch {
    match i {
        0 => Batch::Flat,
        1 => Batch::Tex,
        2 => Batch::Emissive,
        3 => Batch::Glass,
        4 => Batch::Water,
        5 => Batch::Ceiling,
        _ => Batch::CeilingGlow,
    }
}

pub struct Geom {
    pub batches: [Vec<Vertex>; N_BATCHES],
    xf: Mat4,
}

pub fn vert(pos: Vec3, uv: Vec2, col: [f32; 3]) -> Vertex {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
    Vertex {
        position: pos,
        uv,
        color: [c(col[0]), c(col[1]), c(col[2]), 255],
        normal: vec4(0.0, 0.0, 0.0, 0.0),
    }
}

fn face(v: &mut Vec<Vertex>, p: [Vec3; 4], col: [f32; 3], shade: [f32; 3]) {
    let uvs = [vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0)];
    let c = [col[0] * shade[0], col[1] * shade[1], col[2] * shade[2]];
    // macroquad draws empty-index meshes as sequential triangles: 0-1-2, 0-2-3
    for i in [0usize, 1, 2, 0, 2, 3] {
        v.push(vert(p[i], uvs[i], c));
    }
}
fn quad6(v: &mut Vec<Vertex>, p: [Vec3; 4], uvs: [Vec2; 4], col: [f32; 3]) {
    for i in [0usize, 1, 2, 0, 2, 3] {
        v.push(vert(p[i], uvs[i], col));
    }
}


impl Geom {
    pub fn new() -> Self {
        Geom {
            batches: Default::default(),
            xf: Mat4::IDENTITY,
        }
    }

    pub fn set_xf(&mut self, m: Mat4) {
        self.xf = m;
    }

    fn tp(&self, p: Vec3) -> Vec3 {
        self.xf.transform_point3(p)
    }

    /// axis-aligned box in world space (ignores transform)
    pub fn box_aabb(
        &mut self,
        b: Batch,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        col: [f32; 3],
    ) {
        self.box_shade(b, x0, y0, z0, x1, y1, z1, col, [1.0; 3]);
    }

    pub fn box_shade(
        &mut self,
        b: Batch,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        col: [f32; 3],
        shade: [f32; 3],
    ) {
        let v = [
            vec3(x0, y0, z0),
            vec3(x1, y0, z0),
            vec3(x1, y1, z0),
            vec3(x0, y1, z0),
            vec3(x0, y0, z1),
            vec3(x1, y0, z1),
            vec3(x1, y1, z1),
            vec3(x0, y1, z1),
        ];
        let f = [
            ([v[4], v[5], v[6], v[7]], [0.85, 0.85, 0.85]),
            ([v[1], v[0], v[3], v[2]], [0.85, 0.85, 0.85]),
            ([v[0], v[4], v[7], v[3]], [0.7, 0.7, 0.7]),
            ([v[5], v[1], v[2], v[6]], [1.0, 1.0, 1.0]),
            ([v[7], v[6], v[2], v[3]], [1.1, 1.1, 1.1]),
            ([v[0], v[1], v[5], v[4]], [0.5, 0.5, 0.5]),
        ];
        let bs = &mut self.batches[batch_idx(b)];
        for (corners, s) in f {
            face(
                bs,
                corners,
                col,
                [s[0] * shade[0], s[1] * shade[1], s[2] * shade[2]],
            );
        }
    }

    /// textured box: uv baked from world coords at `scale`
    pub fn box_tex(
        &mut self,
        b: Batch,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        col: [f32; 3],
        scale: f32,
    ) {
        let v = [
            vec3(x0, y0, z0),
            vec3(x1, y0, z0),
            vec3(x1, y1, z0),
            vec3(x0, y1, z0),
            vec3(x0, y0, z1),
            vec3(x1, y0, z1),
            vec3(x1, y1, z1),
            vec3(x0, y1, z1),
        ];
        let f = [
            ([v[4], v[5], v[6], v[7]], 0.9f32),
            ([v[1], v[0], v[3], v[2]], 0.9),
            ([v[0], v[4], v[7], v[3]], 0.75),
            ([v[5], v[1], v[2], v[6]], 1.0),
            ([v[7], v[6], v[2], v[3]], 1.0),
            ([v[0], v[1], v[5], v[4]], 0.6),
        ];
        let bs = &mut self.batches[batch_idx(b)];
        for (corners, s) in f {
            let uvs = corners.map(|pt| vec2(pt.x * scale, pt.z * scale));
            let cc = [col[0] * s, col[1] * s, col[2] * s];
            quad6(bs, corners, uvs, cc);
        }
    }

    /// textured horizontal quad (floor / ceiling)
    pub fn floor(
        &mut self,
        b: Batch,
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
        y: f32,
        col: [f32; 3],
        scale: f32,
        flip: bool,
    ) {
        let bs = &mut self.batches[batch_idx(b)];
        let (a, c) = if flip {
            (
                [
                    vec3(x0, y, z1),
                    vec3(x1, y, z1),
                    vec3(x1, y, z0),
                    vec3(x0, y, z0),
                ],
                [
                    vec2(x0 * scale, z1 * scale),
                    vec2(x1 * scale, z1 * scale),
                    vec2(x1 * scale, z0 * scale),
                    vec2(x0 * scale, z0 * scale),
                ],
            )
        } else {
            (
                [
                    vec3(x0, y, z0),
                    vec3(x1, y, z0),
                    vec3(x1, y, z1),
                    vec3(x0, y, z1),
                ],
                [
                    vec2(x0 * scale, z0 * scale),
                    vec2(x1 * scale, z0 * scale),
                    vec2(x1 * scale, z1 * scale),
                    vec2(x0 * scale, z1 * scale),
                ],
            )
        };
        quad6(bs, a, c, col);
    }

    /// vertical quad facing +x or -x (wall art, mirror glass)
    pub fn wall_quad(
        &mut self,
        b: Batch,
        x: f32,
        y0: f32,
        z0: f32,
        z1: f32,
        y1: f32,
        face_pos_x: bool,
        col: [f32; 3],
    ) {
        let bs = &mut self.batches[batch_idx(b)];
        let p = if face_pos_x {
            [
                vec3(x, y0, z1),
                vec3(x, y0, z0),
                vec3(x, y1, z0),
                vec3(x, y1, z1),
            ]
        } else {
            [
                vec3(x, y0, z0),
                vec3(x, y0, z1),
                vec3(x, y1, z1),
                vec3(x, y1, z0),
            ]
        };
        let uvs = [vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(1.0, 0.0), vec2(0.0, 0.0)];
        quad6(bs, p, uvs, col);
    }

    /// cylinder/cone around current transform origin, y0..y1
    pub fn cyl(
        &mut self,
        b: Batch,
        y0: f32,
        y1: f32,
        r0: f32,
        r1: f32,
        col: [f32; 3],
        segs: usize,
        cap_top: bool,
        cap_bot: bool,
    ) {
        let xf = self.xf;
        let tp = |p: Vec3| xf.transform_point3(p);
        let bs = &mut self.batches[batch_idx(b)];
        for i in 0..segs {
            let a0 = i as f32 / segs as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segs as f32 * std::f32::consts::TAU;
            let (c0, s0) = (a0.cos(), a0.sin());
            let (c1, s1) = (a1.cos(), a1.sin());
            let light = 0.75 + 0.35 * (a0 + 0.5).cos().max(0.0);
            let cc = [col[0] * light, col[1] * light, col[2] * light];
            let quad = [
                tp(vec3(c0 * r0, y0, s0 * r0)),
                tp(vec3(c1 * r0, y0, s1 * r0)),
                tp(vec3(c1 * r1, y1, s1 * r1)),
                tp(vec3(c0 * r1, y1, s0 * r1)),
            ];
            quad6(bs, quad, [vec2(0.0, 0.0); 4], cc);
            if cap_top {
                let c = tp(vec3(0.0, y1, 0.0));
                let cc2 = [col[0] * 1.1, col[1] * 1.1, col[2] * 1.1];
                for pt in [quad[3], quad[2], c] {
                    bs.push(vert(pt, vec2(0.0, 0.0), cc2));
                }
            }
            if cap_bot {
                let c = tp(vec3(0.0, y0, 0.0));
                let cc2 = [col[0] * 0.5, col[1] * 0.5, col[2] * 0.5];
                for pt in [quad[0], quad[1], c] {
                    bs.push(vert(pt, vec2(0.0, 0.0), cc2));
                }
            }
        }
    }

    /// box relative to current transform
    pub fn box_local(
        &mut self,
        b: Batch,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        col: [f32; 3],
    ) {
        let pts = [
            vec3(x0, y0, z0),
            vec3(x1, y0, z0),
            vec3(x1, y1, z0),
            vec3(x0, y1, z0),
            vec3(x0, y0, z1),
            vec3(x1, y0, z1),
            vec3(x1, y1, z1),
            vec3(x0, y1, z1),
        ];
        let p: Vec<Vec3> = pts.iter().map(|p| self.tp(*p)).collect();
        let f = [
            ([p[4], p[5], p[6], p[7]], 0.9f32),
            ([p[1], p[0], p[3], p[2]], 0.9),
            ([p[0], p[4], p[7], p[3]], 0.72),
            ([p[5], p[1], p[2], p[6]], 1.0),
            ([p[7], p[6], p[2], p[3]], 1.08),
            ([p[0], p[1], p[5], p[4]], 0.55),
        ];
        let bs = &mut self.batches[batch_idx(b)];
        for (corners, s) in f {
            face(bs, corners, col, [s, s, s]);
        }
    }

    pub fn finish(self) -> Vec<(Batch, Mesh)> {
        // macroquad clamps each draw call to `draw_call_vertex_capacity`
        // (default 10000 verts); split merged batches so nothing is dropped.
        const CHUNK: usize = 3996; // multiple of 3
        let mut out = Vec::new();
        for (i, b) in self.batches.into_iter().enumerate() {
            for part in b.chunks(CHUNK) {
                if part.is_empty() {
                    continue;
                }
                out.push((
                    batch_kind(i),
                    Mesh {
                        vertices: part.to_vec(),
                        indices: (0..part.len() as u16).collect(),
                        texture: None,
                    },
                ));
            }
        }
        out
    }
}

/// A textured quad baked into world space with an arbitrary transform.
pub fn baked_quad(xf: Mat4, tex: &Texture2D, col: [f32; 3]) -> Mesh {
    let corners = [
        vec3(-0.5, -0.5, 0.0),
        vec3(0.5, -0.5, 0.0),
        vec3(0.5, 0.5, 0.0),
        vec3(-0.5, 0.5, 0.0),
    ];
    let uvs = [
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 0.0),
    ];
    let p: Vec<Vec3> = corners.iter().map(|c| xf.transform_point3(*c)).collect();
    let verts = vec![
        vert(p[0], uvs[0], col),
        vert(p[1], uvs[1], col),
        vert(p[2], uvs[2], col),
        vert(p[0], uvs[0], col),
        vert(p[2], uvs[2], col),
        vert(p[3], uvs[3], col),
    ];
    Mesh {
        vertices: verts,
        indices: vec![0, 1, 2, 3, 4, 5],
        texture: Some(tex.weak_clone()),
    }
}
