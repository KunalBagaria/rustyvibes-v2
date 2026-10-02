import { expect, test } from "bun:test";
import { strokesFor } from "../src/client/ui/demo";

test("maps text to key strokes, with shift for capitals and symbols", () => {
  expect(strokesFor("Hi!")).toEqual([
    { code: "KeyH", shift: true, text: "H" },
    { code: "KeyI", shift: false, text: "i" },
    { code: "Digit1", shift: true, text: "!" },
  ]);
  expect(strokesFor("a b.").map((s) => s.code)).toEqual(["KeyA", "Space", "KeyB", "Period"]);
  expect(strokesFor("é?")).toEqual([{ code: "Slash", shift: true, text: "?" }]);
});
