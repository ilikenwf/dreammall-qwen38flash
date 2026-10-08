//! The vaporwave sunset that lives behind the mirrors: grid, sun, Nyan Cat.
//! Drawn from a mirrored camera into a render target for each mirror.

use macroquad::prelude::*;

use crate::geom::baked_quad;
use crate::mats::Mats;
use crate::tex::TexLib;

pub struct Sunset;

fn billboard(pos: Vec3, cam: Vec3, sx: f32, sy: f32) -> Mat4 {
    let yaw = (cam.x - pos.x).atan2(cam.z - pos.z);
    Mat4::from_translation(pos)
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_scale(vec3(sx, sy, 1.0))
}

impl Sunset {
    pub fn new() -> Sunset {
        Sunset
    }

    /// Draw the sunset from this camera into `rt`.
    /// `forward` is the horizon direction (the mirror's facing normal).
    pub fn draw(
        &self,
        time: f32,
        cam_pos: Vec3,
        cam_target: Vec3,
        fov: f32,
        forward: Vec3,
        rt: &RenderTarget,
        mats: &Mats,
        tex: &TexLib,
    ) {
        let f = vec3(forward.x, 0.0, forward.z).normalize();
        let r = vec3(-f.z, 0.0, f.x);
        let up = vec3(0.0, 1.0, 0.0);

        set_camera(&Camera3D {
            position: cam_pos,
            target: cam_target,
            up,
            fovy: fov,
            z_near: 0.3,
            z_far: 1500.0,
            render_target: Some(rt.clone()),
            ..Default::default()
        });
        // the default material misbehaves inside render targets on this GL
        // stack; the custom sprite material (Texture*vcolor*uTint) covers
        // quads and vertex-colored lines alike.
        gl_use_material(&mats.sprite);
        mats.sprite.set_uniform("uTint", vec4(1.0, 1.0, 1.0, 1.0));

        // --- sky dome (huge gradient quad far ahead) ---
        let sky_c = cam_pos + f * 390.0 + up * 120.0;
        let sky_yaw = (-f.x).atan2(-f.z);
        let sky_xf = Mat4::from_translation(sky_c)
            * Mat4::from_rotation_y(sky_yaw)
            * Mat4::from_scale(vec3(1500.0, 1000.0, 1.0));
        draw_mesh(&baked_quad(sky_xf, &tex.sky, [1.0; 3]));

        // --- neon grid ---
        let span = 170.0f32;
        let far = 380.0f32;
        let mut k = -42i32;
        while k <= 42 {
            let off = k as f32 * 4.0;
            let c = if k % 5 == 0 {
                Color::new(1.0, 0.25, 0.9, 1.0)
            } else {
                Color::new(0.55, 0.12, 0.6, 1.0)
            };
            let base = cam_pos + r * off + up * (0.02 - cam_pos.y);
            draw_line_3d(base + f * 20.0, base + f * far, c);
            k += 1;
        }
        let mut j = 15i32;
        while (j as f32) < far / 4.0 {
            let along = j as f32 * 4.0;
            let fade = 1.0 - (along / far).max(0.0) * 0.6;
            let c = if j % 5 == 0 {
                Color::new(0.2 * fade, 0.9 * fade, 1.0 * fade, 1.0)
            } else {
                Color::new(0.1 * fade, 0.5 * fade, 0.75 * fade, 1.0)
            };
            let base = cam_pos + f * along + up * (0.02 - cam_pos.y);
            draw_line_3d(base + r * -span, base + r * span, c);
            j += 1;
        }

        let sun_pos = cam_pos + f * 320.0 + up * 26.0;
        draw_mesh(&baked_quad(
            billboard(sun_pos, cam_pos, 110.0, 110.0),
            &tex.sun,
            [1.0; 3],
        ));

        // --- nyan cat + rainbow trail ---
        let cycle = 42.0f32;
        let t = (time % cycle) / cycle;
        let nu = -260.0 + t * 520.0;
        let nv = 34.0 + (time * 0.7).sin() * 4.0;
        let trail_colors = [
            [1.0f32, 0.2, 0.2],
            [1.0, 0.55, 0.1],
            [1.0, 0.95, 0.2],
            [0.2, 0.9, 0.3],
            [0.2, 0.5, 1.0],
            [0.7, 0.25, 0.95],
        ];
        for (i, c) in trail_colors.iter().enumerate() {
            let tu = nu - 14.0 - i as f32 * 14.0;
            let pos = cam_pos + f * tu + up * (nv + 3.0 - i as f32 * 0.0);
            let yaw = (cam_pos.x - pos.x).atan2(cam_pos.z - pos.z);
            let xf = Mat4::from_translation(pos)
                * Mat4::from_rotation_y(yaw)
                * Mat4::from_scale(vec3(14.5, 1.6, 1.0));
            mats.sprite.set_uniform("uTint", vec4(c[0], c[1], c[2], 1.0));
            draw_mesh(&baked_quad(xf, &tex.white, [1.0; 3]));
        }

        gl_use_default_material();
        set_default_camera();
    }
}
