//! Adapters that read original soundpack layouts into a common shape.

pub mod kbsim;
pub mod mechvibes;

use rvpack::KEY_SLOTS;

/// Raw clips from a source pack, before clean-up. Each key maps to at most one
/// press clip and one release clip (indices into `clips`).
#[derive(Debug)]
pub struct RawPack {
    pub rate: u32,
    pub clips: Vec<Vec<f32>>,
    pub press: [Option<usize>; KEY_SLOTS],
    pub release: [Option<usize>; KEY_SLOTS],
}

impl RawPack {
    /// An empty pack; `rate` 0 adopts the rate of the first clip pushed.
    pub fn new(rate: u32) -> RawPack {
        RawPack { rate, clips: Vec::new(), press: [None; KEY_SLOTS], release: [None; KEY_SLOTS] }
    }

    /// Adds a clip recorded at `rate` and returns its index.
    pub fn push(&mut self, rate: u32, clip: Vec<f32>, what: &str) -> crate::Result<usize> {
        if self.rate == 0 {
            self.rate = rate;
        }
        if rate != self.rate {
            return Err(format!("{what}: sample rate {rate} Hz differs from {} Hz", self.rate));
        }
        self.clips.push(clip);
        Ok(self.clips.len() - 1)
    }
}
