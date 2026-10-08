//! Procedural canvas textures: tiles, signs, posters, Nyan Cat, the sun.
//! Everything is drawn at runtime — the game ships with zero image files.

use std::collections::HashMap;

use macroquad::prelude::*;

pub struct TexLib {
                    pub sky: Texture2D,
    pub posters: Vec<Texture2D>,
        pub sun: Texture2D,
    bold: Font,
    sign_cache: HashMap<u64, Texture2D>,
    keep: Vec<RenderTarget>,
    pub white: Texture2D,
}

fn fill_rect(x: f32, y: f32, w: f32, h: f32, c: Color) {
    draw_rectangle(x, y, w, h, c);
}

fn noise_px(x: f32, y: f32) -> f32 {
    let n = (x * 12.9898 + y * 78.233).sin() * 43758.5453;
    n - n.floor()
}

/// Draw into a fresh render target; returns its texture (kept alive).
fn paint<F: FnOnce()>(keep: &mut Vec<RenderTarget>, w: u16, h: u16, nearest: bool, f: F) -> Texture2D {
    let rt = render_target(w as u32, h as u32);
    rt.texture.set_filter(if nearest {
        FilterMode::Nearest
    } else {
        FilterMode::Linear
    });
    let cam = Camera2D {
        render_target: Some(rt.clone()),
        ..Default::default()
    };
    set_camera(&cam);
    clear_background(Color::new(0.0, 0.0, 0.0, 1.0));
    f();
    set_default_camera();
    let tex = rt.texture.weak_clone();
    keep.push(rt);
    tex
}

impl TexLib {
    pub fn new(bold: Font) -> TexLib {
        let mut keep = Vec::new();
        let sky = Self::build_sky();
        let posters = (0..crate::layout::POSTERS.len())
            .map(|i| {
                let b = bold.clone();
                paint(&mut keep, 256, 352, false, move || Self::paint_poster(i, &b))
            })
            .collect();
        let sun = Self::build_sun();
        let white =
            Texture2D::from_image(&Image::gen_image_color(1, 1, Color::new(1.0, 1.0, 1.0, 1.0)));
        TexLib {
            sky,
            posters,
            sun,
            white,
            sign_cache: HashMap::new(),
            keep,
            bold,
        }
    }

    fn paint_poster(idx: usize, bold: &Font) {
        let hue = ((idx as f32) * 0.37).fract();
        let bg = crate::util::hsv(hue, 0.55, 0.85);
        clear_background(Color::new(bg[0], bg[1], bg[2], 1.0));
        let shapes = 3 + idx % 3;
        for i in 0..shapes {
            let h2 = (hue + 0.3 * (i as f32 + 1.0)).fract();
            let c = crate::util::hsv(h2, 0.6, 0.9);
            let col = Color::new(c[0], c[1], c[2], 0.85);
            let x = 30.0 + ((idx * 53 + i * 97) % 140) as f32;
            let y = 40.0 + ((idx * 31 + i * 61) % 160) as f32;
            match i % 3 {
                0 => draw_circle(x, y, 26.0 + (i as f32 * 9.0), col),
                1 => fill_rect(x, y, 60.0 + i as f32 * 14.0, 34.0, col),
                _ => draw_circle_lines(x, y, 40.0, 3.0, col),
            }
        }
        fill_rect(0.0, 250.0, 256.0, 102.0, Color::new(0.05, 0.05, 0.08, 0.85));
        let text = crate::layout::POSTERS[idx];
        let size = if text.len() > 22 { 17 } else { 22 };
        let s = measure_text(text, Some(bold), size, 1.0);
        draw_text_ex(
            text,
            (256.0 - s.width) / 2.0,
            285.0,
            TextParams {
                font: Some(bold),
                font_size: size,
                font_scale: 1.0,
                font_scale_aspect: 1.0,
                rotation: 0.0,
                color: Color::new(1.0, 0.95, 0.8, 1.0),
            },
        );
    }

    /// Sign texture: dark plate, glowing text. Cached by content hash.
    pub fn sign(&mut self, text: &str, bg: [f32; 3], fg: [f32; 3]) -> Texture2D {
        let mut key: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text.as_bytes() {
            key = (key ^ *b as u64).wrapping_mul(0x100000001b3);
        }
        for c in bg.iter().chain(fg.iter()) {
            key = (key ^ (c.to_be_bytes()[3] as u64)).wrapping_mul(0x100000001b3);
        }
        if let Some(t) = self.sign_cache.get(&key) {
            return t.weak_clone();
        }
        let bold = self.bold.clone();
        let text = text.to_string();
        let tex = paint(&mut self.keep, 512, 128, false, move || {
            Self::paint_sign(&text, bg, fg, &bold)
        });
        self.sign_cache.insert(key, tex.weak_clone());
        tex
    }

    fn paint_sign(text: &str, bg: [f32; 3], fg: [f32; 3], bold: &Font) {
        clear_background(Color::new(bg[0], bg[1], bg[2], 1.0));
        fill_rect(6.0, 6.0, 500.0, 116.0, Color::new(0.06, 0.05, 0.09, 0.9));
        fill_rect(
            10.0,
            10.0,
            492.0,
            108.0,
            Color::new(bg[0] * 0.8, bg[1] * 0.8, bg[2] * 0.8, 1.0),
        );
        let mut size = 64u16;
        let mut s = measure_text(text, Some(bold), size, 1.0);
        while s.width > 470.0 && size > 14 {
            size -= 4;
            s = measure_text(text, Some(bold), size, 1.0);
        }
        let col = Color::new(fg[0].min(1.0), fg[1].min(1.0), fg[2].min(1.0), 1.0);
        draw_text_ex(
            text,
            (512.0 - s.width) / 2.0,
            (128.0 - s.height) / 2.0,
            TextParams {
                font: Some(bold),
                font_size: size,
                font_scale: 1.0,
                font_scale_aspect: 1.0,
                rotation: 0.0,
                color: col,
            },
        );
    }
    fn build_sun() -> Texture2D {
        let mut img = Image::gen_image_color(128, 128, Color::new(0.0, 0.0, 0.0, 0.0));
        for y in 0..128u32 {
            for x in 0..128u32 {
                let dx = (x as f32 - 64.0) / 64.0;
                let dy = (y as f32 - 64.0) / 64.0;
                let d = (dx * dx + dy * dy).sqrt();
                let t = y as f32 / 128.0;
                let col = crate::util::hsv(0.13 - t * 0.11, 0.85, 1.0);
                let a = if d > 1.0 {
                    0.0
                } else {
                    let slit = if y > 78 && (y % 8) < 3 + (y - 78) / 10 {
                        0.0
                    } else {
                        1.0
                    };
                    slit * ((1.0 - d) * 4.0).min(1.0)
                };
                img.set_pixel(x, y, Color::new(col[0], col[1], col[2], a));
            }
        }
        Texture2D::from_image(&img)
    }

    /// Vaporwave sky: purple→pink→orange gradient with stars, for the mirror world.
    fn build_sky() -> Texture2D {
        let mut img = Image::gen_image_color(64, 256, Color::new(0.1, 0.05, 0.2, 1.0));
        for y in 0..256u32 {
            let t = y as f32 / 255.0;
            let c = if t < 0.55 {
                let u = t / 0.55;
                crate::util::mixc(crate::util::hsv(0.74, 0.85, 0.16), crate::util::hsv(0.86, 0.75, 0.55), u)
            } else {
                let u = (t - 0.55) / 0.45;
                crate::util::mixc(crate::util::hsv(0.86, 0.75, 0.55), crate::util::hsv(0.06, 0.85, 1.0), u)
            };
            for x in 0..64u32 {
                let mut col = Color::new(c[0], c[1], c[2], 1.0);
                // stars in the upper band
                if t < 0.4 {
                    let n = noise_px(x as f32 * 13.7, y as f32 * 7.3);
                    if n > 0.985 {
                        col = Color::new(1.0, 0.95, 1.0, 1.0);
                    }
                }
                img.set_pixel(x, y, col);
            }
        }
        Texture2D::from_image(&img)
    }
}
