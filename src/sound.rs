//! Audio: the procedural dream-music, footsteps, and the machinery that keeps
//! a machine with no sound card *silent* rather than dead.
//!
//! Two separate traps here, both real:
//!
//! 1. macroquad 0.4.16 declares `default = []`, so `audio` is **not** a default
//!    feature. Without it, `macroquad::audio` is a no-op stub that prints
//!    `warn: macroquad's "audio" feature disabled.` and the game is mute.
//! 2. Turn the feature on and quad-snd spawns an ALSA thread that *panics*
//!    (`Can't open PCM device.`) when no PCM device opens — which macroquad
//!    does during startup, before any of our code runs. Every later call then
//!    prints `Audio thread died` forever.
//!
//! So [`install_startup_guard`] runs before macroquad boots and swallows that
//! one benign panic, and [`Sfx::boot`] refuses to touch the audio API unless
//! [`device_plausible`] finds something to play through — so a machine with no
//! sound card never calls into the dead thread, never prints
//! `Audio thread died`, shows one clear status line on the title screen, and
//! plays a fully silent game. `--no-audio` forces silence anywhere.

use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};

use macroquad::audio::{self, Sound};

use crate::music::Music;

/// Set when quad-snd's audio thread died at startup (see [`install_startup_guard`]).
static AUDIO_THREAD_DIED: AtomicBool = AtomicBool::new(false);

/// Filter quad-snd's device-init panic out of the panic report.
///
/// Must be called **before** macroquad starts: macroquad builds its audio
/// context during startup, and that is what spawns the ALSA thread that panics
/// on a box with no sound card. Nothing inside the async main runs early enough
/// to matter.
///
/// This deliberately does *not* try to silence libasound's own
/// `cannot find card '0'` chatter. The only handle on it is
/// `snd_lib_error_set_handler`, whose callback is C-variadic, and declaring a
/// variadic `extern "C"` fn is unstable Rust (feature `c_variadic`, rust#44930)
/// — it does not compile on stable. That chatter is what every native ALSA
/// program prints on a machine with no sound card; the panic and the
/// `Audio thread died` spam are the parts that are ours to suppress.
/// `DREAMMALL_AUDIO_VERBOSE=1` restores the real panic report.
pub fn install_startup_guard() {
    if std::env::var("DREAMMALL_AUDIO_VERBOSE").is_ok() {
        return;
    }

    let previous = panic::take_hook();
    // The closure's argument type is inferred rather than named on purpose:
    // `panic::PanicHookInfo` only exists under that name since 1.81 (it used to
    // be `PanicInfo`), and spelling it here would needlessly date the code.
    panic::set_hook(Box::new(move |info| {
        let benign = info
            .location()
            .map(|loc| from_quad_snd(loc.file()))
            .unwrap_or(false);
        if benign {
            AUDIO_THREAD_DIED.store(true, Ordering::Relaxed);
            return;
        }
        previous(info);
    }));
}

/// Does this path belong to quad-snd, where every device-init panic lives?
///
/// Matching on the file keeps every other panic in the game reporting
/// normally — we hide one known-benign message, not panics in general.
fn from_quad_snd(file: &str) -> bool {
    file.contains("quad-snd")
}

/// Is there anything this machine could plausibly play audio through?
///
/// Deliberately does *not* call `snd_pcm_open`: every failed open makes
/// libasound print nine lines of config errors to stderr, so asking that way
/// would double the noise on exactly the machines that have no sound card.
/// Look for what ALSA needs instead of opening it.
///
/// A false "yes" is cheap: quad-snd's thread panics, [`install_startup_guard`]
/// notices, and [`Sfx::boot`] falls back to silence anyway. A false "no" would
/// mute a working machine, so this answers "no" only when every route is
/// missing — and quad-snd's sole Linux backend is ALSA, so these are all of
/// the routes there are.
#[cfg(target_os = "linux")]
fn device_plausible() -> bool {
    if std::env::var("PULSE_SERVER").is_ok() || std::env::var("PIPEWIRE_REMOTE").is_ok() {
        return true;
    }
    // The node quad-snd ultimately opens.
    if std::fs::read_dir("/dev/snd")
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| is_playback_node(&entry.file_name().to_string_lossy()))
        })
        .unwrap_or(false)
    {
        return true;
    }
    // ... or a server that ALSA's `default` route forwards to.
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => {
            std::path::Path::new(&format!("{dir}/pipewire-0")).exists()
                || std::path::Path::new(&format!("{dir}/pulse/native")).exists()
        }
        Err(_) => false,
    }
}

/// Is this `/dev/snd` entry something you can play through?
///
/// `pcmC0D0p` is playback; `pcmC0D0c` is capture, and `controlC0`, `timer` and
/// `seq` are not streams at all.
#[cfg(target_os = "linux")]
fn is_playback_node(name: &str) -> bool {
    name.starts_with("pcmC") && name.ends_with('p')
}

#[cfg(not(target_os = "linux"))]
fn device_plausible() -> bool {
    // Nothing portable to inspect; let macroquad try and fall back if it fails.
    true
}

/// The dream's soundtrack. Silent by construction; `boot` decides otherwise.
pub struct Sfx {
    live: bool,
    status: String,
    music_normal: Option<Sound>,
    music_flight: Option<Sound>,
    step_l: Option<Sound>,
    step_r: Option<Sound>,
    flight_fade: f32,
    flying_now: bool,
}

impl Sfx {
    fn silent(status: &str) -> Sfx {
        Sfx {
            live: false,
            status: status.to_string(),
            music_normal: None,
            music_flight: None,
            step_l: None,
            step_r: None,
            flight_fade: 0.0,
            flying_now: false,
        }
    }

    /// Decide whether audio is possible, then load and start the soundtrack.
    pub async fn boot(asked: bool, seed: u64) -> Sfx {
        if !asked {
            return Sfx::silent("audio: off");
        }
        // Name the real cause: no plausible device at all, versus a device that
        // quad-snd did try to bring up and could not (busy, permissions, ...).
        if !device_plausible() {
            return Sfx::silent("audio: no output device — playing silent");
        }
        if AUDIO_THREAD_DIED.load(Ordering::Relaxed) {
            return Sfx::silent("audio: device failed to start — playing silent");
        }

        let mut sfx = Sfx {
            live: true,
            status: "audio: on".to_string(),
            music_normal: None,
            music_flight: None,
            step_l: None,
            step_r: None,
            flight_fade: 0.0,
            flying_now: false,
        };
        sfx.music_normal = load(&render_music_wav(seed, false)).await;
        sfx.music_flight = load(&render_music_wav(seed, true)).await;
        sfx.step_l = load(&render_step_wav(-0.4)).await;
        sfx.step_r = load(&render_step_wav(0.4)).await;
        if let Some(s) = &sfx.music_normal {
            audio::play_sound(
                s,
                audio::PlaySoundParams {
                    looped: true,
                    volume: 1.0,
                },
            );
        }
        sfx
    }

    /// Human-readable audio state, for the title screen.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// One footstep, panned left/right.
    pub fn footstep(&mut self, pan: f32) {
        let slot = if pan < 0.0 { &self.step_l } else { &self.step_r };
        if let Some(s) = slot {
            audio::play_sound(
                s,
                audio::PlaySoundParams {
                    looped: false,
                    volume: 0.9,
                },
            );
        }
    }

    /// Crossfade normal muzak into the brighter flight theme.
    pub fn update(&mut self, flying: bool, dt: f32) {
        if !self.live {
            return;
        }
        if flying && !self.flying_now {
            self.flying_now = true;
            if let Some(s) = &self.music_flight {
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
            self.flight_fade = (self.flight_fade + dt * 0.8).min(1.0);
            if let Some(s) = &self.music_normal {
                audio::set_sound_volume(s, 1.0 - self.flight_fade);
            }
            if let Some(s) = &self.music_flight {
                audio::set_sound_volume(s, self.flight_fade);
            }
        }
    }
}

async fn load(bytes: &[u8]) -> Option<Sound> {
    audio::load_sound_from_bytes(bytes).await.ok()
}

// ---------------------------------------------------------------------------
// Soundtrack synthesis. The whole score is rendered to WAV in memory at boot,
// so the mall ships with no audio assets and the music is a pure function of
// the seed.
// ---------------------------------------------------------------------------

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
pub fn render_music_wav(seed: u64, flight: bool) -> Vec<u8> {
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

pub fn render_step_wav(pan: f32) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn only_playback_nodes_count_as_an_output() {
        assert!(is_playback_node("pcmC0D0p"));
        assert!(is_playback_node("pcmC1D7p"));
        // Capture streams, mixers, and timers cannot be played through.
        assert!(!is_playback_node("pcmC0D0c"));
        assert!(!is_playback_node("controlC0"));
        assert!(!is_playback_node("timer"));
        assert!(!is_playback_node("seq"));
    }

    #[test]
    fn only_quad_snd_panics_are_swallowed() {
        // The real path shape, as it appears in a panic location.
        assert!(from_quad_snd(
            "/home/matt/.cargo/registry/src/index.crates.io/quad-snd-0.2.8/src/alsa_snd.rs"
        ));
        // Everything else has to keep reporting normally.
        assert!(!from_quad_snd("src/game.rs"));
        assert!(!from_quad_snd("src/sound.rs"));
        assert!(!from_quad_snd("/rust/deps/library/std/src/panicking.rs"));
    }

    #[test]
    fn wav_header_is_well_formed() {
        let wav = render_step_wav(0.0);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        // The RIFF length field must agree with the file we actually built.
        let declared = u32::from_le_bytes([wav[4], wav[5], wav[6], wav[7]]) as usize;
        assert_eq!(declared + 8, wav.len());
        // 16-bit stereo: the data chunk must be a whole number of frames.
        let data = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]) as usize;
        assert_eq!(data % 4, 0);
        assert_eq!(data + 44, wav.len());
    }

    #[test]
    fn flight_theme_is_a_different_track() {
        assert_ne!(render_music_wav(7, false), render_music_wav(7, true));
    }

    #[test]
    fn seed_changes_the_music() {
        assert_ne!(render_music_wav(1, false), render_music_wav(2, false));
    }

    #[test]
    fn samples_stay_in_range() {
        // Clipping would wrap i16 and crackle; pcm16 must clamp instead.
        let pcm = pcm16(&[2.0, -2.0, 0.0]);
        let frames: Vec<i16> = pcm
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(frames, vec![32000, -32000, 0]);
    }
}
