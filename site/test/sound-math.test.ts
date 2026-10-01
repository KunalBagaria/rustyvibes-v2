import { expect, test } from "bun:test";
import {
  centsToRatio, dbToGain, GAIN_JITTER_DB, limiterCurve, PITCH_JITTER_CENTS, softLimit, variation, volumeToGain,
} from "../src/shared/sound-math";

test("constants match the app", () => {
  expect(PITCH_JITTER_CENTS).toBe(35);
  expect(GAIN_JITTER_DB).toBe(1.5);
});

test("volume curve is volume squared, clamped", () => {
  expect(volumeToGain(0.75)).toBeCloseTo(0.5625, 9);
  expect(volumeToGain(2)).toBe(1);
  expect(volumeToGain(-1)).toBe(0);
});

test("soft limiter: identity below the knee, bounded above", () => {
  expect(softLimit(0.5)).toBe(0.5);
  expect(softLimit(-0.8)).toBe(-0.8);
  expect(softLimit(1)).toBeCloseTo(0.9, 9);
  expect(softLimit(-3)).toBeCloseTo(-(0.8 + 0.2 * (11 / 12)), 9);
  expect(Math.abs(softLimit(1e6))).toBeLessThan(1);
});

test("limiter curve spans ±range and is monotonic", () => {
  const curve = limiterCurve(4097, 4);
  expect(curve.length).toBe(4097);
  expect(curve[2048]).toBe(0);
  expect(curve[4096]).toBeCloseTo(softLimit(4), 6);
  expect(curve[0]).toBeCloseTo(softLimit(-4), 6);
  for (let i = 1; i < curve.length; i++) expect(curve[i]! >= curve[i - 1]!).toBe(true);
});

test("variation stays within ±35 cents and ±1.5 dB", () => {
  const low = variation(() => 0);
  expect(low.rate).toBeCloseTo(centsToRatio(-35), 9);
  expect(low.gain).toBeCloseTo(dbToGain(-1.5), 9);
  const mid = variation(() => 0.5);
  expect(mid.rate).toBeCloseTo(1, 9);
  expect(mid.gain).toBeCloseTo(1, 9);
});
