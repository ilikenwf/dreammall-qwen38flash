//! Deterministic splitmix64-based RNG. Everything in the dream derives from a seed.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// f64 in [0,1)
    pub fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// f32 in [0,1)
    pub fn f32v(&mut self) -> f32 {
        self.f() as f32
    }

    /// f32 in [a,b)
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32v()
    }

    /// integer in [0,n)
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// true with probability p
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32v() < p
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// Stable sub-stream for (seed, loop, id) — mutations are reproducible.
pub fn sub_rng(seed: u64, loop_k: u32, id: u64) -> Rng {
    let mut h = seed;
    h ^= 0xA53F_9C7E_1D2B_6F01u64.wrapping_mul(loop_k as u64 + 0x1000193);
    h = h.wrapping_add(id.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    Rng::new(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn ranges_in_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.range(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&v));
            assert!((0..10).contains(&r.below(10)));
        }
    }

    #[test]
    fn sub_streams_differ_per_loop() {
        let a: u64 = sub_rng(1, 0, 5).next_u64();
        let b: u64 = sub_rng(1, 1, 5).next_u64();
        let c: u64 = sub_rng(1, 0, 6).next_u64();
        assert_ne!(a, b);
        assert_ne!(a, c);
        // reproducible
        assert_eq!(a, sub_rng(1, 0, 5).next_u64());
    }
}
