//! Reusable mall props, built from boxes and cylinders into shared batches.

use macroquad::prelude::*;

use crate::geom::{Batch, Geom};
use crate::layout::{Fountain, Storefront};
use crate::util::{hsv, shade};

fn xf(x: f32, z: f32, rot: f32) -> Mat4 {
    Mat4::from_translation(vec3(x, 0.0, z)) * Mat4::from_rotation_y(rot)
}

pub fn add_bench(g: &mut Geom, x: f32, z: f32, rot: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, rot));
    g.box_local(Batch::Flat, -1.0, 0.42, -0.28, 1.0, 0.5, 0.28, color);
    g.box_local(Batch::Flat, -1.0, 0.5, -0.32, 1.0, 1.0, -0.24, shade(color, 0.9));
    g.box_local(Batch::Flat, -0.9, 0.0, -0.22, -0.75, 0.42, 0.22, [0.35, 0.33, 0.3]);
    g.box_local(Batch::Flat, 0.75, 0.0, -0.22, 0.9, 0.42, 0.22, [0.35, 0.33, 0.3]);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_plant(g: &mut Geom, x: f32, z: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, 0.0));
    g.cyl(Batch::Flat, 0.0, 0.42, 0.30, 0.24, [0.6, 0.35, 0.25], 10, false, true);
    g.cyl(Batch::Flat, 0.42, 1.15, 0.34, 0.02, color, 8, true, false);
    g.cyl(Batch::Flat, 0.85, 1.6, 0.26, 0.02, shade(color, 0.85), 8, true, false);
    g.cyl(Batch::Flat, 1.3, 2.0, 0.18, 0.02, shade(color, 1.1), 7, true, false);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_vending(g: &mut Geom, x: f32, z: f32, rot: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, rot));
    g.box_local(Batch::Flat, -0.55, 0.0, -0.36, 0.55, 2.0, 0.36, color);
    // glowing product window
    g.box_local(Batch::Emissive, -0.42, 0.25, 0.36, 0.30, 1.6, 0.37, [0.9, 0.95, 1.0]);
    // side glow strip + top sign
    g.box_local(Batch::Emissive, 0.34, 0.25, 0.36, 0.5, 1.6, 0.37, hsv(0.0, 0.6, 0.9));
    g.box_local(Batch::Emissive, -0.55, 1.72, 0.36, 0.55, 1.95, 0.37, shade(color, 1.4));
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_table(g: &mut Geom, x: f32, z: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, 0.0));
    g.cyl(Batch::Flat, 0.0, 0.05, 0.32, 0.32, [0.4, 0.4, 0.42], 12, true, true);
    g.cyl(Batch::Flat, 0.05, 0.72, 0.07, 0.07, [0.5, 0.5, 0.52], 8, false, false);
    g.cyl(Batch::Flat, 0.72, 0.77, 0.56, 0.56, color, 16, true, true);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_chair(g: &mut Geom, x: f32, z: f32, rot: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, rot));
    g.box_local(Batch::Flat, -0.21, 0.42, -0.21, 0.21, 0.48, 0.21, color);
    g.box_local(Batch::Flat, -0.21, 0.48, -0.24, 0.21, 0.95, -0.17, shade(color, 0.9));
    g.cyl(Batch::Flat, 0.0, 0.42, 0.05, 0.05, [0.4, 0.4, 0.42], 6, false, false);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_trash(g: &mut Geom, x: f32, z: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, 0.0));
    g.cyl(Batch::Flat, 0.0, 0.72, 0.3, 0.26, color, 12, false, true);
    g.cyl(Batch::Flat, 0.72, 0.78, 0.3, 0.3, [0.3, 0.3, 0.32], 12, true, false);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_statue(g: &mut Geom, x: f32, z: f32, rot: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, rot));
    g.box_local(Batch::Flat, -0.5, 0.0, -0.5, 0.5, 0.9, 0.5, [0.6, 0.58, 0.55]);
    g.cyl(Batch::Flat, 0.9, 1.9, 0.28, 0.16, color, 10, true, false);
    g.set_xf(xf(x, z, rot + 0.9));
    g.box_local(Batch::Flat, -0.06, 1.5, -0.5, 0.06, 2.4, 0.06, shade(color, 0.9));
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_luggage(g: &mut Geom, x: f32, z: f32, rot: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, rot));
    g.box_local(Batch::Flat, -0.25, 0.05, -0.14, 0.25, 0.7, 0.14, color);
    g.box_local(Batch::Flat, -0.05, 0.7, -0.03, 0.05, 1.0, 0.03, [0.3, 0.3, 0.32]);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_mannequin(g: &mut Geom, x: f32, z: f32, rot: f32, color: [f32; 3]) {
    g.set_xf(xf(x, z, rot));
    g.cyl(Batch::Flat, 0.0, 0.04, 0.22, 0.22, [0.4, 0.4, 0.4], 10, true, true);
    g.cyl(Batch::Flat, 0.04, 0.95, 0.09, 0.13, color, 8, false, false);
    g.box_local(Batch::Flat, -0.19, 0.95, -0.12, 0.19, 1.55, 0.12, color);
    g.cyl(Batch::Flat, 1.55, 1.82, 0.11, 0.1, shade(color, 0.95), 10, true, true);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
}

pub fn add_column(g: &mut Geom, x: f32, z: f32, h: f32, color: [f32; 3]) {
    g.box_aabb(Batch::Flat, x - 0.4, 0.0, z - 0.4, x + 0.4, h, z + 0.4, color);
    g.box_aabb(Batch::Flat, x - 0.55, 0.0, z - 0.55, x + 0.55, 0.25, z + 0.55, shade(color, 0.85));
    g.box_aabb(Batch::Flat, x - 0.55, h - 0.25, z - 0.55, x + 0.55, h, z + 0.55, shade(color, 0.85));
}

pub fn add_fountain(g: &mut Geom, f: &Fountain) {
    let (x, z) = (f.x, f.z);
    let (hw, hd) = (f.hw, f.hd);
    let rim = f.color;
    // outer rim walls
    g.box_aabb(Batch::Flat, x - hw, 0.0, z - hd, x + hw, 0.9, z - hd + 0.5, rim);
    g.box_aabb(Batch::Flat, x - hw, 0.0, z + hd - 0.5, x + hw, 0.9, z + hd, rim);
    g.box_aabb(Batch::Flat, x - hw, 0.0, z - hd, x - hw + 0.5, 0.9, z + hd, rim);
    g.box_aabb(Batch::Flat, x + hw - 0.5, 0.0, z - hd, x + hw, 0.9, z + hd, rim);
    // inner basin floor + water
    g.floor(Batch::Tex, x - hw + 0.5, z - hd + 0.5, x + hw - 0.5, z + hd - 0.5, 0.06, [0.55, 0.75, 0.8], 0.5, false);
    g.floor(Batch::Water, x - hw + 0.5, z - hd + 0.5, x + hw - 0.5, z + hd - 0.5, 0.72, [1.0; 3], 0.0, false);
    // center pillar + dish
    g.set_xf(Mat4::from_translation(vec3(x, 0.0, z)));
    g.cyl(Batch::Flat, 0.06, 1.7, 0.42, 0.3, shade(rim, 0.9), 12, false, true);
    g.cyl(Batch::Flat, 1.7, 1.82, 0.5, 1.05, shade(rim, 1.05), 14, true, false);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
    // jets
    for (jx, jz) in [
        (x + 0.8, z),
        (x - 0.8, z),
        (x, z + 0.8),
        (x, z - 0.8),
    ] {
        g.box_aabb(Batch::Emissive, jx - 0.05, 0.72, jz - 0.05, jx + 0.05, 1.78, jz + 0.05, [0.75, 0.95, 1.0]);
    }
    // upper dish water
    g.floor(Batch::Water, x - 0.9, z - 0.9, x + 0.9, z + 0.9, 1.8, [1.0; 3], 0.0, false);
}

pub fn add_escalator(g: &mut Geom, side: i8, zc: f32, half_w: f32) {
    // rises along +z on the left wall, along -z on the right wall
    let x0 = if side < 0 { half_w - 2.3 } else { -(half_w - 0.1) };
    let x1 = x0 + 2.2;
    let dir: f32 = if side < 0 { 1.0 } else { -1.0 };
    let z0 = zc - 8.0 * dir.max(0.0) - if dir > 0.0 { 0.0 } else { 0.0 };
    let z_start = zc - 8.0 * dir;
    let _ = z0;
    // truss sides
    let (za, zb) = if dir > 0.0 { (zc - 8.0, zc + 8.0) } else { (zc - 8.0, zc + 8.0) };
    g.box_aabb(Batch::Flat, x0, 0.0, za, x0 + 0.25, 6.2, zb, [0.55, 0.55, 0.6]);
    g.box_aabb(Batch::Flat, x1 - 0.25, 0.0, za, x1, 6.2, zb, [0.55, 0.55, 0.6]);
    // steps
    let steps = 16;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let y = 0.15 + t * 5.6;
        let zz = z_start + dir * (i as f32 * 1.0);
        g.box_aabb(Batch::Flat, x0 + 0.25, y - 0.3, zz - 0.42, x1 - 0.25, y, zz + 0.42, [0.68, 0.68, 0.72]);
        g.box_aabb(Batch::Emissive, x0 + 0.25, y - 0.02, zz - 0.44, x0 + 0.31, y + 0.02, zz + 0.44, [0.9, 0.85, 0.5]);
        g.box_aabb(Batch::Emissive, x1 - 0.31, y - 0.02, zz - 0.44, x1 - 0.25, y + 0.02, zz + 0.44, [0.9, 0.85, 0.5]);
    }
    // handrails (approximated with tilted boxes)
    for edge in [x0 + 0.1, x1 - 0.22] {
        let len = 17.0;
        let ang = (5.6f32).atan2(16.0) * dir;
        g.set_xf(
            Mat4::from_translation(vec3(edge, 3.4, zc))
                * Mat4::from_rotation_x(-ang)
                * Mat4::from_scale(vec3(0.12, 0.12, len)),
        );
        g.box_local(Batch::Emissive, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, [0.35, 0.2, 0.5]);
        g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
    }
    // upper landing
    let zl = zc + dir * 8.0;
    g.box_aabb(Batch::Flat, x0, 5.6, zl - 1.2, x1, 6.0, zl + 1.2, [0.6, 0.6, 0.64]);
}

pub fn add_second_floor(g: &mut Geom, side: i8, zc: f32, half_w: f32) {
    // fake mezzanine across the hall from the escalator
    let x0 = if side < 0 { 1.0 } else { -(half_w) };
    let x1 = if side < 0 { half_w } else { -1.0 };
    g.box_aabb(Batch::Flat, x0, 5.6, zc - 10.0, x1, 6.3, zc + 10.0, [0.78, 0.74, 0.7]);
    // railing
    for i in 0..9 {
        let rz = zc - 9.0 + i as f32 * 2.25;
        let rx = if side < 0 { x0 + 0.1 } else { x1 - 0.1 };
        g.box_aabb(Batch::Flat, rx - 0.05, 6.3, rz - 0.05, rx + 0.05, 7.3, rz + 0.05, [0.5, 0.5, 0.55]);
    }
    let rx = if side < 0 { x0 + 0.1 } else { x1 - 0.1 };
    g.box_aabb(Batch::Emissive, rx - 0.06, 7.25, zc - 10.0, rx + 0.06, 7.35, zc + 10.0, [0.9, 0.8, 0.6]);
    // closed storefront on the mezzanine
    g.box_aabb(Batch::Flat, x0 + 0.2, 6.3, zc - 4.0, x0 + 1.4, 8.6, zc + 4.0, [0.5, 0.45, 0.55]);
    g.box_aabb(Batch::Emissive, x0 + 1.4, 6.5, zc - 3.0, x0 + 1.46, 7.1, zc + 3.0, [0.6, 0.3, 0.5]);
}

pub fn add_storefront(g: &mut Geom, sf: &Storefront, half_w: f32) {
    let wall = sf.side as f32 * half_w; // wall plane
    let depth = 1.9;
    let back = wall + sf.side as f32 * depth;
    let (z0, z1) = (sf.z - 3.2, sf.z + 3.2);
    let open_h = 3.6f32;
    // recess shell
    g.box_tex(Batch::Tex, wall.min(back), 0.0, z0 - 0.3, wall.max(back), open_h + 0.6, z0, [0.7, 0.66, 0.62], 0.4);
    g.box_tex(Batch::Tex, wall.min(back), 0.0, z1, wall.max(back), open_h + 0.6, z1 + 0.3, [0.7, 0.66, 0.62], 0.4);
    g.box_tex(Batch::Tex, back - 0.15 * sf.side as f32 * sf.side as f32, 0.0, z0 - 0.3, back + 0.15, open_h + 0.6, z1 + 0.3, sf.color, 0.4);
    // interior floor + ceiling
    let (fx0, fx1) = if sf.side < 0 { (back, wall) } else { (wall, back) };
    g.floor(Batch::Tex, fx0.min(fx1), z0, fx0.max(fx1), z1, 0.01, [0.9, 0.88, 0.85], 0.5, true);
    g.floor(Batch::Flat, fx0.min(fx1), z0, fx0.max(fx1), z1, open_h + 0.55, [0.5, 0.48, 0.5], 0.0, true);
    // interior light strip
    g.box_aabb(Batch::Emissive, fx0.min(fx1) + 0.3, open_h + 0.45, sf.z - 1.2, fx0.max(fx1) - 0.3, open_h + 0.5, sf.z + 1.2, [1.0, 0.97, 0.88]);
    // glass front
    let gx = wall + sf.side as f32 * -0.06;
    g.wall_quad(Batch::Glass, gx, 0.05, z0 + 0.15, z1 - 0.15, open_h - 0.05, sf.side > 0, sf.glass);
    // interior contents by variant
    match sf.variant {
        0 => {
            // shelves
            for i in 0..3 {
                let zz = z0 + 0.9 + i as f32 * 1.6;
                g.box_aabb(Batch::Flat, fx0.min(fx1) + 0.15, 0.0, zz, fx0.max(fx1) - 0.15, 1.4, zz + 0.5, [0.55, 0.5, 0.45]);
                for j in 0..4 {
                    let bx = fx0.min(fx1) + 0.2 + j as f32 * 0.3;
                    g.box_aabb(Batch::Flat, bx, 1.4, zz + 0.05, bx + 0.2, 1.75, zz + 0.45, hsv((sf.name_idx as f32 * 0.17 + j as f32 * 0.31 + i as f32 * 0.11).fract(), 0.5, 0.8));
                }
            }
        }
        1 => {
            // mannequins
            for i in 0..2 {
                let zz = z0 + 1.2 + i as f32 * 2.4;
                let mx = (fx0 + fx1) / 2.0;
                add_mannequin(g, mx, zz, if i == 0 { 0.4 } else { -0.5 }, [0.85, 0.8, 0.75]);
            }
        }
        2 => {
            // racking
            for i in 0..4 {
                let zz = z0 + 0.7 + i as f32 * 1.3;
                g.box_aabb(Batch::Flat, fx0.min(fx1) + 0.2, 0.0, zz, fx0.max(fx1) - 0.2, 1.7, zz + 0.12, [0.45, 0.42, 0.5]);
                g.box_aabb(Batch::Flat, fx0.min(fx1) + 0.25, 0.35, zz - 0.02, fx0.max(fx1) - 0.25, 1.55, zz + 0.1, hsv((sf.name_idx as f32 * 0.23 + i as f32 * 0.27).fract(), 0.55, 0.75));
            }
        }
        _ => {
            // empty. sad. just a pedestal and a single object
            let mx = (fx0 + fx1) / 2.0;
            g.box_aabb(Batch::Flat, mx - 0.4, 0.0, sf.z - 0.4, mx + 0.4, 0.9, sf.z + 0.4, [0.75, 0.72, 0.7]);
            g.cyl(Batch::Emissive, 0.9, 1.15, 0.18, 0.18, hsv((sf.name_idx as f32 * 0.31).fract(), 0.6, 0.95), 10, true, false);
            let _ = xf(0.0, 0.0, 0.0);
        }
    }
}

pub fn add_mirror_frame(g: &mut Geom, x: f32, z: f32, w: f32, h: f32, y: f32, nx: f32, nz: f32) {
    // The frame box has to stay BEHIND the glass plane (the glass sits exactly
    // at x,z); if it pokes in front it hides the mirror entirely. `w` runs
    // across the normal, so which axis it spans depends on which way it faces.
    let (y0, y1) = (y - h / 2.0 - 0.15, y + h / 2.0 + 0.15);
    let top = y + h / 2.0;
    let across = w / 2.0 + 0.15;
    if nx.abs() >= nz.abs() {
        // side-wall mirror: spans z, thickness along x
        let (a, b) = (x - nx * 0.01, x - nx * 0.18);
        let (x0, x1) = (a.min(b), a.max(b));
        g.box_aabb(Batch::Flat, x0, y0, z - across, x1, y1, z + across, [0.35, 0.3, 0.4]);
        g.box_aabb(Batch::Emissive, x0, top + 0.2, z - 0.1, x1, top + 0.45, z + 0.1, [1.0, 0.9, 0.6]);
    } else {
        // end-on mirror: spans x, thickness along z
        let (a, b) = (z - nz * 0.01, z - nz * 0.18);
        let (z0, z1) = (a.min(b), a.max(b));
        g.box_aabb(Batch::Flat, x - across, y0, z0, x + across, y1, z1, [0.35, 0.3, 0.4]);
        g.box_aabb(Batch::Emissive, x - 0.1, top + 0.2, z0, x + 0.1, top + 0.45, z1, [1.0, 0.9, 0.6]);
    }
}

/// The stub wall a pier mirror hangs on: a block jutting from a side wall into
/// the corridor, with the glass on the face you walk up to. `z` is that front
/// face, so the block grows away from you along +z.
pub fn add_mirror_pier(g: &mut Geom, x: f32, z: f32, depth: f32, half_w: f32, ceiling: f32) {
    let side = x.signum();
    let (a, b) = (side * half_w, side * (half_w - depth));
    let (x0, x1) = (a.min(b), a.max(b));
    g.box_tex(Batch::Tex, x0, 0.0, z, x1, ceiling, z + 0.5, [0.95, 0.93, 0.9], 0.3);
}

pub fn add_hotdog(g: &mut Geom, x: f32, y: f32, z: f32) {
    g.box_aabb(Batch::Flat, x - 0.17, y, z - 0.06, x + 0.17, y + 0.07, z + 0.06, [0.87, 0.66, 0.35]);
    g.set_xf(Mat4::from_translation(vec3(x, y + 0.05, z)) * Mat4::from_rotation_x(1.5708));
    g.cyl(Batch::Flat, -0.19, 0.19, 0.045, 0.045, [0.78, 0.3, 0.22], 8, true, true);
    g.set_xf(Mat4::from_translation(vec3(0.0, 0.0, 0.0)));
    g.box_aabb(Batch::Emissive, x - 0.15, y + 0.06, z - 0.012, x + 0.15, y + 0.08, z + 0.012, [0.95, 0.8, 0.2]);
}