//! DREAM MALL — a small procedurally generated 3D dream-sim (Phase 1).
//!
//! You are trapped in a strange shopping mall, forced to keep walking forward.
//! Somewhere there is a penny, and a fountain. Somewhere else there is a hotdog.

mod game;
mod geom;
mod layout;
mod mats;
mod music;
mod player;
mod props;
mod rng;
mod sunset;
mod tex;
mod util;
mod world;

use std::path::PathBuf;

use macroquad::audio::{self, Sound};
use macroquad::prelude::*;
use macroquad::texture::RenderTargetParams;
use macroquad::ui::root_ui;

use game::{Game, Held, Phase};
use geom::{baked_quad, Batch};
use layout::LOOP_LEN;
use mats::Mats;
use music::Music;
use sunset::Sunset;
use tex::TexLib;
use world::World;

const MIRROR_W: u32 = 512;
const MIRROR_H: u32 = 384;
const LOOK_SENS: f32 = 0.0026;

fn window_conf() -> Conf {
    Conf {
        window_title: "DREAM MALL".to_string(),
        window_width: 1280,
        window_height: 800,
        window_resizable: true,
        sample_count: 1,
        ..Default::default()
    }
}

fn bind_mirrors(world: &mut World, rts: &[RenderTarget]) {
    for (i, md) in world.mirrors.iter_mut().enumerate() {
        md.mesh.texture = Some(rts[i % rts.len()].texture.weak_clone());
        // GL render targets are bottom-up; baked quads carry top-down UVs.
        for v in md.mesh.vertices.iter_mut() {
            v.uv.y = 1.0 - v.uv.y;
        }
    }
}

fn tp(font: &Font, font_size: u16, color: Color) -> TextParams<'_> {
    TextParams {
        font: Some(font),
        font_size,
        font_scale: 1.0,
        font_scale_aspect: 1.0,
        rotation: 0.0,
        color,
    }
}

fn asset_bytes(rel: &str) -> Vec<u8> {
    let mut roots: Vec<PathBuf> = vec![PathBuf::from(".")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
            roots.push(dir.join("../../"));
            roots.push(dir.join("../../../"));
        }
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    for r in roots {
        let p = r.join(rel);
        if let Ok(b) = std::fs::read(&p) {
            return b;
        }
    }
    panic!("asset not found: {rel}");
}

#[derive(PartialEq, Clone, Copy)]
enum AppState {
    Menu,
    Playing,
    Paused,
    Won,
}

struct Audio {
    music_normal: Option<Sound>,
    music_flight: Option<Sound>,
    step_l: Option<Sound>,
    step_r: Option<Sound>,
    flight_fade: f32,
    flying_now: bool,
}

impl Audio {
    fn disabled() -> Audio {
        Audio {
            music_normal: None,
            music_flight: None,
            step_l: None,
            step_r: None,
            flight_fade: 0.0,
            flying_now: false,
        }
    }
}

fn pcm16(samples: &[f32]) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32000.0) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    pcm
}

fn wav_bytes(pcm: &[u8], rate: u32, channels: u16) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + pcm.len());
    let len = 36 + pcm.len() as u32;
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&len.to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&channels.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    w.extend_from_slice(&(channels * 2).to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(pcm);
    w
}

/// Render the whole muzak loop to 16-bit PCM WAV bytes.
fn render_music_wav(seed: u64, flight: bool) -> Vec<u8> {
    let rate = 44100.0f32;
    let n = (rate * 64.0) as usize;
    let mut m = Music::new(rate, seed);
    m.set_flight(flight);
    if flight {
        m.master *= 0.85;
    }
    let mut out = Vec::with_capacity(n * 2);
    let mut block = vec![0.0f32; 2048 * 2];
    while out.len() < n * 2 {
        m.fill(&mut block);
        out.extend_from_slice(&block);
    }
    out.truncate(n * 2);
    wav_bytes(&pcm16(&out), 44100, 2)
}

fn render_step_wav(pan: f32) -> Vec<u8> {
    let mut m = Music::new(44100.0, 99);
    m.footstep(pan);
    let mut out = Vec::new();
    let mut block = vec![0.0f32; 2048 * 2];
    for _ in 0..3 {
        m.fill(&mut block);
        out.extend_from_slice(&block);
    }
    wav_bytes(&pcm16(&out), 44100, 2)
}

#[macroquad::main(window_conf)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let smoke = args.iter().any(|a| a == "--smoke");
    let shot: Option<String> = args
        .iter()
        .position(|a| a == "--shot")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let font_bytes = asset_bytes("assets/LiberationSans-Bold.ttf");
    let bold = macroquad::text::load_ttf_font_from_bytes(&font_bytes).expect("font");

    let mats = Mats::new();
    let mut tex = TexLib::new(bold.clone());
    let sunset = Sunset::new();

    let seed = args
        .iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() ^ (d.subsec_nanos() as u64) << 32)
                .unwrap_or(0xC0FFEE)
        });

    let mut game = Game::new(seed);
    let mut world = World::build(game.layout.clone(), &mut tex);

    // mirror render targets (pool, with depth for the sunset's 3d)
    let mirror_rts: Vec<RenderTarget> = (0..16)
        .map(|_| {
            let rt = render_target_ex(
                MIRROR_W,
                MIRROR_H,
                RenderTargetParams {
                    sample_count: 1,
                    depth: true,
                },
            );
            rt.texture.set_filter(FilterMode::Linear);
            rt
        })
        .collect();

    bind_mirrors(&mut world, &mirror_rts);

    // audio
    let mut audio = Audio::disabled();
    if !smoke && shot.is_none() {
        audio.music_normal = audio::load_sound_from_bytes(&render_music_wav(seed, false))
            .await
            .ok();
        audio.music_flight = audio::load_sound_from_bytes(&render_music_wav(seed, true))
            .await
            .ok();
        audio.step_l = audio::load_sound_from_bytes(&render_step_wav(-0.4)).await.ok();
        audio.step_r = audio::load_sound_from_bytes(&render_step_wav(0.4)).await.ok();
        if let Some(s) = &audio.music_normal {
            audio::play_sound(
                s,
                audio::PlaySoundParams {
                    looped: true,
                    volume: 1.0,
                },
            );
        }
    }

    let mut state = if smoke {
        AppState::Playing
    } else {
        AppState::Menu
    };
    let mut grabbed = false;
    if smoke || shot.is_some() {
        show_mouse(false);
    }
    let mut shot_n = 0u32;
    let mut smoke_step = 0u32;
    let mut smoke_timer = 0.0f32;
    let mut exit = false;

    loop {
        let dt = get_frame_time().min(0.1);
        let time = get_time() as f32;

        // ---------- input & state ----------
        match state {
            AppState::Menu => {
                if is_mouse_button_released(MouseButton::Left) {
                    state = AppState::Playing;
                    set_cursor_grab(true);
                    grabbed = true;
                    show_mouse(false);
                }
            }
            AppState::Playing => {
                if smoke || shot.is_some() {
                    game.update(0.0, 0.0, 0.0, dt);
                }
                if smoke {
                    run_smoke_step(
                        &mut game,
                        &mut world,
                        &mut tex,
                        &mirror_rts,
                        &mut smoke_step,
                        &mut smoke_timer,
                        dt,
                    );
                    if smoke_step == 100 {
                        println!("SMOKE OK");
                        return;
                    }
                    if smoke_step > 100 {
                        println!("SMOKE FAIL at step {smoke_step}");
                        return;
                    }
                } else if shot.is_none() {
                    if is_key_pressed(KeyCode::Escape) {
                        state = AppState::Paused;
                        set_cursor_grab(false);
                        grabbed = false;
                        show_mouse(true);
                    }
                    if grabbed {
                        let d = mouse_delta_position();
                        game.player.look(d.x * LOOK_SENS, d.y * LOOK_SENS);
                        let fwd =
                            (is_key_down(KeyCode::W) || is_key_down(KeyCode::Up)) as i32;
                        let strafe =
                            (is_key_down(KeyCode::D) as i32) - (is_key_down(KeyCode::A) as i32);
                        let up = (is_key_down(KeyCode::Space) as i32
                            - is_key_down(KeyCode::LeftControl) as i32)
                            as f32;
                        game.update(fwd as f32, strafe as f32, up, dt);
                        if is_key_pressed(KeyCode::E) {
                            game.interact();
                        }
                        if is_key_down(KeyCode::E) {
                            game.hold_eat(dt);
                        }
                        if is_mouse_button_pressed(MouseButton::Left) {
                            game.throw_penny();
                        }
                        if game.phase == Phase::Won {
                            state = AppState::Won;
                            set_cursor_grab(false);
                            grabbed = false;
                            show_mouse(true);
                        }
                    }
                }
            }
            AppState::Paused => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    state = AppState::Playing;
                    set_cursor_grab(true);
                    grabbed = true;
                    show_mouse(false);
                }
            }
            AppState::Won => {
                let (w, h) = (screen_width(), screen_height());
                if root_ui().button(vec2(w / 2.0 - 150.0, h / 2.0 + 40.0), "Go back to sleep?") {
                    game = Game::new(seed ^ (game.loop_k as u64 + 0xBEEF));
                    world = World::build(game.layout.clone(), &mut tex);
                    bind_mirrors(&mut world, &mirror_rts);
                    state = AppState::Playing;
                    set_cursor_grab(true);
                    grabbed = true;
                    show_mouse(false);
                }
                if root_ui().button(vec2(w / 2.0 - 150.0, h / 2.0 + 90.0), "Exit") {
                    exit = true;
                }
            }
        }

        // footstep sounds
        while let Some(pan) = game.footfalls.pop() {
            if let Some(s) = if pan < 0.0 { &audio.step_l } else { &audio.step_r } {
                audio::play_sound(
                    s,
                    audio::PlaySoundParams {
                        looped: false,
                        volume: 0.9,
                    },
                );
            }
        }

        // flight music crossfade
        let flying = game.phase == Phase::Flight;
        if flying && !audio.flying_now {
            audio.flying_now = true;
            if let Some(s) = &audio.music_flight {
                audio::play_sound(
                    s,
                    audio::PlaySoundParams {
                        looped: true,
                        volume: 0.0,
                    },
                );
            }
        }
        if flying {
            audio.flight_fade = (audio.flight_fade + dt * 0.8).min(1.0);
            if let Some(s) = &audio.music_normal {
                audio::set_sound_volume(s, 1.0 - audio.flight_fade);
            }
            if let Some(s) = &audio.music_flight {
                audio::set_sound_volume(s, audio.flight_fade);
            }
        }

        // ---------- render ----------
        let fog = if flying {
            [0.55, 0.25, 0.5]
        } else {
            game.layout.fog
        };
        let density = if flying { 0.012 } else { 0.02 };
        clear_background(Color::new(fog[0], fog[1], fog[2], 1.0));

        if state != AppState::Menu {
            let p = &game.player;
            let dir = p.look_dir();
            let roll = if flying { (time * 0.4).sin() * 0.06 } else { 0.0 };
            let cam = Camera3D {
                position: p.pos,
                target: p.pos + dir,
                up: vec3(roll, 1.0, 0.0),
                fovy: if flying { 1.42 } else { 1.22 },
                ..Default::default()
            };
            set_camera(&cam);
            mats.set_fog(fog, density, game.layout.light_tint, p.pos);
            mats.water.set_uniform("uTime", time % 1000.0);

            // world batches
            for (kind, mesh) in &world.batches {
                if flying && matches!(kind, Batch::Ceiling | Batch::CeilingGlow) {
                    continue; // no roof in the dream flight
                }
                let m = match kind {
                    Batch::Flat => &mats.world_flat,
                    Batch::Ceiling => &mats.world_flat,
                    Batch::CeilingGlow => &mats.emissive,
                    Batch::Tex => &mats.world_tex,
                    Batch::Emissive => &mats.emissive,
                    Batch::Glass => &mats.glass,
                    Batch::Water => &mats.water,
                };
                gl_use_material(m);
                draw_mesh(mesh);
            }

            // signs
            gl_use_material(&mats.sign);
            for s in world.signs.iter().chain(world.storefront_signs.iter()) {
                draw_mesh(s);
            }

            // mirrors: render the sunset from the reflected camera, then the quad
            for (i, md) in world.mirrors.iter().enumerate() {
                let dx = md.x - p.pos.x;
                let dz = md.z - p.pos.z;
                let dist2 = dx * dx + dz * dz;
                if dist2 > 55.0 * 55.0 {
                    continue;
                }
                let facing = vec3(md.facing_x, 0.0, 0.0);
                if facing.dot(vec3(-dx, 0.0, -dz)) < 0.0 {
                    continue; // player is behind the mirror plane
                }
                let rt = &mirror_rts[i % mirror_rts.len()];
                let mx = md.x;
                let rpos = vec3(2.0 * mx - p.pos.x, p.pos.y, p.pos.z);
                let rtarget =
                    vec3(2.0 * mx - (p.pos.x + dir.x), p.pos.y + dir.y, p.pos.z + dir.z);
                sunset.draw(time, rpos, rtarget, cam.fovy, facing, rt, &mats, &tex);
                set_camera(&cam);
                gl_use_material(&mats.mirror);
                draw_mesh(&md.mesh);
            }
            gl_use_default_material();

            // penny
            let penny_pos = match game.penny {
                game::PennyState::OnFloor { x, z } | game::PennyState::Resting { x, z } => {
                    Some(vec3(x, 0.05, z))
                }
                game::PennyState::Thrown { x, y, z, .. } => Some(vec3(x, y, z)),
                game::PennyState::Held => Some(p.pos + dir * 0.55 + vec3(0.22, -0.18, 0.0)),
                game::PennyState::InFountain => None,
            };
            if let Some(pos) = penny_pos {
                gl_use_material(&mats.emissive);
                let xf = Mat4::from_translation(pos)
                    * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
                    * Mat4::from_scale(vec3(0.28, 0.28, 1.0));
                draw_mesh(&baked_quad(xf, &tex.white, [1.0, 0.62, 0.3]));
            }

            // hotdog (on its table, or held)
            let hotdog_at = match game.hotdog {
                game::HotdogState::OnTable => {
                    Some(vec3(game.layout.hotdog[0], game.layout.hotdog_y, game.layout.hotdog[1]))
                }
                game::HotdogState::Held => Some(p.pos + dir * 0.55 + vec3(-0.25, -0.2, 0.0)),
                game::HotdogState::Eaten => None,
            };
            if let Some(hp) = hotdog_at {
                let mut g = geom::Geom::new();
                props::add_hotdog(&mut g, hp.x, hp.y, hp.z);
                gl_use_material(&mats.world_flat);
                for (_, mesh) in g.finish() {
                    draw_mesh(&mesh);
                }
            }
            gl_use_default_material();
            set_default_camera();

            // ---------- HUD ----------
            let (sw, sh) = (screen_width(), screen_height());
            draw_circle(sw / 2.0, sh / 2.0, 2.0, Color::new(1.0, 1.0, 1.0, 0.6));
            if let Some(prompt) = &game.prompt {
                let s = measure_text(prompt, Some(&bold), 22, 1.0);
                draw_text_ex(
                    prompt,
                    (sw - s.width) / 2.0,
                    sh * 0.78,
                    tp(&bold, 22, Color::new(1.0, 1.0, 0.9, 0.9)),
                );
            }
            let held_msg = match game.holding {
                Some(Held::Penny) => Some("you are holding: a penny"),
                Some(Held::Hotdog) => Some("you are holding: a hotdog (hold E)"),
                None => None,
            };
            if let Some(msg) = held_msg {
                draw_text_ex(msg, 20.0, 34.0, tp(&bold, 18, Color::new(1.0, 1.0, 1.0, 0.85)));
            }
            if game.eat_progress > 0.0 && game.holding == Some(Held::Hotdog) {
                let w = 200.0 * (game.eat_progress / 1.5).min(1.0);
                draw_rectangle(
                    sw / 2.0 - 100.0,
                    sh * 0.82,
                    w,
                    8.0,
                    Color::new(0.9, 0.4, 0.3, 0.9),
                );
            }
            if game.loop_k > 0 {
                draw_text_ex(
                    &format!("dream pass {n}", n = game.loop_k),
                    20.0,
                    sh - 20.0,
                    tp(&bold, 14, Color::new(1.0, 1.0, 1.0, 0.5)),
                );
            }
            if game.fade > 0.0 {
                draw_rectangle(
                    0.0,
                    0.0,
                    sw,
                    sh,
                    Color::new(0.02, 0.0, 0.05, game.fade.min(1.0)),
                );
            }
            if flying {
                draw_rectangle(
                    0.0,
                    0.0,
                    sw,
                    sh,
                    Color::new(0.8, 0.2, 0.7, 0.06 + 0.03 * (time * 1.3).sin()),
                );
            }
        }

        // ---------- overlays ----------
        match state {
            AppState::Menu => {
                let (sw, sh) = (screen_width(), screen_height());
                draw_menu_background(time);
                let title = "DREAM MALL";
                let s = measure_text(title, Some(&bold), 84, 1.0);
                draw_text_ex(
                    title,
                    (sw - s.width) / 2.0,
                    sh * 0.35,
                    tp(&bold, 84, Color::new(1.0, 0.9, 1.0, 1.0)),
                );
                let sub = "you have always been here";
                let s2 = measure_text(sub, Some(&bold), 22, 1.0);
                draw_text_ex(
                    sub,
                    (sw - s2.width) / 2.0,
                    sh * 0.35 + 60.0,
                    tp(&bold, 22, Color::new(1.0, 0.85, 0.9, 0.9)),
                );
                let start = "click to fall asleep";
                let s3 = measure_text(start, Some(&bold), 28, 1.0);
                let pulse = 0.6 + 0.4 * (time * 2.0).sin();
                draw_text_ex(
                    start,
                    (sw - s3.width) / 2.0,
                    sh * 0.62,
                    tp(&bold, 28, Color::new(1.0, 1.0, 1.0, pulse)),
                );
                let help =
                    "W forward   A/D strafe   mouse look   E use   click throw   Esc pause";
                let s4 = measure_text(help, Some(&bold), 18, 1.0);
                draw_text_ex(
                    help,
                    (sw - s4.width) / 2.0,
                    sh * 0.75,
                    tp(&bold, 18, Color::new(1.0, 1.0, 1.0, 0.8)),
                );
                let help2 = "you cannot walk backward. the mall is that kind of place.";
                let s5 = measure_text(help2, Some(&bold), 18, 1.0);
                draw_text_ex(
                    help2,
                    (sw - s5.width) / 2.0,
                    sh * 0.75 + 26.0,
                    tp(&bold, 18, Color::new(1.0, 1.0, 1.0, 0.6)),
                );
            }
            AppState::Paused => {
                let (sw, sh) = (screen_width(), screen_height());
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.02, 0.6));
                let t = "paused — click to keep walking";
                let s = measure_text(t, Some(&bold), 30, 1.0);
                draw_text_ex(
                    t,
                    (sw - s.width) / 2.0,
                    sh / 2.0,
                    tp(&bold, 30, Color::new(1.0, 1.0, 1.0, 0.95)),
                );
            }
            AppState::Won => {
                let (sw, sh) = (screen_width(), screen_height());
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.98, 0.96, 0.9, 0.96));
                let t = "You woke up, congratulations.";
                let s = measure_text(t, Some(&bold), 46, 1.0);
                draw_text_ex(
                    t,
                    (sw - s.width) / 2.0,
                    sh * 0.38,
                    tp(&bold, 46, Color::new(0.1, 0.08, 0.12, 1.0)),
                );
            }
            _ => {}
        }
        // ---------- scripted screenshots ----------
        if let Some(dir) = &shot {
            shot_n += 1;
            use std::f32::consts::FRAC_PI_2;
            let f0 = game.layout.fountains[0];
            match shot_n {
                6 => {
                    get_screen_data().export_png(&format!("{dir}/01_menu.png"));
                    state = AppState::Playing;
                    show_mouse(false);
                    game.fade = 0.0;
                    game.player.pos = vec3(f0.x, player::EYE, f0.z - f0.hd - 3.0);
                    game.player.yaw = 0.0;
                    game.player.pitch = 0.0;
                }
                16 => {
                    get_screen_data().export_png(&format!("{dir}/02_atrium.png"));
                    let sf = game.layout.storefronts[0];
                    game.player.pos = vec3(0.0, player::EYE, sf.z);
                    game.player.yaw = if sf.side < 0 { -FRAC_PI_2 } else { FRAC_PI_2 };
                }
                26 => {
                    get_screen_data().export_png(&format!("{dir}/03_storefronts.png"));
                    let m = game.layout.mirrors[0];
                    game.player.pos = vec3(0.9, player::EYE, m.z);
                    game.player.yaw = if m.x < 0.0 { -FRAC_PI_2 } else { FRAC_PI_2 };
                }
                36 => {
                    get_screen_data().export_png(&format!("{dir}/04_mirror.png"));
                    game.phase = Phase::Flight;
                    game.player.flying = true;
                    game.player.pos = vec3(f0.x, 5.5, f0.z - f0.hd - 4.0);
                    game.player.yaw = 0.0;
                    game.player.pitch = 0.25;
                }
                46 => {
                    get_screen_data().export_png(&format!("{dir}/05_flight.png"));
                    println!("SHOTS OK");
                    return;
                }
                _ => {}
            }
        }

        next_frame().await;
        if exit {
            break;
        }
    }
}

fn draw_menu_background(time: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let bands = 40;
    for b in 0..bands {
        let t = b as f32 / (bands - 1) as f32;
        let c = if t < 0.55 {
            let u = t / 0.55;
            util::mixc(
                util::hsv(0.74, 0.85, 0.16),
                util::hsv(0.86, 0.75, 0.55),
                u,
            )
        } else {
            let u = (t - 0.55) / 0.45;
            util::mixc(
                util::hsv(0.86, 0.75, 0.55),
                util::hsv(0.06, 0.85, 1.0),
                u,
            )
        };
        draw_rectangle(
            0.0,
            b as f32 * sh / bands as f32,
            sw,
            sh / bands as f32 + 1.0,
            Color::new(c[0], c[1], c[2], 1.0),
        );
    }
    // drifting grid
    let y0 = sh * 0.62;
    for i in 0..14 {
        let t = i as f32 / 13.0;
        let y = y0 + (sh - y0) * t * t;
        draw_line(0.0, y, sw, y, 1.0, Color::new(1.0, 0.25, 0.9, 0.35));
    }
    let off = (time * 30.0) % 80.0;
    let mut x = -80.0 + off;
    while x < sw + 80.0 {
        draw_line(
            x,
            y0,
            x + (x - sw / 2.0) * 1.4,
            sh,
            1.0,
            Color::new(0.2, 0.9, 1.0, 0.3),
        );
        x += 80.0;
    }
}

/// Scripted playthrough for `--smoke` (headless verification under Xvfb).
fn run_smoke_step(
    game: &mut Game,
    world: &mut World,
    tex: &mut TexLib,
    rts: &[RenderTarget],
    step: &mut u32,
    timer: &mut f32,
    dt: f32,
) {
    *timer += dt;
    let advance = |s: u32, timer: &mut f32, step: &mut u32| {
        *step = s;
        *timer = 0.0;
    };
    match *step {
        0 => {
            // pick up the penny
            let (px, pz) = (game.layout.penny[0], game.layout.penny[1]);
            game.player.pos.x = px;
            game.player.pos.z = pz - 1.0;
            game.interact();
            if game.holding == Some(Held::Penny) {
                advance(1, timer, step);
            } else {
                *step = 999;
            }
        }
        1 => {
            // throw it into fountain 0
            let f = game.layout.fountains[0];
            game.player.pos.x = f.x;
            game.player.pos.z = f.z - (f.hd + 2.5);
            game.player.yaw = 0.0;
            game.player.pitch = 0.15;
            game.throw_penny();
            advance(2, timer, step);
        }
        2 => {
            if game.phase == Phase::Won {
                if game.win_path == Some("penny-in-fountain") {
                    advance(3, timer, step);
                } else {
                    *step = 998;
                }
            } else if *timer > 8.0 {
                *step = 997;
            }
        }
        3 => {
            // restart the dream, find the hotdog
            let s = game.seed ^ 0x5EED;
            *game = Game::new(s);
            *world = World::build(game.layout.clone(), tex);
            bind_mirrors(world, rts);
            let (hx, hz) = (game.layout.hotdog[0], game.layout.hotdog[1]);
            game.player.pos.x = hx + 1.0;
            game.player.pos.z = hz;
            game.interact();
            if game.holding == Some(Held::Hotdog) {
                advance(4, timer, step);
            } else {
                *step = 996;
            }
        }
        4 => {
            game.hold_eat(1.6);
            if game.phase == Phase::Flight {
                advance(5, timer, step);
            } else {
                *step = 995;
            }
        }
        5 => {
            // dive into a fountain
            let f = game.layout.fountains[0];
            game.player.pos.x = f.x;
            game.player.pos.z = f.z;
            game.player.pos.y = 3.0;
            advance(6, timer, step);
        }
        6 => {
            game.update(0.0, 0.0, -1.0, 1.0 / 60.0);
            if game.phase == Phase::Won {
                if game.win_path == Some("fountain-dive") {
                    advance(7, timer, step);
                } else {
                    *step = 994;
                }
            } else if *timer > 10.0 {
                *step = 993;
            }
        }
        7 => {
            // loop wrap: walk through the seam, dream mutates
            let s = game.seed ^ 0xA11CE;
            *game = Game::new(s);
            *world = World::build(game.layout.clone(), tex);
            bind_mirrors(world, rts);
            game.player.pos.z = LOOP_LEN - 0.5;
            game.player.yaw = 0.0;
            advance(8, timer, step);
        }
        8 => {
            game.update(1.0, 0.0, 0.0, 1.0 / 30.0);
            if game.loop_k >= 1 {
                if game.rebuild_needed {
                    *world = World::build(game.layout.clone(), tex);
                    bind_mirrors(world, rts);
                    game.rebuild_needed = false;
                }
                advance(9, timer, step);
            } else if *timer > 20.0 {
                *step = 992;
            }
        }
        9 => {
            // render a few frames of the mutated mall, then done
            if *timer > 1.0 {
                *step = 100;
            }
        }
        _ => {}
    }
}

