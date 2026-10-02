import { expect, test } from "bun:test";
import { diffInput, strokeFor } from "../src/client/ui/touch-typing";

test("diffs what a phone keyboard changed", () => {
  expect(diffInput("abc", "abcd")).toEqual({ deleted: 0, inserted: "d" });
  expect(diffInput("abc", "ab")).toEqual({ deleted: 1, inserted: "" });
  expect(diffInput("abc", "abX")).toEqual({ deleted: 1, inserted: "X" });
  expect(diffInput("ab", "ab")).toEqual({ deleted: 0, inserted: "" });
  // Autocomplete replacing a word, and characters outside the BMP.
  expect(diffInput("  teh", "  the ")).toEqual({ deleted: 2, inserted: "he " });
  expect(diffInput("a😀", "a")).toEqual({ deleted: 1, inserted: "" });
});

test("every character a phone can type maps to a key", () => {
  expect(strokeFor("a")).toEqual({ code: "KeyA", shift: false });
  expect(strokeFor("A")).toEqual({ code: "KeyA", shift: true });
  expect(strokeFor(" ")).toEqual({ code: "Space", shift: false });
  expect(strokeFor("\n")).toEqual({ code: "Enter", shift: false });
  expect(strokeFor("5")).toEqual({ code: "Digit5", shift: false });
  const accent = strokeFor("é");
  expect(accent.code).toMatch(/^Key[A-Z]$/);
  expect(strokeFor("é")).toEqual(accent);
  expect(strokeFor("😀").code).toMatch(/^Key[A-Z]$/);
});
