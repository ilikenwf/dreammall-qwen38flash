# DREAM MALL

*A small, complete, playable 3D procedurally generated dream simulator (Phase 1).*

You have always been here. Keep walking forward through a strange, mostly empty
shopping mall. There are two ways out.

## Run

```sh
cargo run --release
```

Linux needs the X11/GL runtime libs that macroquad's bundled GLFW links against
(`libX11`, `libXi`, `libxcb`, `libGL`; all standard desktop distros have them).
Windows builds and runs with `cargo run --release` out of the box — no extra
toolchain steps.

## Controls

| Key | Action |
| --- | --- |
| `W` | forward (the only way) |
| `A` / `D` | strafe |
| mouse | look (pointer lock) |
| `E` | use / pick up / throw; hold to eat |
| `Esc` | pause |
| `Space` | fly up (only once flying) |
| `Ctrl` / `C` | fly down (only once flying) |

There is no backward. The mall is that kind of place.

## The two ways out

1. **The penny.** Somewhere in the mall there is a copper penny. Pick it up
   (`E`), carry it to a fountain, and throw it in (`E`). Make a wish. Wake up.
2. **The hotdog.** Find the hotdog stand, take the hotdog (`E`), and eat it
   (`E`). The dream cannot survive that much processed meat: you lift off
   through the ceiling and the mall gets small from up-here.

   The hotdog only buys you about 75 seconds of sky. Dive into a fountain before
   the dream-fuel bar runs out, or the effect wears off and you wake up standing
   at the beginning again — with a fresh hotdog on a table somewhere, after a
   beat.

## How it works

- `layout.rs` — the mall: a ring of reusable section templates (corridor,
  atrium, food court, escalator hall) stitched into a looping plan, plus
  storefronts, mirrors, signs, props, seeded by `Rng`.
- `world.rs` / `geom.rs` — the whole layout is baked into a handful of merged
  vertex batches (macroquad has no model matrices); ceilings and ceiling
  lights are their own batches so the flight phase can cull them.
- `sunset.rs` + mirrors — each mirror renders the synthwave sunset (gradient
  sky, striped sun, neon grid, nyan cat) from a camera mirrored across the wall
  plane into a small render target, sampled by the mirror quad. Most hang on the
  side walls; the ones set into stub walls face back down the corridor, because
  you can never turn around and a side-wall mirror was only ever a glimpse.
- `music.rs` — procedural dream-music: software-synth WAV generated at boot from
  the seed; a second, brighter variant takes over during the flight.
- `game.rs` — state machine: walking, penny/hotdog logic, the flight clock,
  loop-wrap, endings.
- `sound.rs` — the soundtrack, and the audio-thread bookkeeping that keeps a
  machine with no sound card silent instead of dead (see [Audio](#audio)).

## Headless verification

```sh
cargo test                      # 33 unit tests (layout, rng, music, sound, game)
cargo run --release -- --smoke  # boots, simulates, prints SMOKE OK, exits
cargo run --release -- --shot /tmp/shots --seed 4242  # 6 PNGs, SHOTS OK
```

`--smoke` and `--shot` never touch audio or input; on a display-less machine:

```sh
LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -s "-screen 0 1280x800x24" \
  ./target/release/dreammall --smoke
```

## Audio

Sound is on by default and needs no assets: the soundtrack is generated at
runtime by `src/music.rs`, from the mall's seed, and written straight to the
device by `src/alsa.rs`.

Nothing about the device is assumed. At startup the game tries `default`, then
`pipewire`, then every playback stream it can find under `/proc/asound` — first
through `plughw:`, ALSA's own conversion layer, then raw `hw:` — and asks each one
which sample formats it has and which rate it will actually run at. Whatever it
answers is the rate the synthesiser is built at, so a 48 kHz-only machine gets
48 kHz music instead of a refusal, and there is no resampler anywhere because
there is nothing to resample. The title screen says what was agreed:

```text
audio: on · plughw:0,0 @ 48000 Hz S16
```

If nothing opens, it says why, in the device's own words, and prints every device
it tried and what each one said to stderr once:

```text
audio: device advertises no sample rate — playing silent
```

| Switch | Effect |
| --- | --- |
| `--no-audio` or `DREAMMALL_AUDIO=0` | force silence |
| `DREAMMALL_AUDIO=1` | print every device tried and what each said, on a normal launch |
| `--audio-check` | open the device, print the whole diagnosis, exit |

`libasound` is linked directly, with `#[link(name = "asound")]`; nothing sits
between the game and ALSA. macroquad's `audio` feature is deliberately off: it
pulls in `quad-snd`, which opened `default`, demanded 44100 Hz / `FLOAT_LE` / a
4096-frame buffer, and panicked on a thread nobody else could observe when any of
that was refused.

libasound still prints its own `cannot find card '0'` lines on a machine with no
sound card, because that is what every native ALSA program prints there. The only
API that could silence them, `snd_lib_error_set_handler`, takes a C-variadic
callback, and declaring a variadic `extern "C"` fn is unstable Rust (feature
`c_variadic`, rust#44930), so the game does not try.

## No sound?

Start here. One command, one pasteable block, nothing to interpret:

```sh
./target/release/dreammall --audio-check
```

```text
font: ./assets/LiberationSans-Bold.ttf (414568 bytes, fnv d0821eaeb9b16f0c)
title screen: audio: on · default @ 44100 Hz F32
tried:
  default  → cannot open (No such file or directory)
  pipewire → cannot open (No such file or directory)
  plughw:0,0 → plughw:0,0 @ 48000 Hz S32
$ aplay -l
card 0: PCH [HDA Intel PCH], device 0: ALC892 Analog
$ aplay -L
default
… 41 more
~/.asoundrc: absent
/etc/asound.conf: absent
$DREAMMALL_AUDIO: unset
$ALSA_CARD: unset
$DISPLAY: :1
```

The `tried:` list is the whole story of a launch: every name, in the order they
were attempted, with what each one said. A device further down working while
`default` fails is normal and fine — that is the fallback doing its job. Paste the
whole block into a bug report.

The bottom-left of the title screen says which branch a launch took:

| Title screen | Meaning |
| --- | --- |
| `audio: off` | `--no-audio` / `DREAMMALL_AUDIO=0`, or a headless mode |
| `audio: on · <device> @ <rate> Hz <format>` | samples are leaving the process, and that is what it agreed to |
| `audio: <reason> — playing silent` | every candidate refused; the reason is below |

The reasons come from `src/alsa.rs`, so they name the parameter that was refused
rather than guessing at hardware:

| Reason | What it means |
| --- | --- |
| `no output device` | every candidate failed to open at all — no card, or no permission on `/dev/snd` |
| `cannot open (…)` | that one name is not a device; ALSA's own words are in brackets |
| `device advertises no sample rate` | it opened but offered no rate whatsoever — a broken `route`/`dmix` chain |
| `device would not describe itself` | `snd_pcm_hw_params_any` failed on an open stream |
| `device will not do interleaved playback` | no `RW_INTERLEAVED` access |
| `device will not do stereo` | it cannot run with two channels |
| `device has no sample format we can produce` | none of `F32`, `S32`, `S16` |
| `device refused the settings it just described` | `snd_pcm_hw_params` rejected the negotiated set |
| `device would not start playing` | `snd_pcm_prepare` failed |

A device that exists but refuses one parameter is an ALSA routing problem, not
missing hardware. The game already retries through `plughw:` for exactly that
reason; if even that fails and `default` points straight at a raw `hw:` device, a
`plug` layer answers on its behalf — but on a PulseAudio or PipeWire desktop
`default` already converts, and this would bypass the mixer instead of helping:

```text
pcm.!default {
    type plug
    slave.pcm "hw:0,0"
}
```

To see which case you are in, without rebuilding anything:

```sh
aplay -L | head -20    # what ALSA calls your devices, and whether default is a plug
alsamixer              # ... and whether the channel is merely muted
```

If it reads `audio: on` and the room is still quiet, the problem is below the
game. The mix peaks near −7 dBFS, so a low volume setting is easy to mistake for
silence; the first note lands on the first frame, since the soundtrack is
generated as it plays rather than pre-rendered before the window appears.

A handful of libasound `cannot find card '0'` lines on a machine with no sound
card is ALSA itself, not the game: every native ALSA program prints them there.

To hear the soundtrack with none of the game's audio path involved — no window,
no device, no display — write it to a file:

```sh
./target/release/dreammall --music bgm.wav --seed 4242
./target/release/dreammall --music flight.wav --seed 4242 --flight
```

64 seconds of 44.1 kHz stereo WAV, a pure function of the seed. If that file
sounds fine and the game is quiet, the mall is not what is broken.

## Seeds

`--seed <n>` fixes the mall. Same seed, same mall, same music, every time.
