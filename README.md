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

Sound is on by default and needs no assets: the whole soundtrack is synthesised
into memory at boot, from the mall's seed.

macroquad 0.4.16 ships `audio` as a **non-default** feature, so `Cargo.toml`
enables it explicitly — without it every call is a silent no-op stub. With it on,
`quad-snd` spawns an ALSA thread during startup that panics outright if no PCM
device can be opened, and then prints `Audio thread died` on every call, forever.

So the game treats quad-snd's own failure as the diagnosis: that startup panic is
swallowed and recorded as a flag, and `Sfx::boot` will not call into a dead audio
thread. A machine with no sound card gets one line on the title screen,
`audio: no output device — playing silent`, and an otherwise identical, silent
dream. Nothing is probed or guessed first: a wrong "no device" mutes a machine
that has one, and no amount of sniffing `/dev/snd` beats the backend actually
reporting that it failed.

libasound still prints its own `cannot find card '0'` errors, because macroquad
builds its audio context before any game code runs and quad-snd tries to open a
device regardless. Nine lines, once, at startup — which is what every native
ALSA program prints on such a box. The only API that could silence them,
`snd_lib_error_set_handler`, takes a C-variadic callback, and declaring a
variadic `extern "C"` fn is unstable Rust (feature `c_variadic`, rust#44930): it
does not compile on stable, so the game does not try.

| Switch | Effect |
| --- | --- |
| `--no-audio` or `DREAMMALL_AUDIO=0` | force silence |
| `DREAMMALL_AUDIO=1` | play anyway, even if the audio thread died — the title screen then reads `audio: forced on — <reason>` |
| `DREAMMALL_AUDIO_VERBOSE=1` | keep quad-snd's real panic report, for debugging audio |
| `--audio-check` | open the device, print the whole diagnosis, exit |

Linux needs `libasound` at runtime (already a dependency of the audio backend).

## No sound?

Start here. One command, one pasteable block, nothing to interpret:

```sh
./target/release/dreammall --audio-check
```

```text
quad-snd: Can't open PCM device. (alsa_snd.rs:30)
title screen: audio: no output device — playing silent
$ aplay -l
aplay: device_list:277: no sound cards found (-5)
$ aplay -L
default
… 41 more
~/.asoundrc: absent
/etc/asound.conf: absent
$DREAMMALL_AUDIO: unset
$DREAMMALL_AUDIO_VERBOSE: unset
$ALSA_CARD: unset
$DISPLAY: :1
DREAMMALL_AUDIO_VERBOSE=1 --audio-check also prints the panic and backtrace.
```

It forces the attempt — so a machine that gave up earlier still answers — then
prints what quad-snd said, what ALSA believes exists, and which config files and
environment variables are in play. That block is the thing to paste into a bug
report; on a working device the first two lines read `no failure recorded` and
`audio: on`.

The bottom-left of the title screen says which branch a normal launch took:

| Title screen | Meaning |
| --- | --- |
| `audio: off` | `--no-audio` / `DREAMMALL_AUDIO=0`, or a headless mode |
| `audio: <reason> — playing silent` | quad-snd's mixer thread died; reasons below |
| `audio: forced on — <reason>` | you insisted with `DREAMMALL_AUDIO=1`; it is still not going to work |
| `audio: on` | samples are leaving the process |

The `<reason>` is quad-snd's failure, translated. Every panic the audio backend
can produce is here — all thirteen, taken from the dependency's source and pinned
by a test, because the first version of this table guessed at the wording and a
real failure slipped straight through it:

| On screen | quad-snd's words | What it means |
| --- | --- | --- |
| `no output device` | `Can't open PCM device.` | `snd_pcm_open` failed on both `default` and `pipewire` |
| `device refuses 32-bit float output` | `Can't set SND_PCM_FORMAT_FLOAT_LE format` | quad-snd only ever asks for `FLOAT_LE`; a raw device that will not convert needs a `plug` layer in front of it |
| `device rejected the buffer size` | `Cant's set buffer size` | quad-snd asks for exactly 4096 frames; `dmix` and some plugins refuse |
| `device advertises no sample rate` | `Can't set rate.` | `set_rate_near` rounds to the nearest supported rate, so failing at all means the device advertised *no* rate: a broken `route`/`dmix`/`plug` chain, not missing hardware |
| `device rejected stereo` | `Can't set channels number.` | the device will not run with two channels |
| `device rejected interleaved audio` | `Can't set interleaved mode` | the device will not do interleaved samples |
| `device refused its own hardware settings` | `Can't set harware parameters.` | `snd_pcm_hw_params` was refused outright — the crate's spelling, not ours |
| `device would not start playing` | `cannot prepare audio interface for use` | `snd_pcm_prepare` failed |
| `device refused the start mode` | `cannot set start mode` | the software parameters were refused |
| `device refused its playback settings` | `cannot … software parameters …` | allocating, initialising or setting `sw_params` failed |
| `device never became ready` | `PCM device is not ready` | the stream opened but never reached `PREPARED` |
| `device failed to start` | anything else | unrecognised — stderr has the exact panic |

Whichever it is, the same failure is printed once to stderr in the dependency's
own words, with a file and line. That line is the thing to paste into a bug
report; `DREAMMALL_AUDIO_VERBOSE=1` adds the panic backtrace.

```text
audio: quad-snd could not start — Can't open PCM device. (alsa_snd.rs:30)
```

Three of these — `FLOAT_LE`, the buffer size, and the rate — are a device that
exists, works, and refuses one parameter. That is an ALSA routing problem, not
missing hardware, and a `plug` layer in front of the device answers all three by
converting on its behalf. Only do this if `default` points straight at a raw
`hw:` device: on a PulseAudio or PipeWire desktop `default` already converts, and
this would bypass the mixer instead of helping it.

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
game. Two things worth knowing: the first note lands about 7 s after launch,
because both 64-second tracks are synthesised before the first frame, and the mix
peaks near −7 dBFS, so a low volume setting can make it easy to miss.

`Audio thread died` comes from quad-snd, once for every sound handed to a mixer
thread that has already panicked. The game stops queueing the moment it notices
the thread is gone, so a *stream* of them is a bug rather than a warning to
ignore. A handful of libasound `cannot find card '0'` lines is not: that is ALSA
itself, and every native ALSA program prints them on a machine with no sound card.

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
