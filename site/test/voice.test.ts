import { expect, test } from "bun:test";
import { channelGains, pickClip, voiceFor } from "../src/client/audio/voice";
import { centsToRatio, dbToGain } from "../src/shared/sound-math";

test("picks among a key's candidates", () => {
  expect(pickClip(undefined, () => 0)).toBeNull();
  expect(pickClip([4, 1], () => 0.99)).toBe(4);
  expect(pickClip([4, 3], () => 0)).toBe(4);
  expect(pickClip([4, 3], () => 0.99)).toBe(6);
});

test("variation and spatial switch off cleanly", () => {
  const flat = voiceFor(0x00, { variation: false, spatial: false, random: () => 0 });
  expect(flat).toEqual({ rate: 1, gain: 1, pan: 0, level: Math.SQRT2 });
  const full = voiceFor(0x00, { variation: true, spatial: true, random: () => 0 });
  expect(full.rate).toBeCloseTo(centsToRatio(-35), 9);
  expect(full.gain).toBeCloseTo(dbToGain(-1.5), 9);
  expect(full.pan).toBeLessThan(0); // A sits left of centre
});

test("output level matches the app's pan law: unity at the centre", () => {
  // The app: angle = (pan + 1)·π/4, gains = (cos, sin)·√2 (crates/rustyvibes/src/engine/mod.rs).
  for (const pan of [0, 0.4, -0.4, 0.17]) {
    const angle = ((pan + 1) * Math.PI) / 4;
    const [l, r] = channelGains(pan);
    expect(l).toBeCloseTo(Math.cos(angle) * Math.SQRT2, 9);
    expect(r).toBeCloseTo(Math.sin(angle) * Math.SQRT2, 9);
  }
  expect(channelGains(0)[0]).toBeCloseTo(1, 9);
  const flat = voiceFor(0x31, { variation: false, spatial: false, random: () => 0.5 });
  // StereoPannerNode gives 1/√2 per channel at the centre; the voice level makes up for it.
  expect(flat.level * Math.SQRT1_2).toBeCloseTo(1, 9);
});
