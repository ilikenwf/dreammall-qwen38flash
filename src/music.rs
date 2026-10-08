//! Procedural vaporwave / smooth-jazz mall muzak. Pure sample synthesis —
//! no audio files. Chords (Rhodes-ish), walking bass, sparse pentatonic
//! melody, soft offbeat hats, tape echo, dreamy lowpass.

use crate::rng::Rng;

const BPM: f32 = 76.0;
const BEAT: f32 = 60.0 / BPM;
const EIGHTH: f32 = BEAT / 2.0;
const CHORD_DUR: f32 = BEAT * 2.0;

/// (root semitone from A2=110Hz, chord intervals)
type Chord = (i16, [i16; 4]);

const DM7: Chord = (2, [3, 7, 10, 14]);
const G7: Chord = (7, [4, 7, 10, 14]);
const CM7: Chord = (0, [4, 7, 11, 16]);
const AM7: Chord = (9, [3, 7, 10, 12]);
const FM7: Chord = (5, [4, 7, 11, 14]);
const EM7: Chord = (4, [3, 7, 10, 13]);

const PROGRESSIONS: [[Chord; 4]; 3] = [
    [DM7, G7, CM7, AM7],
    [FM7, EM7, DM7, CM7],
    [CM7, AM7, DM7, G7],
];

const PENTA: [i16; 8] = [0, 2, 4, 7, 9, 12, 14, 16];

const V_RHODES: u8 = 0;
const V_BASS: u8 = 1;
const V_MEL: u8 = 2;
const V_HAT: u8 = 3;
const V_FOOT: u8 = 4;

struct Voice {
    kind: u8,
    freq: f32,
    age: f32,
    dur: f32,
    vel: f32,
    pan: f32,
    phase: f32,
    phase2: f32,
    phase3: f32,
}

pub struct Music {
    rate: f32,
    rng: Rng,
    voices: Vec<Voice>,
    next_chord: f32,
    prog: usize,
    prog_ticks: usize,
    next_mel: f32,
    mel_step: usize,
    phrase: [i16; 16], // -1 = rest, else PENTA index
    next_bass: f32,
    bass_step: usize,
    next_hat: f32,
    hat_step: usize,
    chord: Chord,
    // echo
    dline: Vec<f32>,
    dpos: usize,
    // dream filter
    lp_l: f32,
    lp_r: f32,
    detune: f32,
    clock: f32,
    pub flight: bool,
    wobble: f32,
    pub master: f32,
}

fn semi_to_freq(semi: i16) -> f32 {
    110.0f32 * (semi as f32 * 12.0f32).to_radians().exp() // 110 * 2^(s/12)
}

impl Music {
    pub fn new(rate: f32, seed: u64) -> Music {
        let mut rng = Rng::new(seed);
        let mut phrase = [-1i16; 16];
        for i in 0..16 {
            if rng.chance(0.3) {
                phrase[i] = rng.below(PENTA.len()) as i16;
            }
        }
        let dlen = (0.375 * rate) as usize;
        Music {
            rate,
            rng,
            voices: Vec::new(),
            next_chord: 0.0,
            prog: rng.below(3),
            prog_ticks: 0,
            next_mel: 0.0,
            mel_step: 0,
            phrase,
            next_bass: 0.0,
            bass_step: 0,
            next_hat: 0.0,
            hat_step: 0,
            chord: CM7,
            dline: vec![0.0; dlen],
            clock: 0.0,
            dpos: 0,
            lp_l: 0.0,
            lp_r: 0.0,
            detune: 1.0,
            flight: false,
            wobble: 0.0,
            // Safe ceiling: `fill` multiplies a `tanh` by this, so the output can
            // never exceed `master` no matter how many voices stack up.
            master: 0.85,
        }
    }

    pub fn set_flight(&mut self, flight: bool) {
        self.flight = flight;
    }

    pub fn footstep(&mut self, pan: f32) {
        self.voices.push(Voice {
            kind: V_FOOT,
            freq: 55.0,
            age: 0.0,
            dur: 0.16,
            vel: 0.5,
            pan,
            phase: 0.0,
            phase2: 0.0,
            phase3: 0.0,
        });
    }

    fn schedule(&mut self, t: f32) {
        if t >= self.next_chord {
            self.next_chord = t + CHORD_DUR;
            let prog = PROGRESSIONS[self.prog];
            let c = prog[self.prog_ticks % 4];
            self.chord = c;
            self.prog_ticks += 1;
            if self.prog_ticks % 8 == 0 && self.rng.chance(0.5) {
                self.prog = self.rng.below(3);
            }
            // Rhodes voicing: spread the chord around A3..A5
            for (i, iv) in c.1.iter().enumerate() {
                let oct = if i == 0 { 24 } else { 12 + 12 * (i as i16 / 3) };
                self.voices.push(Voice {
                    kind: V_RHODES,
                    freq: semi_to_freq(c.0 + iv + oct),
                    age: 0.0,
                    dur: CHORD_DUR * 1.6,
                    vel: 0.16 + self.rng.f32v() * 0.05,
                    pan: 0.5 * (i as f32 / 3.0 * 2.0 - 1.0),
                    phase: self.rng.f32v() * 6.28,
                    phase2: 0.0,
                    phase3: 0.0,
                });
            }
        }
        if t >= self.next_bass {
            self.next_bass = t + BEAT;
            let step = self.bass_step % 4;
            self.bass_step += 1;
            let semi = match step {
                0 | 2 => self.chord.0,
                3 => self.chord.0 + 7,
                _ => return,
            };
            self.voices.push(Voice {
                kind: V_BASS,
                freq: semi_to_freq(semi),
                age: 0.0,
                dur: BEAT * 0.9,
                vel: 0.24,
                pan: 0.0,
                phase: 0.0,
                phase2: 0.0,
                phase3: 0.0,
            });
        }
        if t >= self.next_mel {
            self.next_mel = t + EIGHTH;
            let step = self.mel_step % 16;
            self.mel_step += 1;
            if step == 0 && self.rng.chance(0.6) {
                // new phrase
                for i in 0..16 {
                    self.phrase[i] = if self.rng.chance(0.28) {
                        self.rng.below(PENTA.len()) as i16
                    } else {
                        -1
                    };
                }
            }
            let p = self.phrase[step as usize];
            if p >= 0 {
                let semi = self.chord.0 + 24 + PENTA[p as usize];
                self.voices.push(Voice {
                    kind: V_MEL,
                    freq: semi_to_freq(semi),
                    age: 0.0,
                    dur: 1.1,
                    vel: 0.13 + self.rng.f32v() * 0.05,
                    pan: self.rng.range(-0.6, 0.6),
                    phase: 0.0,
                    phase2: 0.0,
                    phase3: 0.0,
                });
            }
        }
        if t >= self.next_hat {
            self.next_hat = t + EIGHTH;
            let step = self.hat_step;
            self.hat_step += 1;
            if step % 2 == 1 {
                self.voices.push(Voice {
                    kind: V_HAT,
                    freq: 0.0,
                    age: 0.0,
                    dur: 0.05,
                    vel: 0.05 + self.rng.f32v() * 0.02,
                    pan: if step % 4 == 1 { -0.3 } else { 0.3 },
                    phase: 0.0,
                    phase2: 0.0,
                    phase3: 0.0,
                });
            }
        }
    }

    fn voice_sample(&mut self, v: &mut Voice, t: f32) -> (f32, f32) {
        let x = v.age;
        if x > v.dur {
            return (0.0, 0.0);
        }
        let det = self.detune * (1.0 + self.wobble * 0.004 * (t * 0.7).sin());
        let f = v.freq * det;
        let dt = 1.0 / self.rate;
        match v.kind {
            V_RHODES => {
                v.phase += f * dt;
                v.phase2 += f * 2.008 * dt;
                v.phase3 += f * 3.01 * dt;
                let env = (1.0 - x / v.dur).powi(2) * (1.0 - (-x * 140.0).exp()) * (1.0 - x * 0.5);
                let s = v.phase.sin()
                    + 0.35 * v.phase2.sin()
                    + 0.18 * v.phase3.sin()
                    + 0.06 * (v.phase * 4.77).sin();
                let s = s * env * v.vel;
                (s * (0.5 - v.pan * 0.5), s * (0.5 + v.pan * 0.5))
            }
            V_BASS => {
                v.phase += f * dt;
                let env = (1.0 - x / v.dur).powi(3) * (1.0 - (-x * 300.0).exp());
                let s = (v.phase.sin() + 0.3 * (v.phase * 2.0).sin()) * env * v.vel;
                (s, s)
            }
            V_MEL => {
                v.phase += f * dt;
                v.phase2 += f * 2.01 * dt;
                let env = (1.0 - x / v.dur).powi(2) * (1.0 - (-x * 90.0).exp());
                let breath = if (x * self.rate as f32) as i64 % 2 == 0 { 0.015 } else { -0.015 };
                let s = (v.phase.sin() * 0.8 + v.phase2.sin() * 0.25 + breath) * env * v.vel;
                (s * (0.5 - v.pan * 0.5), s * (0.5 + v.pan * 0.5))
            }
            V_HAT => {
                let env = (-x * 70.0).exp();
                let n = if ((x * self.rate as f32 * 13.7) as i64) % 2 == 0 { 1.0 } else { -1.0 };
                let s = n * env * v.vel;
                (s * (0.5 - v.pan * 0.5), s * (0.5 + v.pan * 0.5))
            }
            _ => {
                // footstep: low thump + scuff
                v.phase += f * dt;
                let env = (-x * 22.0).exp();
                let scuff = if ((x * self.rate as f32 * 7.3) as i64) % 2 == 0 { 0.5 } else { -0.5 };
                let s = (v.phase.sin() * 0.8 + scuff * 0.5) * env * v.vel;
                (s * (0.5 - v.pan * 0.5), s * (0.5 + v.pan * 0.5))
            }
        }
    }

    /// Fill interleaved stereo samples.
    pub fn fill(&mut self, out: &mut [f32]) {
        let n = out.len() / 2;
        let dt = 1.0 / self.rate;
        let dlen = self.dline.len();
        for i in 0..n {
            let time = self.clock + i as f32 * dt;
            self.schedule(time);
            self.wobble = if self.flight {
                1.0 + 0.6 * (time * 0.9).sin()
            } else {
                1.0
            };
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            let mut voices = std::mem::take(&mut self.voices);
            for v in voices.iter_mut() {
                let (a, b) = self.voice_sample(v, time);
                l += a;
                r += b;
                v.age += dt;
            }
            self.voices = voices;
            self.voices.retain(|v| v.age <= v.dur);
            // echo (mono tap)
            let d = self.dline[self.dpos];
            let mix = (l + r) * 0.5;
            self.dline[self.dpos] = mix + d * 0.35;
            self.dpos = (self.dpos + 1) % dlen;
            l += d * 0.25;
            r += d * 0.25;
            // The flight mix is the *bright* one: the same gentle dream smear as
            // walking, a touch more top end so it cuts through, and the pitch
            // wobble above. It used to sit at k≈0.02 — a one-pole corner near
            // 140 Hz, which left nothing but bass and cost 12 dB against the
            // walking mix, so the crossfade into flight just went quiet.
            let k = if self.flight {
                0.55 + 0.1 * (time * 0.5).sin()
            } else {
                0.35
            };
            self.lp_l += (l - self.lp_l) * k;
            self.lp_r += (r - self.lp_r) * k;
            let (l, r) = (
                l * 0.7 + self.lp_l * 0.3,
                r * 0.7 + self.lp_r * 0.3,
            );
            out[i * 2] = l.tanh() * self.master;
            out[i * 2 + 1] = r.tanh() * self.master;
        }
        self.clock += n as f32 * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_sound_and_stays_finite() {
        let mut m = Music::new(48000.0, 42);
        let mut buf = vec![0.0f32; 4800 * 2];
        let mut peak = 0.0f32;
        for _ in 0..200 {
            m.fill(&mut buf);
            for s in &buf {
                assert!(s.is_finite());
                peak = peak.max(s.abs());
            }
        }
        assert!(peak > 0.05, "muzak should actually play, peak={peak}");
        assert!(peak <= 1.0);
    }

    #[test]
    fn deterministic() {
        let mut a = Music::new(48000.0, 7);
        let mut b = Music::new(48000.0, 7);
        let mut ba = vec![0.0f32; 997 * 2];
        let mut bb = vec![0.0f32; 997 * 2];
        for _ in 0..50 {
            a.fill(&mut ba);
            b.fill(&mut bb);
        }
        assert_eq!(ba, bb);
    }

    #[test]
    fn footstep_makes_noise() {
        let mut m = Music::new(48000.0, 3);
        let mut buf = vec![0.0f32; 2048 * 2];
        // drain any scheduled onset first
        m.fill(&mut buf);
        m.footstep(0.0);
        let mut peak_after = 0.0f32;
        m.fill(&mut buf);
        for s in &buf {
            peak_after = peak_after.max(s.abs());
        }
        assert!(peak_after > 0.01);
    }
}

