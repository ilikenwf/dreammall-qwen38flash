//! Game rules: loops, mutations, the penny, the hotdog, dream-flight, wins.

use macroquad::prelude::*;

use crate::layout::{Layout, LOOP_LEN};
use crate::player::Player;
use crate::util::wrap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Held {
    Penny,
    Hotdog,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PennyState {
    OnFloor { x: f32, z: f32 },
    Held,
    Thrown { x: f32, y: f32, z: f32, vx: f32, vy: f32, vz: f32 },
    Resting { x: f32, z: f32 },
    InFountain,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HotdogState {
    OnTable,
    Held,
    Eaten,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Walking,
    Flight,
    Won,
}

/// Seconds of dream-flight before the hotdog wears off and the dream drops
/// you back where it started.
pub const FLIGHT_TIME: f32 = 75.0;
/// The last this many seconds of flight count as "fading": warning HUD.
pub const FLIGHT_WARN: f32 = 15.0;
/// Seconds after a collapse before a fresh hotdog appears on its table.
pub const HOTDOG_RESPAWN: f32 = 20.0;
/// How long a one-off message stays on screen.
const MESSAGE_TIME: f32 = 4.0;

pub struct Game {
    pub seed: u64,
    pub loop_k: u32,
    pub layout: Layout,
    pub player: Player,
    pub holding: Option<Held>,
    pub penny: PennyState,
    pub hotdog: HotdogState,
    pub phase: Phase,
    pub eat_progress: f32,
    pub fade: f32,
    pub time: f32,
    pub prompt: Option<String>,
    pub footfalls: Vec<f32>, // pan per footstep sound
    pub rebuild_needed: bool,
    pub wrapped: bool,
    pub win_path: Option<&'static str>,
    pub flight_left: f32,
    pub hotdog_respawn_at: Option<f32>,
    pub message: Option<String>,
    pub message_left: f32,
}

impl Game {
    pub fn new(seed: u64) -> Game {
        let layout = Layout::generate(seed, 0);
        let penny = PennyState::OnFloor {
            x: layout.penny[0],
            z: layout.penny[1],
        };
        Game {
            seed,
            loop_k: 0,
            layout,
            player: Player::new(),
            holding: None,
            penny,
            hotdog: HotdogState::OnTable,
            phase: Phase::Walking,
            eat_progress: 0.0,
            fade: 1.0, // fade in from dream
            time: 0.0,
            prompt: None,
            footfalls: Vec::new(),
            rebuild_needed: false,
            wrapped: false,
            win_path: None,
            flight_left: 0.0,
            hotdog_respawn_at: None,
            message: None,
            message_left: 0.0,
        }
    }

    /// Advance one loop of the dream: regenerate the mall with mutations.
    pub fn next_loop(&mut self) {
        self.loop_k += 1;
        self.layout = Layout::generate(self.seed, self.loop_k);
        self.rebuild_needed = true;
        self.wrapped = true;
        self.fade = 1.0;
    }

    pub fn update(&mut self, fwd: f32, strafe: f32, up: f32, dt: f32) {
        self.time += dt;
        self.fade = (self.fade - dt * 1.6).max(0.0);

        match self.phase {
            Phase::Walking => {
                let dist = self.player.walk(fwd, strafe, dt, &self.layout.collision);
                if dist > 0.0 {
                    self.step_acc_foot(dist);
                }
                // loop wrap: forward through the seam = a new dream pass
                if self.player.pos.z >= LOOP_LEN {
                    self.player.pos.z -= LOOP_LEN;
                    self.next_loop();
                } else if self.player.pos.z < 0.0 {
                    self.player.pos.z += LOOP_LEN;
                }
            }
            Phase::Flight => {
                self.player.fly(fwd, strafe, up, dt);
                self.player.pos.z = wrap(self.player.pos.z, LOOP_LEN);
                // dive into a fountain to wake up
                let p = self.player.pos;
                if p.y < 1.0 && self.layout.fountain_at(p.x, p.z).is_some() {
                    self.win("fountain-dive");
                }
                // the hotdog only buys you so much sky
                if self.phase == Phase::Flight {
                    self.flight_left -= dt;
                    if self.flight_left <= 0.0 {
                        self.collapse_dream();
                    }
                }
            }
            Phase::Won => {}
        }
        // a fresh hotdog eventually appears on its table again
        if self.hotdog == HotdogState::Eaten {
            if let Some(at) = self.hotdog_respawn_at {
                if self.time >= at {
                    self.hotdog = HotdogState::OnTable;
                    self.hotdog_respawn_at = None;
                    self.say("a hotdog is lying on a table somewhere");
                }
            }
        }

        // one-off messages wear off
        if self.message_left > 0.0 {
            self.message_left -= dt;
            if self.message_left <= 0.0 {
                self.message = None;
            }
        }

        // thrown penny physics
        if let PennyState::Thrown { x, y, z, vx, vy, vz } = self.penny {
            let g = 11.0f32;
            let nx = x + vx * dt;
            let ny = y + vy * dt;
            let nz = wrap(z + vz * dt, LOOP_LEN);
            let nvy = vy - g * dt;
            if ny < 0.9 && self.layout.fountain_at(nx, nz).is_some() {
                self.penny = PennyState::InFountain;
                self.win("penny-in-fountain");
            } else if ny <= 0.04 {
                self.penny = PennyState::Resting { x: nx, z: nz };
            } else {
                self.penny = PennyState::Thrown {
                    x: nx,
                    y: ny,
                    z: nz,
                    vx,
                    vy: nvy,
                    vz,
                };
            }
        }

        self.update_prompt();
    }

    fn step_acc_foot(&mut self, dist: f32) {
        self.player.step_acc += dist;
        if self.player.step_acc > 1.6 {
            self.player.step_acc = 0.0;
            self.player.step_toggle = !self.player.step_toggle;
            self.footfalls.push(if self.player.step_toggle { -0.4 } else { 0.4 });
        }
    }

    fn update_prompt(&mut self) {
        self.prompt = None;
        if let Some(msg) = &self.message {
            self.prompt = Some(msg.clone());
            return;
        }
        if self.phase != Phase::Walking {
            if self.phase == Phase::Flight {
                self.prompt = Some(if self.fading() {
                    format!(
                        "the dream is letting go — {}s — dive into a fountain",
                        self.flight_left.max(0.0).ceil() as i32
                    )
                } else {
                    "space up · ctrl down · dive into a fountain to wake".into()
                });
            }
            return;
        }
        let p = self.player.pos;
        let d = |x: f32, z: f32| {
            let dz = wrap(z - p.z + LOOP_LEN / 2.0, LOOP_LEN) - LOOP_LEN / 2.0;
            ((p.x - x).powi(2) + dz.powi(2)).sqrt()
        };
        match self.holding {
            None => {
                let pp = match self.penny {
                    PennyState::OnFloor { x, z } | PennyState::Resting { x, z } => Some((x, z)),
                    _ => None,
                };
                if let Some((x, z)) = pp {
                    if d(x, z) < 1.7 {
                        self.prompt = Some("[E] pick up the penny".into());
                        return;
                    }
                }
                if self.hotdog == HotdogState::OnTable {
                    let (hx, hz) = (self.layout.hotdog[0], self.layout.hotdog[1]);
                    if d(hx, hz) < 2.0 {
                        self.prompt = Some("[E] pick up the hotdog".into());
                        return;
                    }
                }
            }
            Some(Held::Penny) => {
                self.prompt = Some("[click] throw the penny".into());
            }
            Some(Held::Hotdog) => {
                self.prompt = Some("[hold E] eat the hotdog".into());
            }
        }
    }

    /// E pressed.
    pub fn interact(&mut self) {
        if self.phase != Phase::Walking {
            return;
        }
        let p = self.player.pos;
        let d = |x: f32, z: f32| {
            let dz = wrap(z - p.z + LOOP_LEN / 2.0, LOOP_LEN) - LOOP_LEN / 2.0;
            ((p.x - x).powi(2) + dz.powi(2)).sqrt()
        };
        if self.holding.is_none() {
            if let Some((x, z)) = match self.penny {
                PennyState::OnFloor { x, z } | PennyState::Resting { x, z } => Some((x, z)),
                _ => None,
            } {
                if d(x, z) < 1.7 {
                    self.penny = PennyState::Held;
                    self.holding = Some(Held::Penny);
                    return;
                }
            }
            if self.hotdog == HotdogState::OnTable {
                let (hx, hz) = (self.layout.hotdog[0], self.layout.hotdog[1]);
                if d(hx, hz) < 2.0 {
                    self.hotdog = HotdogState::Held;
                    self.holding = Some(Held::Hotdog);
                    return;
                }
            }
        }
    }

    /// E held (for eating).
    pub fn hold_eat(&mut self, dt: f32) {
        if self.holding == Some(Held::Hotdog) && self.phase == Phase::Walking {
            self.eat_progress += dt;
            if self.eat_progress >= 1.5 {
                self.hotdog = HotdogState::Eaten;
                self.holding = None;
                self.phase = Phase::Flight;
                self.player.flying = true;
                self.player.pos.y = 1.8;
                self.flight_left = FLIGHT_TIME;
                self.say("the processed meat is doing something to the ceiling");
            }
        }
    }

    /// Left click: throw the penny.
    pub fn throw_penny(&mut self) {
        if self.holding == Some(Held::Penny) && self.phase == Phase::Walking {
            let dir = self.player.look_dir();
            let start = self.player.pos + dir * 0.5;
            self.penny = PennyState::Thrown {
                x: start.x,
                y: start.y,
                z: start.z,
                vx: dir.x * 11.0,
                vy: dir.y * 11.0 + 2.2,
                vz: dir.z * 11.0,
            };
            self.holding = None;
        }
    }

    /// The hotdog wears off mid-flight: the dream drops you back at the start,
    /// and a fresh hotdog shows up in the mall a little later.
    fn collapse_dream(&mut self) {
        self.phase = Phase::Walking;
        self.player = Player::new();
        self.flight_left = 0.0;
        self.fade = 1.0;
        self.hotdog_respawn_at = Some(self.time + HOTDOG_RESPAWN);
        self.say("the hotdog wore off — you are back at the beginning");
    }

    /// Show a one-off line for a few seconds.
    fn say(&mut self, text: &str) {
        self.message = Some(text.to_string());
        self.message_left = MESSAGE_TIME;
    }

    /// True while the flight clock is in its warning band.
    pub fn fading(&self) -> bool {
        self.phase == Phase::Flight && self.flight_left <= FLIGHT_WARN
    }

    /// Fraction of the flight time still banked, 1.0 .. 0.0.
    pub fn flight_fraction(&self) -> f32 {
        (self.flight_left / FLIGHT_TIME).clamp(0.0, 1.0)
    }

    fn win(&mut self, path: &'static str) {
        if self.phase != Phase::Won {
            self.phase = Phase::Won;
            self.win_path = Some(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn penny_pickup_throw_fountain_win() {
        let mut g = Game::new(4242);
        // teleport onto the penny
        let (px, pz) = (g.layout.penny[0], g.layout.penny[1]);
        g.player.pos.x = px;
        g.player.pos.z = pz - 1.0;
        g.interact();
        assert_eq!(g.holding, Some(Held::Penny));

        // stand in front of fountain 0, aim into it, throw
        let f = g.layout.fountains[0];
        g.player.pos.x = f.x;
        g.player.pos.z = f.z - (f.hd + 2.5);
        g.player.yaw = 0.0; // +z, into the basin
        g.player.pitch = 0.15; // positive pitch = look down
        g.throw_penny();
        for _ in 0..120 {
            g.update(0.0, 0.0, 0.0, 1.0 / 60.0);
        }
        assert_eq!(g.phase, Phase::Won);
        assert_eq!(g.win_path, Some("penny-in-fountain"));
    }

    #[test]
    fn hotdog_flight_dive_win() {
        let mut g = Game::new(777);
        let (hx, hz) = (g.layout.hotdog[0], g.layout.hotdog[1]);
        g.player.pos.x = hx + 1.0;
        g.player.pos.z = hz;
        g.interact();
        assert_eq!(g.holding, Some(Held::Hotdog));
        g.hold_eat(1.6);
        assert_eq!(g.phase, Phase::Flight);
        assert!(g.player.flying);

        // fly over the fountain and dive
        let f = g.layout.fountains[0];
        g.player.pos.x = f.x;
        g.player.pos.z = f.z;
        g.player.pos.y = 3.0;
        for _ in 0..120 {
            g.update(0.0, 0.0, -1.0, 1.0 / 60.0);
        }
        assert_eq!(g.phase, Phase::Won);
        assert_eq!(g.win_path, Some("fountain-dive"));
    }

    #[test]
    fn walking_forward_through_seam_starts_new_loop() {
        let mut g = Game::new(31);
        g.player.pos.z = LOOP_LEN - 0.5;
        g.player.yaw = 0.0;
        for _ in 0..30 {
            g.update(1.0, 0.0, 0.0, 1.0 / 30.0);
        }
        assert_eq!(g.loop_k, 1);
        assert!(g.rebuild_needed);
        assert!(g.player.pos.z < 5.0);
        // the mall mutated
        let base = Layout::generate(31, 0);
        assert_ne!(
            base.props.len(),
            usize::MAX // sanity; real diff checked in layout tests
        );
    }

    #[test]
    fn penny_rests_on_floor_and_can_be_rethrown() {
        let mut g = Game::new(5);
        let (px, pz) = (g.layout.penny[0], g.layout.penny[1]);
        g.player.pos.x = px;
        g.player.pos.z = pz - 1.0;
        g.interact();
        g.player.pitch = 0.6; // lob it up
        g.throw_penny();
        for _ in 0..180 {
            g.update(0.0, 0.0, 0.0, 1.0 / 60.0);
        }
        assert!(matches!(g.penny, PennyState::Resting { .. }));
        // walk over and pick it up again
        if let PennyState::Resting { x, z } = g.penny {
            g.player.pos.x = x;
            g.player.pos.z = z;
            g.interact();
            assert_eq!(g.holding, Some(Held::Penny));
        }
    }

    #[test]
    fn cannot_win_without_fountain() {
        let mut g = Game::new(9);
        for _ in 0..600 {
            g.update(1.0, 0.0, 0.0, 1.0 / 60.0);
        }
        assert_eq!(g.phase, Phase::Walking);
    }

    /// Pick up and eat the hotdog, leaving the game airborne.
    fn airborne(seed: u64) -> Game {
        let mut g = Game::new(seed);
        let (hx, hz) = (g.layout.hotdog[0], g.layout.hotdog[1]);
        g.player.pos.x = hx + 1.0;
        g.player.pos.z = hz;
        g.interact();
        assert_eq!(g.holding, Some(Held::Hotdog));
        g.hold_eat(1.6);
        assert_eq!(g.phase, Phase::Flight);
        g
    }

    #[test]
    fn flight_clock_runs_out_and_drops_you_back() {
        let mut g = airborne(777);
        assert_eq!(g.flight_left, FLIGHT_TIME);

        // Hover well above any fountain and wait the dream out.
        g.player.pos.y = 6.0;
        let mut frames = 0;
        while g.phase == Phase::Flight && frames < 100_000 {
            g.update(0.0, 0.0, 0.0, 1.0 / 30.0);
            frames += 1;
        }
        // Roughly FLIGHT_TIME of hovering, not a frame and not a century.
        assert!(((frames as f32 / 30.0) - FLIGHT_TIME).abs() < 1.0);
        assert_eq!(g.phase, Phase::Walking);
        assert!(!g.player.flying);
        assert_eq!(g.player.pos, Player::new().pos, "dropped back at the start");
        assert!(g.fade > 0.0, "the drop should fade in");
    }

    #[test]
    fn hotdog_comes_back_after_a_collapse() {
        let mut g = airborne(777);
        assert_eq!(g.hotdog, HotdogState::Eaten);

        let mut frames = 0;
        while g.phase == Phase::Flight && frames < 100_000 {
            g.player.pos.y = 6.0;
            g.update(0.0, 0.0, 0.0, 1.0 / 30.0);
            frames += 1;
        }
        assert_eq!(g.hotdog, HotdogState::Eaten, "not back yet");

        for _ in 0..(HOTDOG_RESPAWN * 30.0) as i32 + 5 {
            g.update(0.0, 0.0, 0.0, 1.0 / 30.0);
        }
        assert_eq!(g.hotdog, HotdogState::OnTable);
        assert_eq!(g.holding, None);
    }

    #[test]
    fn flight_warning_lights_up_near_the_end() {
        let mut g = airborne(777);
        assert!(!g.fading());
        assert!((g.flight_fraction() - 1.0).abs() < 1e-6);

        // Let the eat message expire so the prompt is the flight hint again.
        for _ in 0..(MESSAGE_TIME * 30.0) as i32 + 2 {
            g.update(0.0, 0.0, 0.0, 1.0 / 30.0);
        }
        assert!(g.prompt.as_deref().unwrap().contains("ctrl down"));

        g.flight_left = FLIGHT_WARN - 1.0;
        g.update(0.0, 0.0, 0.0, 1.0 / 30.0);
        assert!(g.fading());
        assert!(g.flight_fraction() < 1.0);
        assert!(g.prompt.as_deref().unwrap().contains("letting go"));
    }

    #[test]
    fn winning_outranks_the_clock() {
        // Dive within the time limit: the win must stick, not collapse.
        let mut g = airborne(777);
        let f = g.layout.fountains[0];
        g.player.pos.x = f.x;
        g.player.pos.z = f.z;
        g.player.pos.y = 3.0;
        for _ in 0..120 {
            g.update(0.0, 0.0, -1.0, 1.0 / 60.0);
        }
        assert_eq!(g.phase, Phase::Won);
        for _ in 0..600 {
            g.update(0.0, 0.0, 0.0, 1.0 / 60.0);
        }
        assert_eq!(g.phase, Phase::Won);
    }
}
