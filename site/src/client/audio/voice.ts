import { panForKeycode } from "../../shared/keys";
import { variation } from "../../shared/sound-math";

/** One clip index from a key's [first, count] candidates, or null when the key is silent. */
export function pickClip(ref: readonly [number, number] | undefined, random: () => number): number | null {
  if (!ref || ref[1] <= 0) return null;
  return ref[0] + Math.min(ref[1] - 1, Math.floor(random() * ref[1]));
}

/**
 * StereoPannerNode splits a mono voice with an equal-power law, (cos, sin) of the pan angle:
 * 1/√2 per channel at the centre. The app scales the same law by √2 so a centred key keeps
 * unity gain; voices on the page do the same.
 */
const PAN_MAKEUP = Math.SQRT2;

/** The per-channel gains a voice at `pan` ends up with on the page. */
export function channelGains(pan: number): [left: number, right: number] {
  const x = (Math.min(1, Math.max(-1, pan)) + 1) / 2;
  return [Math.cos((x * Math.PI) / 2) * PAN_MAKEUP, Math.sin((x * Math.PI) / 2) * PAN_MAKEUP];
}

/**
 * Playback rate, gain and pan for one keystroke, like the app's engine. `level` is the
 * voice's GainNode value: the variation gain plus the pan make-up.
 */
export function voiceFor(
  kvk: number,
  opts: { variation: boolean; spatial: boolean; random: () => number },
): { rate: number; gain: number; pan: number; level: number } {
  const { rate, gain } = opts.variation ? variation(opts.random) : { rate: 1, gain: 1 };
  return { rate, gain, pan: opts.spatial ? panForKeycode(kvk) : 0, level: gain * PAN_MAKEUP };
}
