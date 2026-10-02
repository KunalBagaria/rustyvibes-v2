import { expect, test } from "bun:test";
import { HeldKeys, KeyRouter, shouldIntercept, typeKey } from "../src/client/input/typing";

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

function router() {
  const log: string[] = [];
  const timers: (() => void)[] = [];
  let now = 0;
  const r = new KeyRouter(
    { press: (code) => log.push(`down ${code}`), release: (code) => log.push(`up ${code}`) },
    (fn) => void timers.push(fn),
    () => now,
  );
  const ev = (code: string, extra: Partial<{ repeat: boolean; metaKey: boolean }> = {}) => ({
    code, key: code, repeat: false, isComposing: false, metaKey: false, ctrlKey: false, altKey: false, ...extra,
  });
  return { r, log, ev, flush: () => timers.splice(0).forEach((fn) => fn()), at: (t: number) => (now = t) };
}

test("Caps Lock taps instead of sticking down (macOS sends its keyup only when the lock disengages)", () => {
  const { r, log, ev, flush, at } = router();
  at(0);
  r.down(ev("CapsLock"), null);
  flush();
  expect(log).toEqual(["down CapsLock", "up CapsLock"]);
  at(3000); // the press that turns the lock off arrives as a lone keyup
  r.up(ev("CapsLock"));
  flush();
  expect(log).toEqual(["down CapsLock", "up CapsLock", "down CapsLock", "up CapsLock"]);
});

test("Caps Lock with a normal keyup (other systems) still plays once", () => {
  const { r, log, ev, flush, at } = router();
  at(0);
  r.down(ev("CapsLock"), null);
  at(120);
  r.up(ev("CapsLock"));
  flush();
  expect(log).toEqual(["down CapsLock", "up CapsLock"]);
});

test("the router releases keys pressed with command held, and ignores auto-repeat", () => {
  const { r, log, ev } = router();
  r.down(ev("MetaLeft", { metaKey: true }), null);
  r.down(ev("KeyC", { metaKey: true }), "c");
  r.down(ev("KeyC", { metaKey: true, repeat: true }), "c");
  r.up(ev("MetaLeft"));
  expect(log.sort()).toEqual(["down KeyC", "down MetaLeft", "up KeyC", "up MetaLeft"]);
});
