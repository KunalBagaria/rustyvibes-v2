import { expect, test } from "bun:test";
import { runWithSettings, strokesFor, Typer } from "../src/client/ui/demo";

test("maps text to key strokes, with shift for capitals and symbols", () => {
  expect(strokesFor("Hi!")).toEqual([
    { code: "KeyH", shift: true, text: "H" },
    { code: "KeyI", shift: false, text: "i" },
    { code: "Digit1", shift: true, text: "!" },
  ]);
  expect(strokesFor("a b.").map((s) => s.code)).toEqual(["KeyA", "Space", "KeyB", "Period"]);
  expect(strokesFor("é?")).toEqual([{ code: "Slash", shift: true, text: "?" }]);
});

test("overlapping demos put the visitor's settings back", async () => {
  const target = { variation: true, spatial: false, ready: async () => {} };
  const typer = new Typer({ press: () => {}, release: () => {} });
  await runWithSettings(target, typer, { variation: false }, () => typer.repeat("KeyJ", 3, 15));
  expect(target.variation).toBe(false);
  // A second demo starts while the first still plays.
  await runWithSettings(target, typer, { spatial: true }, () => typer.sweep(["KeyA", "KeyS"], 15));
  expect(target.variation).toBe(true);
  expect(target.spatial).toBe(true);
  await new Promise((resolve) => setTimeout(resolve, 250));
  expect(target).toMatchObject({ variation: true, spatial: false });
});
