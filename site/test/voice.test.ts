import { expect, test } from "bun:test";
import { pickClip, voiceFor } from "../src/client/audio/voice";
import { centsToRatio, dbToGain } from "../src/shared/sound-math";

test("picks among a key's candidates", () => {
  expect(pickClip(undefined, () => 0)).toBeNull();
  expect(pickClip([4, 1], () => 0.99)).toBe(4);
  expect(pickClip([4, 3], () => 0)).toBe(4);
  expect(pickClip([4, 3], () => 0.99)).toBe(6);
});

test("variation and spatial switch off cleanly", () => {
  const flat = voiceFor(0x00, { variation: false, spatial: false, random: () => 0 });
  expect(flat).toEqual({ rate: 1, gain: 1, pan: 0 });
  const full = voiceFor(0x00, { variation: true, spatial: true, random: () => 0 });
  expect(full.rate).toBeCloseTo(centsToRatio(-35), 9);
  expect(full.gain).toBeCloseTo(dbToGain(-1.5), 9);
  expect(full.pan).toBeLessThan(0); // A sits left of centre
});
