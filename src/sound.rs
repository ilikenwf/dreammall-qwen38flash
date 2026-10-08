//! Sound: the dream's soundtrack, generated at whatever rate the machine has.
//!
//! There is no audio thread to die, no panic to filter, and no device to guess
//! about. [`Sfx::boot`] asks ALSA for a stream ([`crate::alsa`]), learns the
//! rate and format it actually got, and hands the device to one thread that runs
//! the synthesiser at that rate for as long as the game lasts. Commands arrive
//! over a channel; the thread stops when the handle goes quiet or the hardware
//! stops answering.
//!
//! The soundtrack used to be two 64-second WAVs, pre-rendered at 44100 Hz and
//! crossfaded, because the previous backend could play nothing but buffers. The
//! synthesiser was always continuous and always took a rate, so both the
//! pre-render and the crossfade were the backend's limitation, not the game's.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::alsa::{self, Device};
use crate::music::Music;

/// How long the `--music` export is, in seconds. The in-game soundtrack is
/// generated continuously and has no length; this only bounds the WAV file.
pub const MUSIC_SECONDS: usize = 64;

/// How the player feels about audio.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AudioWanted {
    /// `--no-audio` or `DREAMMALL_AUDIO=0`: never touch the audio API.
    Off,
    /// Default: open the first device that will take stereo PCM.
    Auto,
    /// `DREAMMALL_AUDIO=1`: same attempt, but print every device tried and what
    /// each one said. The escape hatch is information, not a second code path —
    /// with negotiation there is nothing left to insist past.
    Verbose,
}

/// What the mixer thread is told to do.
enum Cmd {
    Flight(bool),
    Step(f32),
}

/// The dream's soundtrack. Silent by construction; `boot` decides otherwise.
pub struct Sfx {
    live: bool,
    status: String,
    tx: Option<Sender<Cmd>>,
    thread: Option<JoinHandle<()>>,
    /// Cleared by the mixer thread when the device stops answering, so the game
    /// stops queueing sounds nobody is going to play.
    alive: Arc<AtomicBool>,
    flying_now: bool,
    /// Every device tried at startup, kept for `--audio-check`.
    attempts: Mutex<Vec<(String, String)>>,
}

impl Sfx {
    fn silent(status: &str) -> Sfx {
        Sfx {
            live: false,
            status: status.to_string(),
            tx: None,
            thread: None,
            alive: Arc::new(AtomicBool::new(false)),
            flying_now: false,
            attempts: Mutex::new(Vec::new()),
        }
    }

    /// Decide whether audio is possible, then start the soundtrack.
    ///
    /// This blocks for the length of one device negotiation — milliseconds, and
    /// it has to, because the rate the device agrees to is the rate the
    /// synthesiser is built at.
    pub fn boot(wanted: AudioWanted, seed: u64) -> Sfx {
        if wanted == AudioWanted::Off {
            return Sfx::silent("audio: off");
        }
        let verbose = wanted == AudioWanted::Verbose;
        match alsa::open() {
            Ok((device, attempts)) => {
                if verbose {
                    log(&attempts);
                }
                let summary = device.summary();
                let (tx, rx) = mpsc::channel();
                let alive = Arc::new(AtomicBool::new(true));
                let flag = alive.clone();
                let spawned = std::thread::Builder::new()
                    .name("dreammall-audio".to_string())
                    .spawn(move || mixer(device, seed, rx, flag));
                // A failed spawn drops the closure, which drops the device,
                // which drains and closes it. Nothing is left dangling.
                match spawned {
                    Ok(thread) => Sfx {
                        live: true,
                        status: format!("audio: on · {summary}"),
                        tx: Some(tx),
                        thread: Some(thread),
                        alive,
                        flying_now: false,
                        attempts: Mutex::new(named(attempts)),
                    },
                    Err(_) => Sfx::silent("audio: could not start the audio thread"),
                }
            }
            Err(attempts) => {
                // Always printed: this is the one moment the player cannot see
                // what the machine did, and "no audio" without a reason is a
                // dead end.
                log(&attempts);
                let status = format!("audio: {} — playing silent", verdict(&attempts));
                Sfx::silent(&status).with_attempts(attempts)
            }
        }
    }

    fn with_attempts(self, attempts: Vec<alsa::Attempt>) -> Sfx {
        if let Ok(mut held) = self.attempts.lock() {
            *held = named(attempts);
        }
        self
    }

    /// Human-readable audio state, for the title screen.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// The whole audio path, described, for `--audio-check`: what was tried,
    /// what each device said, what ALSA believes exists, and which config files
    /// and environment variables are in play. One pasteable block.
    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("title screen: {}\n", self.status));
        if let Ok(held) = self.attempts.lock() {
            if held.is_empty() {
                out.push_str("tried: (nothing — audio is switched off)\n");
            } else {
                out.push_str("tried:\n");
                let width = held.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
                for (name, outcome) in held.iter() {
                    out.push_str(&format!("  {name:<width$} → {outcome}\n"));
                }
            }
        }
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
        for name in ["DREAMMALL_AUDIO", "ALSA_CARD", "DISPLAY"] {
            out.push_str(&format!(
                "${name}: {}\n",
                std::env::var(name).unwrap_or_else(|_| "unset".to_string())
            ));
        }
        out.push_str("DREAMMALL_AUDIO=1 prints the device list on a normal launch too.");
        out
    }

    /// Tell the soundtrack whether the player is flying.
    pub fn update(&mut self, flying: bool) {
        if !self.live || flying == self.flying_now {
            return;
        }
        self.flying_now = flying;
        self.send(Cmd::Flight(flying));
    }

    /// One footstep, panned.
    pub fn footstep(&mut self, pan: f32) {
        if !self.live {
            return;
        }
        self.send(Cmd::Step(pan));
    }

    fn send(&mut self, cmd: Cmd) {
        if !self.alive.load(Ordering::Relaxed) {
            self.live = false;
            return;
        }
        if let Some(tx) = &self.tx {
            if tx.send(cmd).is_err() {
                self.live = false;
            }
        }
    }
}

impl Drop for Sfx {
    fn drop(&mut self) {
        // Closing the channel is the shutdown signal; the thread finishes the
        // block it is writing, drains the device, and returns.
        self.tx = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The mixer: one synthesiser, one device, one clock.
///
/// `write` blocks until the hardware has taken the samples, so the loop needs no
/// timer — the device pulls the music out of the synthesiser at exactly the rate
/// it plays it, whatever rate that turned out to be.
fn mixer(mut device: Device, seed: u64, rx: Receiver<Cmd>, alive: Arc<AtomicBool>) {
    let mut music = Music::new(device.rate as f32, seed);
    // 40 ms per block: short enough that a footstep lands on the next frame,
    // long enough that the thread spends almost all its time inside ALSA.
    let frames = (device.rate as usize / 25).max(64);
    let mut block = vec![0.0f32; frames * 2];
    loop {
        match rx.try_recv() {
            Ok(Cmd::Flight(flight)) => music.set_flight(flight),
            Ok(Cmd::Step(pan)) => music.footstep(pan),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }
        music.fill(&mut block);
        if device.write(&block).is_err() {
            break;
        }
    }
    alive.store(false, Ordering::Relaxed);
}

fn named(attempts: Vec<alsa::Attempt>) -> Vec<(String, String)> {
    attempts.into_iter().map(|a| (a.name, a.outcome)).collect()
}

/// What to put on the title screen when nothing worked.
///
/// A device that could not be opened at all is a missing sound card; a device
/// that opened and then refused a parameter is a configuration problem, and
/// saying "no output device" about it is the lie that started this.
fn verdict(attempts: &[alsa::Attempt]) -> String {
    match attempts.first() {
        None => "no output device".to_string(),
        Some(first) => {
            if attempts.iter().all(|a| a.outcome.starts_with("cannot open")) {
                "no output device".to_string()
            } else {
                first.outcome.clone()
            }
        }
    }
}

fn log(attempts: &[alsa::Attempt]) {
    for a in attempts {
        eprintln!("audio: {name} — {outcome}", name = a.name, outcome = a.outcome);
    }
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

/// Interleaved stereo f32, clamped to i16 — clipping must never wrap.
fn pcm16(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32000.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn wav_bytes(pcm: &[u8], rate: u32, channels: u16) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + pcm.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&((36 + pcm.len()) as u32).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(name: &str, outcome: &str) -> alsa::Attempt {
        alsa::Attempt { name: name.to_string(), outcome: outcome.to_string() }
    }

    #[test]
    fn a_machine_with_no_cards_is_called_that() {
        let attempts = vec![
            attempt("default", "cannot open (No such file or directory)"),
            attempt("pipewire", "cannot open (No such file or directory)"),
        ];
        assert_eq!(verdict(&attempts), "no output device");
    }

    #[test]
    fn a_refused_parameter_is_not_reported_as_a_missing_card() {
        // The whole point: a device that exists and refuses one setting must
        // name that setting, because "no output device" sends the player off
        // hunting for hardware they already have.
        let attempts = vec![
            attempt("default", "device advertises no sample rate"),
            attempt("pipewire", "cannot open (No such file or directory)"),
        ];
        assert_eq!(
            verdict(&attempts),
            "device advertises no sample rate"
        );
    }

    #[test]
    fn nothing_tried_is_still_not_a_crash() {
        assert_eq!(verdict(&[]), "no output device");
    }

    #[test]
    fn silent_audio_never_claims_to_be_playing() {
        let mut sfx = Sfx::silent("audio: off");
        assert_eq!(sfx.status(), "audio: off");
        assert!(!sfx.live);
        // Neither of these may reach for a device that is not there.
        sfx.update(true);
        sfx.footstep(-0.4);
        assert!(!sfx.live);
    }

    #[test]
    fn switching_to_silent_clears_the_thread_flag() {
        let sfx = Sfx::silent("audio: off");
        assert!(!sfx.alive.load(Ordering::Relaxed));
        assert!(sfx.thread.is_none());
    }

    #[test]
    fn wav_header_is_well_formed() {
        let wav = render_music_wav(1, false);
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
