// Values and curves copied from crates/rustyvibes/src/engine (mod.rs, mixer.rs).
export const PITCH_JITTER_CENTS = 35;
export const GAIN_JITTER_DB = 1.5;
export const PAN_WIDTH = 0.4;
export const DEFAULT_VOLUME = 0.75;
export const LIMITER_KNEE = 0.8;
export const MAX_VOICES = 32;

export const centsToRatio = (cents: number) => 2 ** (cents / 1200);
export const dbToGain = (db: number) => 10 ** (db / 20);

/** The app's perceptual volume curve: volume². */
export function volumeToGain(volume: number): number {
  const v = Math.min(1, Math.max(0, volume));
  return v * v;
}

/** The app's soft-knee limiter: identity below the knee, then a curve towards ±1. */
export function softLimit(x: number): number {
  const a = Math.abs(x);
  if (a <= LIMITER_KNEE) return x;
  const over = (a - LIMITER_KNEE) / (1 - LIMITER_KNEE);
  return Math.sign(x) * (LIMITER_KNEE + ((1 - LIMITER_KNEE) * over) / (1 + over));
}

/**
 * A WaveShaper curve for `softLimit` over inputs in ±`range`; feed the shaper the
 * signal scaled by 1/`range`.
 */
export function limiterCurve(points = 4097, range = 4): Float32Array {
  const curve = new Float32Array(points);
  for (let i = 0; i < points; i++) curve[i] = softLimit(((i / (points - 1)) * 2 - 1) * range);
  return curve;
}

/** One keystroke's natural variation; `random` returns values in [0, 1). */
export function variation(random: () => number): { rate: number; gain: number } {
  const spread = (width: number) => (random() * 2 - 1) * width;
  return { rate: centsToRatio(spread(PITCH_JITTER_CENTS)), gain: dbToGain(spread(GAIN_JITTER_DB)) };
}
