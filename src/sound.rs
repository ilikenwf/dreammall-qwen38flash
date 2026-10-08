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
//! So [`install_startup_guard`] runs before macroquad boots, records that one
//! benign panic as a flag, and [`Sfx::boot`] refuses to touch the audio API once
//! the flag is set — a machine with no sound card never calls into the dead
//! thread, never prints `Audio thread died`, shows one clear status line on the
//! title screen, and plays a fully silent game. That flag is the entire
//! diagnosis: nothing is probed or sniffed, because a wrong "no device" mutes a
//! machine that has one. `--no-audio` forces silence; `DREAMMALL_AUDIO=1`
//! insists anyway.

use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use macroquad::audio::{self, Sound};

use crate::music::Music;

/// Set when quad-snd's audio thread died at startup (see [`install_startup_guard`]).
static AUDIO_THREAD_DIED: AtomicBool = AtomicBool::new(false);

/// What quad-snd actually panicked about, e.g. `Can't open PCM device.`.
///
/// That thread fails in several distinct ways and they need different fixes: no
/// card at all, a card that refuses `FLOAT_LE`, a buffer size `dmix` will not
/// agree to. Calling all of them "no output device" sends people off hunting for
/// hardware they already have.
static AUDIO_FAILURE: Mutex<Option<String>> = Mutex::new(None);

/// Has quad-snd's mixer thread gone? Set from the panic hook, on *its* thread,
/// at whatever moment the device gives up — so it can only ever be read as
/// "not yet", never as a promise about the next call.
fn died() -> bool {
    AUDIO_THREAD_DIED.load(Ordering::Relaxed)
}

/// quad-snd's own words, if it got as far as complaining.
fn failure() -> Option<String> {
    AUDIO_FAILURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// quad-snd's panic strings, translated into something a player can act on.
///
/// The dependency's exact words, with file and line, go to stderr, where they
/// can be pasted into a bug report; `alsa_snd.rs:30` under a title screen helps
/// nobody. Each entry is a different failure with a different fix, which is the
/// point: calling all of them "no output device" is what sent a player with
/// working hardware off hunting for a sound card.
const REASONS: &[(&str, &str)] = &[
    ("open PCM", "no output device"),
    ("FLOAT_LE", "device refuses 32-bit float output"),
    ("buffer size", "device rejected the buffer size"),
    // quad-snd's own spelling, not a typo to correct.
    ("treshold", "device rejected the buffer timing"),
    ("sample rate", "device rejected 44100 Hz"),
    ("channels", "device rejected stereo"),
    ("not ready", "device never became ready"),
    ("access type", "device rejected read-write access"),
    ("hw parameters", "device would not describe itself"),
];

/// The player-facing half of a failure. Note what an unrecognised reason does
/// *not* claim: "no output device" was a guess, and stays one.
fn short_reason(why: &str) -> &'static str {
    REASONS
        .iter()
        .find(|(needle, _)| why.contains(needle))
        .map(|(_, message)| *message)
        .unwrap_or(if why.is_empty() {
            "no output device"
        } else {
            "device failed to start"
        })
}

/// The title screen line for a dead audio thread.
fn audio_status(insisting: bool) -> String {
    let short = short_reason(&failure().unwrap_or_default());
    if insisting {
        format!("audio: forced on — {short}")
    } else {
        format!("audio: {short} — playing silent")
    }
}

/// The panic's message. Takes the payload directly rather than the hook-info
/// type, which only answers to `PanicHookInfo` since 1.81 (before that,
/// `PanicInfo`) and naming it would needlessly date the code.
fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "failed to start".to_string()
    }
}

/// Just the file name from a panic location's full path.
fn file_name(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit('\\').next().unwrap_or(name)
}

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
    let verbose = std::env::var("DREAMMALL_AUDIO_VERBOSE").is_ok();
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
            // Detection is not optional: `Sfx::boot` reads this flag to work out
            // whether the audio API is a corpse. Only the printing is gated.
            let why = match info.location() {
                Some(loc) => format!(
                    "{} ({}:{})",
                    panic_text(info.payload()),
                    file_name(loc.file()),
                    loc.line()
                ),
                None => panic_text(info.payload()),
            };
            *AUDIO_FAILURE.lock().unwrap_or_else(|p| p.into_inner()) = Some(why);
            AUDIO_THREAD_DIED.store(true, Ordering::Relaxed);
            if !verbose {
                return;
            }
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

/// What the command line asked for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AudioWanted {
    /// `--no-audio` or `DREAMMALL_AUDIO=0`: never touch the audio API.
    Off,
    /// Default: play unless quad-snd's audio thread died at startup.
    Auto,
    /// `DREAMMALL_AUDIO=1`: play even if the audio thread died. The escape
    /// hatch for a device worth shouting at — insisting is the only way to
    /// argue with the diagnosis.
    Forced,
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

    /// Give up on audio, quoting quad-snd rather than guessing at its reasons.
    fn gone(insisting: bool) -> Sfx {
        if let Some(why) = failure() {
            eprintln!("audio: quad-snd could not start — {why}");
            eprintln!(
                "       DREAMMALL_AUDIO_VERBOSE=1 prints the panic; --music \
                 out.wav writes the soundtrack to a file instead"
            );
        }
        Sfx::silent(&audio_status(insisting))
    }

    /// Decide whether audio is possible, then load and start the soundtrack.
    pub async fn boot(wanted: AudioWanted, seed: u64) -> Sfx {
        let insisting = wanted == AudioWanted::Forced;
        if wanted == AudioWanted::Off {
            return Sfx::silent("audio: off");
        }
        // quad-snd's thread panics when it cannot open a device, and the hook in
        // [`install_startup_guard`] turns that panic into a flag. That is direct
        // evidence, so there is nothing to guess here: sniffing `/dev/snd` or
        // probing `snd_pcm_open` could only ever be wrong, and a wrong "no
        // device" mutes a machine that has one.
        if !insisting && died() {
            return Sfx::gone(insisting);
        }

        // Synthesise before handing anything to the audio API. The thread that
        // owns the device is spawned during macroquad's startup and panics
        // *asynchronously*, so the flag can be set at any moment — including one
        // just after we read it. Synthesis takes seconds, which is the grace that
        // thread needs to fail; each sound sent to a corpse instead prints a line
        // of `Audio thread died` from inside quad-snd.
        let normal = render_music_wav(seed, false);
        let music_normal = load(&normal).await;
        // One sound is now in flight — the earliest a dead thread can possibly be
        // known about. Insisting has bought its look; from here the thread is
        // gone, and every further send is one more printed line for nobody.
        if died() {
            return Sfx::gone(insisting);
        }
        let flight = render_music_wav(seed, true);
        let music_flight = load(&flight).await;
        let left = render_step_wav(-0.4);
        let step_l = load(&left).await;
        let right = render_step_wav(0.4);
        let step_r = load(&right).await;

        let sfx = Sfx {
            live: true,
            status: "audio: on".to_string(),
            music_normal,
            music_flight,
            step_l,
            step_r,
            flight_fade: 0.0,
            flying_now: false,
        };
        // Again: the thread can die in the gap between any two of these calls.
        if sfx.talkable() {
            if let Some(s) = &sfx.music_normal {
                audio::play_sound(
                    s,
                    audio::PlaySoundParams {
                        looped: true,
                        volume: 1.0,
                    },
                );
            }
        }
        sfx
    }

    /// Human-readable audio state, for the title screen.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Whether it is still worth calling into the audio API. Once the mixer
    /// thread is gone, every call buys another line of `Audio thread died` —
    /// including a thread that outlived startup and died later, mid-game.
    fn talkable(&self) -> bool {
        self.live && !died()
    }

    /// One footstep, panned left/right.
    pub fn footstep(&mut self, pan: f32) {
        if !self.talkable() {
            return;
        }
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
        if !self.talkable() {
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

/// Decode a sound — unless the mixer thread is already known to be gone.
///
/// The guard belongs here and not at each call site because this is the one
/// place that hands data to quad-snd's thread, and every send to a corpse prints
/// `Audio thread died` from inside the dependency. The thread can die between any
/// two of the loads in [`Sfx::boot`], so the check has to be per-send.
async fn load(bytes: &[u8]) -> Option<Sound> {
    if died() {
        return None;
    }
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

/// The whole audio path, described, for `--audio-check`.
///
/// One pasteable block: what quad-snd actually said, and what ALSA believes
/// exists. The title screen's one-line reason is only half the story — the other
/// half is whether `default` resolves to anything at all, and only the machine
/// running the game knows that. Reads the same process-global state the mixer
/// does, which is why this is a free function and not a method on a sound handle.
pub fn report() -> String {
    let mut out = String::new();
    match failure() {
        Some(why) => out.push_str(&format!("quad-snd: {why}\n")),
        None => out.push_str("quad-snd: no failure recorded\n"),
    }
    // What a *normal* launch would say; the check itself forces the attempt, so
    // the status it leaves behind is the insisting variant.
    let verdict = if died() {
        audio_status(false)
    } else {
        "audio: on".to_string()
    };
    out.push_str(&format!("title screen: {verdict}\n"));
    out.push_str(&format!("$ aplay -l\n{}\n", probe("aplay", &["-l"])));
    out.push_str(&format!("$ aplay -L\n{}\n", probe("aplay", &["-L"])));
    let home = std::env::var("HOME").unwrap_or_default();
    for (label, path) in [
        ("~/.asoundrc", format!("{home}/.asoundrc")),
        ("/etc/asound.conf", "/etc/asound.conf".to_string()),
    ] {
        out.push_str(&format!(
            "{label}: {}\n",
            if std::path::Path::new(&path).exists() {
                "present"
            } else {
                "absent"
            }
        ));
    }
    for name in ["DREAMMALL_AUDIO", "DREAMMALL_AUDIO_VERBOSE", "ALSA_CARD", "DISPLAY"] {
        out.push_str(&format!(
            "${name}: {}\n",
            std::env::var(name).unwrap_or_else(|_| "unset".to_string())
        ));
    }
    out.push_str("DREAMMALL_AUDIO_VERBOSE=1 --audio-check also prints the panic and backtrace.");
    out
}

/// Runs a diagnostic command and keeps its first lines. A missing binary is an
/// answer rather than an error: no `aplay` says something about the box. ALSA
/// reports "no sound cards found" on stderr, so an empty stdout falls back.
fn probe(cmd: &str, args: &[&str]) -> String {
    let Ok(out) = std::process::Command::new(cmd).args(args).output() else {
        return format!("(no {cmd} on PATH)");
    };
    let bytes = if out.stdout.is_empty() { &out.stderr } else { &out.stdout };
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return "(no output)".to_string();
    }
    let kept = lines.iter().take(14).copied().collect::<Vec<_>>().join("\n");
    match lines.len().checked_sub(14) {
        Some(rest) => format!("{kept}\n… {rest} more"),
        None => kept,
    }
}

/// How long the soundtrack loop is, in seconds. Both the in-game loop and
/// `--music` use this, so the number in the help text cannot drift.
pub const MUSIC_SECONDS: usize = 64;

/// Render the whole muzak loop to 16-bit PCM WAV bytes.
pub fn render_music_wav(seed: u64, flight: bool) -> Vec<u8> {
    let rate = 44100.0f32;
    let n = (rate * MUSIC_SECONDS as f32) as usize;
    let mut m = Music::new(rate, seed);
    m.set_flight(flight);
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

    #[test]
    fn quad_snd_reasons_are_quoted_verbatim() {
        // The two real quad-snd failures that need entirely different fixes must
        // not collapse into the same sentence on the title screen.
        let format = String::from("Can't set SND_PCM_FORMAT_FLOAT_LE format");
        assert_eq!(panic_text(&format), "Can't set SND_PCM_FORMAT_FLOAT_LE format");
        static OPEN: &str = "Can't open PCM device.";
        assert_eq!(panic_text(&OPEN), "Can't open PCM device.");
        // A panic payload that is neither kind of string still has to say
        // something rather than nothing.
        assert_eq!(panic_text(&42u32), "failed to start");
    }

    #[test]
    fn panic_locations_are_reduced_to_a_file_and_line() {
        assert_eq!(
            file_name("/home/matt/.cargo/registry/src/quad-snd-0.2.8/src/alsa_snd.rs"),
            "alsa_snd.rs"
        );
        assert_eq!(file_name("C:\\Users\\m\\quad-snd-0.2.8\\src\\alsa_snd.rs"), "alsa_snd.rs");
        assert_eq!(file_name("alsa_snd.rs"), "alsa_snd.rs");
    }

    #[test]
    fn each_device_failure_gets_its_own_sentence() {
        // The bug this guards: every one of these used to read "no output
        // device" on the title screen.
        assert_eq!(short_reason("Can't open PCM device."), "no output device");
        assert_eq!(
            short_reason("Can't set SND_PCM_FORMAT_FLOAT_LE format"),
            "device refuses 32-bit float output"
        );
        assert_eq!(
            short_reason("Cant's set buffer size"),
            "device rejected the buffer size"
        );
        // Matching the correctly-spelled word would find nothing.
        assert_eq!(
            short_reason("Cant's set treshold size"),
            "device rejected the buffer timing"
        );
        // An unrecognised reason must not accuse anyone's hardware of being absent.
        assert_eq!(short_reason("something new"), "device failed to start");
        assert_eq!(short_reason(""), "no output device");
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

