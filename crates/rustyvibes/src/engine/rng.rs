//! Tiny xorshift32 generator for per-keystroke variation (not cryptographic).

#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    /// Seeds the generator; zero is replaced because xorshift cannot leave it.
    pub fn new(seed: u32) -> Rng {
        Rng(if seed == 0 { 0x9E37_79B9 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_seed_still_produces_values() {
        assert_ne!(Rng::new(0).next_u32(), 0);
    }

    #[test]
    fn range_stays_in_bounds_and_covers_it() {
        let mut rng = Rng::new(42);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..10_000 {
            let x = rng.range(-35.0, 35.0);
            assert!((-35.0..35.0).contains(&x));
            lo = lo.min(x);
            hi = hi.max(x);
        }
        assert!(lo < -34.0 && hi > 34.0);
    }
}
