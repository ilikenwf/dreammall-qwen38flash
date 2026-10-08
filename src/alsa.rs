//! ALSA playback, negotiated instead of assumed.
//!
//! The backend this replaces opened `default`, demanded 44100 Hz, stereo,
//! `FLOAT_LE` and a 4096-frame buffer, and panicked at the first refusal — on a
//! thread nobody else could see. A machine whose `default` advertises no usable
//! rate (a broken `route`/`dmix` chain; `Can't set rate.`) therefore got neither
//! sound nor an alternative, even though ALSA has `plughw:` for exactly that
//! device: it converts rate and format on the application's behalf.
//!
//! So nothing here is assumed. Each candidate is asked which formats it has and
//! which rate it will actually run at, and the answer is what the rest of the
//! game is built on. There is no resampler because there is nothing to
//! resample: the soundtrack is generated, and [`crate::music::Music::new`]
//! already takes a rate, so the device decides and the synthesiser obeys.
//!
//! libasound is linked, not dlopened — the audio backend linked it already, so
//! this asks nothing new of a build machine.

use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong};
use std::ptr;

// Values read out of /usr/include/alsa/pcm.h rather than remembered; the last
// time this file's constants were written from memory they turned out to name
// panics that did not exist.
const STREAM_PLAYBACK: c_int = 0;
const ACCESS_RW_INTERLEAVED: c_int = 3;
const FORMAT_S16_LE: c_int = 2;
const FORMAT_S32_LE: c_int = 10;
const FORMAT_FLOAT_LE: c_int = 14;

/// Stereo throughout: the whole synthesiser pans, so mono would be a downgrade.
const CHANNELS: c_uint = 2;
/// What the synthesiser would like, not what it insists on.
const PREFERRED_RATE: c_uint = 44_100;
/// ~90 ms of slack at 48 kHz, which is generous enough that a scheduler hiccup
/// is an xrun ALSA recovers from rather than an audible gap.
const WANT_BUFFER: c_ulong = 8192;
const WANT_PERIOD: c_ulong = 1024;

#[link(name = "asound")]
extern "C" {
    fn snd_pcm_open(
        pcm: *mut *mut c_void,
        name: *const c_char,
        stream: c_int,
        mode: c_int,
    ) -> c_int;
    fn snd_pcm_close(pcm: *mut c_void) -> c_int;
    fn snd_pcm_hw_params_malloc(params: *mut *mut c_void) -> c_int;
    fn snd_pcm_hw_params_free(params: *mut c_void);
    fn snd_pcm_hw_params_any(pcm: *mut c_void, params: *mut c_void) -> c_int;
    fn snd_pcm_hw_params_set_access(pcm: *mut c_void, params: *mut c_void, access: c_int) -> c_int;
    fn snd_pcm_hw_params_test_format(pcm: *mut c_void, params: *mut c_void, val: c_int) -> c_int;
    fn snd_pcm_hw_params_set_format(pcm: *mut c_void, params: *mut c_void, val: c_int) -> c_int;
    fn snd_pcm_hw_params_test_channels(pcm: *mut c_void, params: *mut c_void, val: c_uint) -> c_int;
    fn snd_pcm_hw_params_set_channels(pcm: *mut c_void, params: *mut c_void, val: c_uint) -> c_int;
    fn snd_pcm_hw_params_set_rate_near(
        pcm: *mut c_void,
        params: *mut c_void,
        val: *mut c_uint,
        dir: *mut c_int,
    ) -> c_int;
    fn snd_pcm_hw_params_set_buffer_size_near(
        pcm: *mut c_void,
        params: *mut c_void,
        val: *mut c_ulong,
    ) -> c_int;
    fn snd_pcm_hw_params_set_period_size_near(
        pcm: *mut c_void,
        params: *mut c_void,
        val: *mut c_ulong,
        dir: *mut c_int,
    ) -> c_int;
    fn snd_pcm_hw_params(pcm: *mut c_void, params: *mut c_void) -> c_int;
    fn snd_pcm_prepare(pcm: *mut c_void) -> c_int;
    fn snd_pcm_recover(pcm: *mut c_void, err: c_int, silent: c_int) -> c_int;
    fn snd_pcm_writei(pcm: *mut c_void, buffer: *const c_void, size: c_ulong) -> c_long;
    fn snd_pcm_drain(pcm: *mut c_void) -> c_int;
    fn snd_strerror(err: c_int) -> *const c_char;
}

/// How samples reach the device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    F32,
    S32,
    S16,
}

impl Format {
    fn label(self) -> &'static str {
        match self {
            Format::F32 => "F32",
            Format::S32 => "S32",
            Format::S16 => "S16",
        }
    }

    fn bytes(self) -> usize {
        match self {
            Format::S16 => 2,
            Format::F32 | Format::S32 => 4,
        }
    }
}

/// One candidate tried, and what it said. Kept whether or not it worked,
/// because "the first thing we tried failed" is only useful with the name and
/// the reason attached.
pub struct Attempt {
    pub name: String,
    pub outcome: String,
}

/// An open playback stream, configured to the device's own terms.
pub struct Device {
    pcm: *mut c_void,
    pub name: String,
    pub rate: u32,
    pub format: Format,
    scratch: Vec<u8>,
}

// The handle is created on one thread and only ever touched from the mixer
// thread that receives it; ALSA's contract is one caller per handle at a time,
// which that arrangement satisfies.
unsafe impl Send for Device {}

impl Device {
    /// What the machine agreed to, for the title screen: `plughw:0,0 @ 48000 Hz S16`.
    pub fn summary(&self) -> String {
        format!("{} @ {} Hz {}", self.name, self.rate, self.format.label())
    }

    /// Push interleaved stereo samples in `[-1, 1]`, blocking until the device
    /// has taken all of them. That blocking is the whole clock of the audio
    /// thread: the synthesiser runs exactly as fast as the hardware eats.
    pub fn write(&mut self, frames: &[f32]) -> Result<(), c_int> {
        let want = frames.len() / CHANNELS as usize;
        self.encode(frames);
        let stride = CHANNELS as usize * self.format.bytes();
        let mut done = 0usize;
        let mut retries = 0;
        while done < want {
            let left = (want - done) as c_ulong;
            let at = unsafe { self.scratch.as_ptr().add(done * stride) } as *const c_void;
            let written = unsafe { snd_pcm_writei(self.pcm, at, left) };
            if written > 0 {
                done += written as usize;
                retries = 0;
                continue;
            }
            // `-EPIPE` is an xrun and `-EAGAIN` merely "not this instant"; ALSA
            // recovers from both, and from underruns caused by suspend. Anything
            // it cannot fix, the caller stops for.
            if unsafe { snd_pcm_recover(self.pcm, written as c_int, 1) } < 0 {
                return Err(written as c_int);
            }
            retries += 1;
            if retries > 8 {
                return Err(written as c_int);
            }
        }
        Ok(())
    }

    fn encode(&mut self, frames: &[f32]) {
        let n = frames.len();
        self.scratch.resize(n * self.format.bytes(), 0);
        match self.format {
            Format::F32 => {
                for (i, s) in frames.iter().enumerate() {
                    let at = i * 4;
                    self.scratch[at..at + 4].copy_from_slice(&s.clamp(-1.0, 1.0).to_le_bytes());
                }
            }
            Format::S32 => {
                for (i, s) in frames.iter().enumerate() {
                    let v = (s.clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
                    let at = i * 4;
                    self.scratch[at..at + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
            Format::S16 => {
                for (i, s) in frames.iter().enumerate() {
                    let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    let at = i * 2;
                    self.scratch[at..at + 2].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // A `Device` with no handle was never opened: the tests build one to
        // exercise the encoding without a sound card, and ALSA would hand the
        // null straight to the driver.
        if self.pcm.is_null() {
            return;
        }
        // Draining first lets whatever is queued finish instead of clipping the
        // tail off the last footstep.
        unsafe {
            snd_pcm_drain(self.pcm);
            snd_pcm_close(self.pcm);
        }
    }
}

/// Device names worth trying, best first.
///
/// `default` first, because that is what the machine's owner configured. Then
/// every hardware stream through `plughw:`, ALSA's own conversion layer: a
/// device that refuses 44100 Hz or `FLOAT_LE` outright will usually take them
/// through `plug`, which is precisely the case that has no answer when the only
/// name ever tried is `default`. Raw `hw:` last — it bypasses mixing, so it is
/// a last resort rather than a convenience.
pub fn candidates() -> Vec<String> {
    let hardware = hardware_streams();
    let mut out = vec!["default".to_string(), "pipewire".to_string()];
    for (card, device) in &hardware {
        out.push(format!("plughw:{card},{device}"));
    }
    for (card, device) in &hardware {
        out.push(format!("hw:{card},{device}"));
    }
    out
}

/// `(card, device)` for every playback stream under /proc/asound. Reading the
/// procfs tree keeps the control-plane API out of this file entirely: no
/// `snd_ctl_*`, no card-info structs, no second handle to leak.
fn hardware_streams() -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let Ok(cards) = std::fs::read_dir("/proc/asound") else {
        return out;
    };
    for card in cards.flatten() {
        let Some(number) = card
            .file_name()
            .to_string_lossy()
            .strip_prefix("card")
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(streams) = std::fs::read_dir(card.path()) else {
            continue;
        };
        for stream in streams.flatten() {
            let name = stream.file_name().to_string_lossy().into_owned();
            // `pcm0p` is device 0 of card N, playback. Capture (`pcmNc`) is not
            // something a dream-sim asks for.
            if let Some(rest) = name.strip_prefix("pcm").and_then(|s| s.strip_suffix('p')) {
                if let Ok(device) = rest.parse::<u32>() {
                    out.push((number, device));
                }
            }
        }
    }
    out.sort_unstable();
    out
}

/// Open the first candidate that will take stereo PCM.
///
/// Both outcomes carry the attempt list: a working `plughw:0,0` is much more
/// reassuring when you can see that `default` was the thing that was broken.
pub fn open() -> Result<(Device, Vec<Attempt>), Vec<Attempt>> {
    let mut attempts = Vec::new();
    for name in candidates() {
        match try_open(&name) {
            Ok(device) => {
                attempts.push(Attempt { name: name.clone(), outcome: device.summary() });
                return Ok((device, attempts));
            }
            Err(why) => attempts.push(Attempt { name, outcome: why }),
        }
    }
    Err(attempts)
}

fn try_open(name: &str) -> Result<Device, String> {
    let cname = CString::new(name).map_err(|_| "device name contains a NUL".to_string())?;
    let mut pcm: *mut c_void = ptr::null_mut();
    let rc = unsafe { snd_pcm_open(&mut pcm, cname.as_ptr(), STREAM_PLAYBACK, 0) };
    if rc < 0 {
        return Err(format!("cannot open ({})", explain(rc)));
    }

    let mut params: *mut c_void = ptr::null_mut();
    if unsafe { snd_pcm_hw_params_malloc(&mut params) } < 0 {
        unsafe { snd_pcm_close(pcm) };
        return Err("out of memory asking for parameters".to_string());
    }
    let negotiated = unsafe { negotiate(pcm, params) };
    unsafe { snd_pcm_hw_params_free(params) };

    match negotiated {
        Ok((rate, format)) => Ok(Device {
            pcm,
            name: name.to_string(),
            rate,
            format,
            scratch: Vec::new(),
        }),
        Err(why) => {
            unsafe { snd_pcm_close(pcm) };
            Err(why)
        }
    }
}

/// Ask, don't tell: which format does it have, what rate will it actually run.
///
/// Every step is a question ALSA answers with a yes or a no, so a refusal here
/// becomes a sentence on the title screen instead of a panic on someone else's
/// thread.
unsafe fn negotiate(pcm: *mut c_void, params: *mut c_void) -> Result<(u32, Format), String> {
    if snd_pcm_hw_params_any(pcm, params) < 0 {
        return Err("device would not describe itself".to_string());
    }
    if snd_pcm_hw_params_set_access(pcm, params, ACCESS_RW_INTERLEAVED) < 0 {
        return Err("device will not do interleaved playback".to_string());
    }
    if snd_pcm_hw_params_test_channels(pcm, params, CHANNELS) < 0 {
        return Err("device will not do stereo".to_string());
    }
    let (format_id, format) = [(FORMAT_FLOAT_LE, Format::F32), (FORMAT_S32_LE, Format::S32)]
        .into_iter()
        .chain(std::iter::once((FORMAT_S16_LE, Format::S16)))
        .find(|(id, _)| snd_pcm_hw_params_test_format(pcm, params, *id) == 0)
        .ok_or_else(|| "device has no sample format we can produce".to_string())?;

    // The rate is the whole reason this file exists. `set_rate_near` rounds to
    // the nearest rate the device supports and writes it back, so the answer —
    // not the request — is what gets synthesised. It only fails when the device
    // advertises no rate at all, which is a broken plugin chain rather than a
    // missing sound card.
    let mut rate: c_uint = PREFERRED_RATE;
    if snd_pcm_hw_params_set_rate_near(pcm, params, &mut rate, ptr::null_mut()) < 0 {
        return Err("device advertises no sample rate".to_string());
    }

    if snd_pcm_hw_params_set_format(pcm, params, format_id) < 0 {
        return Err(format!("device refused {}", format.label()));
    }
    if snd_pcm_hw_params_set_channels(pcm, params, CHANNELS) < 0 {
        return Err("device refused stereo".to_string());
    }
    // Buffer and period are advisory: `near` takes the closest the device has
    // and there is nothing to complain about.
    let mut buffer = WANT_BUFFER;
    snd_pcm_hw_params_set_buffer_size_near(pcm, params, &mut buffer);
    let mut period = WANT_PERIOD;
    snd_pcm_hw_params_set_period_size_near(pcm, params, &mut period, ptr::null_mut());

    if snd_pcm_hw_params(pcm, params) < 0 {
        return Err("device refused the settings it just described".to_string());
    }
    if snd_pcm_prepare(pcm) < 0 {
        return Err("device would not start playing".to_string());
    }
    Ok((rate, format))
}

/// ALSA's own words for an error code.
fn explain(err: c_int) -> String {
    let s = unsafe { snd_strerror(err) };
    if s.is_null() {
        return format!("error {err}");
    }
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plughw_is_tried_before_raw_hardware() {
        // The conversion layer has to be attempted before the thing that needs
        // converting, or a rate- or format-refusing device never gets a second
        // chance — which is the bug this module was written for.
        let names = candidates();
        assert_eq!(names[0], "default");
        assert!(names.contains(&"pipewire".to_string()));
        let plug = names.iter().position(|n| n.starts_with("plughw:"));
        let raw = names.iter().position(|n| n.starts_with("hw:"));
        match (plug, raw) {
            (Some(p), Some(r)) => assert!(p < r, "plughw must be tried before hw"),
            // No cards on this machine: only the two named devices remain.
            (None, None) => assert_eq!(names.len(), 2),
            _ => panic!("candidates ordered wrong: {names:?}"),
        }
    }

    #[test]
    fn every_candidate_is_a_name_alsa_understands() {
        for name in candidates() {
            assert!(!name.is_empty());
            assert!(!name.contains('\0'));
            if let Some(rest) = name.strip_prefix("plughw:") {
                assert!(rest.contains(','), "plughw needs card,device: {rest}");
            }
        }
    }

    #[test]
    fn encoding_respects_the_negotiated_format() {
        let mut dev = Device {
            pcm: ptr::null_mut(),
            name: "test".to_string(),
            rate: 48_000,
            format: Format::S16,
            scratch: Vec::new(),
        };
        dev.encode(&[0.0, 1.0, -1.0, 0.5]);
        assert_eq!(dev.scratch.len(), 8);
        let i16_at = |i: usize| i16::from_le_bytes([dev.scratch[i * 2], dev.scratch[i * 2 + 1]]);
        assert_eq!(i16_at(0), 0);
        assert_eq!(i16_at(1), i16::MAX);
        assert_eq!(i16_at(2), -i16::MAX);
        assert!(((i16_at(3) as f32 / i16::MAX as f32) - 0.5).abs() < 0.001);

        dev.format = Format::F32;
        dev.encode(&[2.0, -2.0]);
        assert_eq!(dev.scratch.len(), 8);
        let f32_at = |i: usize| {
            f32::from_le_bytes([
                dev.scratch[i * 4],
                dev.scratch[i * 4 + 1],
                dev.scratch[i * 4 + 2],
                dev.scratch[i * 4 + 3],
            ])
        };
        // Out-of-range input is clamped, never wrapped into an audible pop.
        assert_eq!(f32_at(0), 1.0);
        assert_eq!(f32_at(1), -1.0);
    }

    #[test]
    fn summary_names_device_rate_and_format() {
        let dev = Device {
            pcm: ptr::null_mut(),
            name: "plughw:0,0".to_string(),
            rate: 48_000,
            format: Format::S16,
            scratch: Vec::new(),
        };
        assert_eq!(dev.summary(), "plughw:0,0 @ 48000 Hz S16");
    }

    #[test]
    fn alsa_errors_are_quoted_not_invented() {
        // Whatever ALSA says for -ENOENT, it is its own words and never empty.
        let why = explain(-2);
        assert!(!why.is_empty());
        assert_ne!(why, "error -2", "snd_strerror should be reachable");
    }
}
