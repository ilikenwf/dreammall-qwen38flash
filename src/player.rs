//! First-person controller. Walking: forward + strafe only — the dream will
//! not let you go back. Flight replaces all of it.

use macroquad::prelude::*;

use crate::layout::Rect;

pub const WALK_SPEED: f32 = 3.4;
pub const EYE: f32 = 1.62;
pub const RADIUS: f32 = 0.35;

#[derive(Clone)]
pub struct Player {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub vel: Vec3,
    pub flying: bool,
    pub bob: f32,
    pub step_acc: f32,
    pub step_toggle: bool,
}

impl Player {
    pub fn new() -> Player {
        Player {
            pos: vec3(0.0, EYE, 2.0),
            yaw: 0.0,
            pitch: 0.0,
            vel: vec3(0.0, 0.0, 0.0),
            flying: false,
            bob: 0.0,
            step_acc: 0.0,
            step_toggle: false,
        }
    }

    pub fn look_dir(&self) -> Vec3 {
        let cp = self.pitch.cos();
        vec3(self.yaw.sin() * cp, -self.pitch.sin(), self.yaw.cos() * cp)
    }

    fn right(&self) -> Vec3 {
        vec3(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw += dx;
        self.pitch = (self.pitch + dy).clamp(-1.45, 1.45);
        if self.yaw > std::f32::consts::TAU {
            self.yaw -= std::f32::consts::TAU;
        }
        if self.yaw < 0.0 {
            self.yaw += std::f32::consts::TAU;
        }
    }

    /// Walk. `fwd` is clamped to >= 0: you cannot walk backward. Returns the
    /// horizontal distance moved (for footfalls).
    pub fn walk(&mut self, fwd: f32, strafe: f32, dt: f32, collision: &[Rect]) -> f32 {
        let fwd = fwd.max(0.0);
        let dir = vec3(self.yaw.sin(), 0.0, self.yaw.cos());
        let want = dir * (fwd * WALK_SPEED) + self.right() * (strafe * WALK_SPEED * 0.8);
        let speed = want.length();
        self.bob += speed * dt * 2.2;
        self.pos.y = EYE + (self.bob * 3.0).sin() * 0.025 * (speed / WALK_SPEED).min(1.0);

        let before = self.pos;
        // axis-separated collision
        self.pos.x += want.x * dt;
        self.resolve_xz(collision, true);
        self.pos.z += want.z * dt;
        self.resolve_xz(collision, false);
        (self.pos.xz() - before.xz()).length()
    }

    fn resolve_xz(&mut self, collision: &[Rect], x_axis: bool) {
        for r in collision {
            let pad = RADIUS;
            if self.pos.x > r.x0 - pad
                && self.pos.x < r.x1 + pad
                && self.pos.z > r.z0 - pad
                && self.pos.z < r.z1 + pad
            {
                if x_axis {
                    let left = self.pos.x - (r.x0 - pad);
                    let right = (r.x1 + pad) - self.pos.x;
                    self.pos.x = if left < right { r.x0 - pad } else { r.x1 + pad };
                } else {
                    let near = self.pos.z - (r.z0 - pad);
                    let far = (r.z1 + pad) - self.pos.z;
                    self.pos.z = if near < far { r.z0 - pad } else { r.z1 + pad };
                }
            }
        }
    }

    /// Dream flight: no walls, no rules.
    pub fn fly(&mut self, fwd: f32, strafe: f32, up: f32, dt: f32) {
        let dir = self.look_dir();
        let right = self.right();
        let want = dir * (fwd * 9.0) + right * (strafe * 6.0) + vec3(0.0, up, 0.0) * 5.0;
        let k = 1.0 - (-6.0 * dt).exp();
        self.vel += (want - self.vel) * k;
        self.pos += self.vel * dt;
        if self.pos.y < 0.4 {
            self.pos.y = 0.4;
            self.vel.y = self.vel.y.max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Rect;

    #[test]
    fn cannot_walk_backward() {
        let mut p = Player::new();
        let start = p.pos.z;
        p.walk(-1.0, 0.0, 0.1, &[]);
        assert!((p.pos.z - start).abs() < 1e-5, "backward input must do nothing");
    }

    #[test]
    fn walks_forward_and_strafes() {
        let mut p = Player::new();
        p.walk(1.0, 0.0, 0.5, &[]);
        assert!(p.pos.z > 1.5);
        let z = p.pos.z;
        p.yaw = 0.0;
        p.walk(0.0, 1.0, 0.5, &[]);
        assert!((p.pos.x - 1.36).abs() < 0.2);
        assert!((p.pos.z - z).abs() < 0.05);
    }

    #[test]
    fn collision_blocks_walls() {
        let mut p = Player::new();
        p.yaw = std::f32::consts::FRAC_PI_2; // face +x
        let walls = [Rect { x0: 2.0, z0: -10.0, x1: 3.0, z1: 10.0 }];
        for _ in 0..60 {
            p.walk(1.0, 0.0, 0.05, &walls);
        }
        assert!(p.pos.x < 2.0 - RADIUS + 0.01, "must not tunnel: {}", p.pos.x);
    }

    #[test]
    fn flight_ignores_walls() {
        let mut p = Player::new();
        p.flying = true;
        p.yaw = std::f32::consts::FRAC_PI_2;
        let walls = [Rect { x0: 2.0, z0: -10.0, x1: 3.0, z1: 10.0 }];
        let _ = walls;
        for _ in 0..60 {
            p.fly(1.0, 0.0, 0.0, 0.05);
        }
        assert!(p.pos.x > 3.0, "flight passes through architecture");
    }
}
