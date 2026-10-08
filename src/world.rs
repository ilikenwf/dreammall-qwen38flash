//! Assembles a Layout into GPU meshes: batched mall geometry + per-sign quads.

use macroquad::prelude::*;

use crate::geom::{baked_quad, Batch, Geom};
use crate::layout::Layout;
use crate::props;
use crate::tex::TexLib;

pub struct MirrorDraw {
    pub mesh: Mesh,
    pub x: f32,
    pub z: f32,
    pub facing_x: f32, // +1 faces +x, -1 faces -x
}

pub struct World {
    pub batches: Vec<(Batch, Mesh)>,
    pub signs: Vec<Mesh>,
    pub storefront_signs: Vec<Mesh>,
    pub mirrors: Vec<MirrorDraw>,
}

fn sign_transform(x: f32, y: f32, z: f32, w: f32, h: f32, face_rot: f32, tilt: f32) -> Mat4 {
    Mat4::from_translation(vec3(x, y, z))
        * Mat4::from_rotation_y(face_rot)
        * Mat4::from_rotation_z(tilt)
        * Mat4::from_scale(vec3(w, h, 1.0))
}

impl World {
    pub fn build(layout: Layout, tex: &mut TexLib) -> World {
        let mut g = Geom::new();
        let mut signs = Vec::new();
        let mut storefront_signs = Vec::new();

        // ---- floors, ceilings, walls per section ----
        for s in &layout.sections {
            let (hw, ceil) = (s.half_w, s.ceiling);
            let carpet = matches!(s.kind, crate::layout::SectionKind::FoodCourt);
            let floor_col = if carpet { [0.9; 3] } else { [1.0; 3] };
            let scale = if carpet { 0.25 } else { 0.22 };
            g.floor(Batch::Tex, -hw, s.z0, hw, s.z1, 0.0, floor_col, scale, false);
            g.floor(Batch::Ceiling, -hw, s.z0, hw, s.z1, ceil, [0.62, 0.6, 0.62], 0.0, true);

            // ceiling light strips
            let mut lz = s.z0 + 2.0;
            while lz < s.z1 - 1.0 {
                g.box_aabb(Batch::CeilingGlow, -1.4, ceil - 0.1, lz, 1.4, ceil - 0.04, lz + 0.4, [1.0, 0.98, 0.9]);
                g.box_aabb(Batch::CeilingGlow, -hw + 1.0, ceil - 0.1, lz, -hw + 1.8, ceil - 0.04, lz + 0.4, [1.0, 0.96, 0.85]);
                g.box_aabb(Batch::CeilingGlow, hw - 1.8, ceil - 0.1, lz, hw - 1.0, ceil - 0.04, lz + 0.4, [1.0, 0.96, 0.85]);
                lz += 4.0;
            }
            if matches!(s.kind, crate::layout::SectionKind::Atrium) {
                let zc = (s.z0 + s.z1) / 2.0;
                g.floor(Batch::CeilingGlow, -3.0, zc - 2.0, 3.0, zc + 2.0, ceil - 0.02, [0.95, 0.93, 0.85], 0.0, true);
            }

            // walls with storefront openings
            for side in [-1.0f32, 1.0f32] {
                let openings: Vec<(f32, f32)> = layout
                    .storefronts
                    .iter()
                    .filter(|sf| (sf.side as f32) == side && sf.z > s.z0 && sf.z < s.z1)
                    .map(|sf| (sf.z - 3.2, sf.z + 3.2))
                    .collect();
                let (wx0, wx1) = if side < 0.0 { (-hw - 0.4, -hw) } else { (hw, hw + 0.4) };
                let mut cursor = s.z0;
                let open_h = 3.6f32;
                for (oz0, oz1) in openings {
                    let emit = |g: &mut Geom, z0: f32, z1: f32, y0: f32, y1: f32| {
                        if z1 - z0 < 0.01 || y1 - y0 < 0.01 {
                            return;
                        }
                        g.box_tex(Batch::Tex, wx0, y0, z0, wx1, y1, z1, [0.95, 0.93, 0.9], 0.3);
                    };
                    emit(&mut g, cursor, oz0, 0.0, ceil);
                    emit(&mut g, oz0, oz1, open_h, ceil); // lintel
                    cursor = oz1;
                }
                let emit = |g: &mut Geom, z0: f32, z1: f32, y0: f32, y1: f32| {
                    if z1 - z0 < 0.01 || y1 - y0 < 0.01 {
                        return;
                    }
                    g.box_tex(Batch::Tex, wx0, y0, z0, wx1, y1, z1, [0.95, 0.93, 0.9], 0.3);
                };
                emit(&mut g, cursor, s.z1, 0.0, ceil);
            }
        }

        // ---- storefronts + their signs ----
        for sf in &layout.storefronts {
            props::add_storefront(&mut g, sf, layout.section_at(sf.z).half_w);
            let hw = layout.section_at(sf.z).half_w;
            let wall = sf.side as f32 * (hw - 0.12);
            let face_rot = if sf.side < 0 {
                std::f32::consts::FRAC_PI_2
            } else {
                -std::f32::consts::FRAC_PI_2
            };
            let name = crate::layout::store_name(sf.name_idx);
            let t = tex.sign(&name, sf.color, [1.0, 0.97, 0.85]);
            storefront_signs.push(baked_quad(
                sign_transform(wall, 4.15, sf.z, 5.4, 1.05, face_rot, 0.0),
                &t,
                [1.0; 3],
            ));
        }

        // ---- props ----
        for p in &layout.props {
            if p.gone {
                continue;
            }
            match p.kind {
                crate::layout::PropKind::Bench => props::add_bench(&mut g, p.x, p.z, p.rot, p.color),
                crate::layout::PropKind::Plant => props::add_plant(&mut g, p.x, p.z, p.color),
                crate::layout::PropKind::Vending => props::add_vending(&mut g, p.x, p.z, p.rot, p.color),
                crate::layout::PropKind::Table => props::add_table(&mut g, p.x, p.z, p.color),
                crate::layout::PropKind::Chair => props::add_chair(&mut g, p.x, p.z, p.rot, p.color),
                crate::layout::PropKind::Trash => props::add_trash(&mut g, p.x, p.z, p.color),
                crate::layout::PropKind::Statue => props::add_statue(&mut g, p.x, p.z, p.rot, p.color),
                crate::layout::PropKind::Luggage => props::add_luggage(&mut g, p.x, p.z, p.rot, p.color),
                crate::layout::PropKind::Mannequin => props::add_mannequin(&mut g, p.x, p.z, p.rot, p.color),
                crate::layout::PropKind::Column => {
                    let h = layout.section_at(p.z).ceiling;
                    props::add_column(&mut g, p.x, p.z, h, p.color)
                }
            }
        }

        // ---- fountains ----
        for f in &layout.fountains {
            props::add_fountain(&mut g, f);
        }

        // ---- escalators + mezzanines ----
        for e in &layout.escalators {
            let hw = layout.section_at(e.z).half_w;
            props::add_escalator(&mut g, e.side, e.z, hw);
            props::add_second_floor(&mut g, e.side, e.z, hw);
        }

        // ---- signs + posters ----
        for s in &layout.signs {
            let hw = layout.section_at(s.z).half_w;
            let (x, face_rot) = if s.x.abs() > hw - 1.0 {
                if s.x < 0.0 {
                    (-hw + 0.06, std::f32::consts::FRAC_PI_2)
                } else {
                    (hw - 0.06, -std::f32::consts::FRAC_PI_2)
                }
            } else {
                (s.x, std::f32::consts::PI)
            };
            let t = match s.poster {
                Some(i) => tex.posters[i].weak_clone(),
                None => tex.sign(&s.text, s.bg, s.fg),
            };
            signs.push(baked_quad(
                sign_transform(x, s.y, s.z, s.w, s.h, face_rot, s.tilt),
                &t,
                [1.0; 3],
            ));
        }

        // ---- mirrors ----
        let mut mirrors = Vec::new();
        for m in &layout.mirrors {
            let hw = layout.section_at(m.z).half_w;
            let (face_rot, facing_x) = if m.x < 0.0 {
                (std::f32::consts::FRAC_PI_2, 1.0f32)
            } else {
                (-std::f32::consts::FRAC_PI_2, -1.0f32)
            };
            let x = m.x.signum() * (hw - 0.05);
            let white = tex.white.weak_clone();
            mirrors.push(MirrorDraw {
                mesh: baked_quad(
                    sign_transform(x, m.y, m.z, m.w, m.h, face_rot, 0.0),
                    &white,
                    [1.0; 3],
                ),
                x,
                z: m.z,
                facing_x,
            });
            props::add_mirror_frame(&mut g, x, m.z, m.w, m.h, m.y);
        }

        World {
            batches: g.finish(),
            signs,
            storefront_signs,
            mirrors,
        }
    }
}
