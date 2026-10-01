import { expect, test } from "bun:test";
import { CODE_TO_KVK, KVK_X, panForKeycode, panForX } from "../src/shared/keys";

const keysRs = await Bun.file(new URL("../../crates/rvpack/src/keys.rs", import.meta.url)).text();
const appGeometry = new Map<number, number>();
for (const m of keysRs.matchAll(/k\(0x([0-9A-Fa-f]{2}),\s*"[^"]*",\s*\d+,\s*([\d.]+)\)/g)) {
  appGeometry.set(Number.parseInt(m[1]!, 16), Number(m[2]));
}

test("the app's geometry table was found", () => expect(appGeometry.size).toBeGreaterThan(100));

test("KVK_X matches the app's key geometry exactly", () => {
  expect(new Map(Object.entries(KVK_X).map(([k, x]) => [Number(k), x]))).toEqual(appGeometry);
});

test("every browser key code maps to a key the app knows", () => {
  for (const [code, kvk] of Object.entries(CODE_TO_KVK)) {
    expect({ code, known: appGeometry.has(kvk) }).toEqual({ code, known: true });
  }
});

test("codes map to distinct keys except the Insert/Help alias", () => {
  const seen = new Map<number, string>();
  for (const [code, kvk] of Object.entries(CODE_TO_KVK)) {
    if (seen.has(kvk)) expect([seen.get(kvk), code].sort()).toEqual(["Help", "Insert"]);
    seen.set(kvk, code);
  }
});

test("pan law matches the app", () => {
  expect(panForX(7.25)).toBe(0);
  expect(panForX(0)).toBeLessThan(0);
  expect(panForX(14)).toBeGreaterThan(0);
  expect(panForX(22)).toBeCloseTo(0.4, 6);
  expect(panForX(-50)).toBeCloseTo(-0.4, 6);
  expect(panForKeycode(CODE_TO_KVK.KeyA!)).toBeCloseTo(panForX(2.25), 6);
  expect(panForKeycode(0x7f)).toBe(0); // unknown key: centred
});
