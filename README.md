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
- `sound.rs` — the soundtrack, and the device sniffing that keeps a machine with
  no sound card silent instead of dead (see [Audio](#audio)).

## Headless verification

```sh
cargo test                      # 34 unit tests (layout, rng, music, sound, game)
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

So the game refuses to touch the audio API until it has found something to play
through — an ALSA playback node under `/dev/snd`, or a PipeWire/Pulse socket —
and it filters that one startup panic out of the report. A machine with no sound
card gets one line on the title screen, `audio: no output device — playing
silent`, and an otherwise identical, silent dream.

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
| `DREAMMALL_AUDIO_VERBOSE=1` | keep quad-snd's real panic report, for debugging audio |

Linux needs `libasound` at runtime (already a dependency of the audio backend).

## Seeds

`--seed <n>` fixes the mall. Same seed, same mall, same music, every time.
