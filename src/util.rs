//! Small math/color helpers shared by every module.

pub fn clamp(v: f32, a: f32, b: f32) -> f32 {
    if v < a {
        a
    } else if v > b {
        b
    } else {
        v
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// hue in [0,1), sat/val in [0,1] -> linear-ish sRGB triple
pub fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = (h - (h as f32).floor()) * 6.0;
    let i = h.floor() as i32;
    let f = h - h.floor();
    let (r, g, b) = match i % 6 {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0 - f),
    };
    [
        v * (1.0 - s + s * r),
        v * (1.0 - s + s * g),
        v * (1.0 - s + s * b),
    ]
}

pub fn shade(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn mixc(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// wrap x into [0, m)
pub fn wrap(x: f32, m: f32) -> f32 {
    let r = x % m;
    if r < 0.0 {
        r + m
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_sanity() {
        let r = hsv(0.0, 1.0, 1.0);
        assert!((r[0] - 1.0).abs() < 1e-5 && r[1].abs() < 1e-5 && r[2].abs() < 1e-5);
        let g = hsv(1.0 / 3.0, 1.0, 1.0);
        assert!(g[1] > 0.99);
    }

    #[test]
    fn wrap_negative() {
        assert!((wrap(-0.5, 10.0) - 9.5).abs() < 1e-5);
        assert!(wrap(10.5, 10.0) < 0.51);
    }
}
