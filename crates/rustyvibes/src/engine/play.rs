/// A request to start a voice, sent from the input or UI thread to the audio thread.
#[derive(Clone, Copy, Debug)]
pub struct Play {
    /// Clip samples including guards: `rvpack::GUARD_BEFORE` zeros, the audio,
    /// then `rvpack::GUARD_AFTER` zeros.
    pub samples: &'static [i16],
    /// Sample rate of the clip in Hz.
    pub src_rate: u32,
    /// Playback-rate multiplier; 1.0 keeps the original pitch.
    pub pitch: f32,
    pub gain_l: f32,
    pub gain_r: f32,
    /// Delay before the voice starts, in microseconds.
    pub delay_us: u32,
}
