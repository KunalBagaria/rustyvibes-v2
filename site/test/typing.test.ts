import { expect, test } from "bun:test";
import { HeldKeys, shouldIntercept, typeKey } from "../src/client/input/typing";

const none = { meta: false, ctrl: false, alt: false };
const key = (k: string, extra: Partial<{ metaKey: boolean; ctrlKey: boolean; altKey: boolean }> = {}) => ({
  key: k, metaKey: false, ctrlKey: false, altKey: false, ...extra,
});

test("the typing line appends, deletes and caps its length", () => {
  expect(typeKey("hell", "o", none)).toBe("hello");
  expect(typeKey("hello", "Backspace", none)).toBe("hell");
  expect(typeKey("hi", "Shift", none)).toBe("hi");
  expect(typeKey("hi", "Enter", none)).toBe("");
  expect(typeKey("hi", "c", { ...none, meta: true })).toBe("hi");
  expect(typeKey("x".repeat(48), "y", none)).toBe("x".repeat(47) + "y");
  expect(typeKey("abcd", "e", none, 3)).toBe("cde");
});

test("intercepts printable keys and Space while the stage is in view", () => {
  const ctx = { activeIsEditableOrControl: false, stageVisible: true };
  expect(shouldIntercept(key("a"), ctx)).toBe(true);
  expect(shouldIntercept(key(" "), ctx)).toBe(true);
  expect(shouldIntercept(key("ArrowDown"), ctx)).toBe(false);
  expect(shouldIntercept(key("a", { metaKey: true }), ctx)).toBe(false);
  expect(shouldIntercept(key("a"), { ...ctx, stageVisible: false })).toBe(false);
});

test("does not intercept keys aimed at focused controls", () => {
  expect(shouldIntercept(key(" "), { activeIsEditableOrControl: true, stageVisible: true })).toBe(false);
});

test("releases every held key when command goes up", () => {
  const held = new HeldKeys();
  expect(held.down("MetaLeft")).toBe(true);
  expect(held.down("KeyC")).toBe(true);
  expect(held.down("KeyC")).toBe(false); // already down
  expect(held.releaseAll().sort()).toEqual(["KeyC", "MetaLeft"]);
  expect(held.up("KeyC")).toBe(false);
});
