import { expect, test } from "bun:test";
import { LAYOUT, placedKeys } from "../src/client/keyboard/layout";
import { CODE_TO_KVK } from "../src/shared/keys";

test("every row is 16 units wide", () => {
  for (const row of LAYOUT) expect(row.reduce((w, k) => w + k.w + (k.gap ?? 0), 0)).toBeCloseTo(16, 9);
});

test("codes are unique and known to the sound engine", () => {
  const codes = LAYOUT.flat().flatMap((k) => (k.code ? [k.code] : []));
  expect(new Set(codes).size).toBe(codes.length);
  for (const code of codes) expect(CODE_TO_KVK[code]).toBeDefined();
  expect(codes).toHaveLength(82);
});

test("arrow up sits above arrow down", () => {
  const at = (code: string) => placedKeys().find((k) => k.def.code === code)!;
  expect(at("ArrowUp").x).toBe(at("ArrowDown").x);
  expect(at("ArrowUp").row).toBe(at("ArrowDown").row - 1);
});
