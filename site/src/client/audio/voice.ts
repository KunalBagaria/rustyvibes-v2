import { panForKeycode } from "../../shared/keys";
import { variation } from "../../shared/sound-math";

/** One clip index from a key's [first, count] candidates, or null when the key is silent. */
export function pickClip(ref: readonly [number, number] | undefined, random: () => number): number | null {
  if (!ref || ref[1] <= 0) return null;
  return ref[0] + Math.min(ref[1] - 1, Math.floor(random() * ref[1]));
}

/** Playback rate, gain and pan for one keystroke, like the app's engine. */
export function voiceFor(
  kvk: number,
  opts: { variation: boolean; spatial: boolean; random: () => number },
): { rate: number; gain: number; pan: number } {
  const { rate, gain } = opts.variation ? variation(opts.random) : { rate: 1, gain: 1 };
  return { rate, gain, pan: opts.spatial ? panForKeycode(kvk) : 0 };
}
