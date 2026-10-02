import { CODE_TO_KVK } from "../../shared/keys";

const MAX_LINE = 48;
/** How long a Caps Lock press shows: macOS reports its keyup only when the lock turns off. */
const LOCK_TAP_MS = 110;
/** A Caps Lock keyup this soon after its keydown belongs to the same press. */
const LOCK_SAME_PRESS_MS = 400;

export interface KeyLike {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
}

export interface KeyEventLike extends KeyLike {
  code: string;
  repeat: boolean;
  isComposing: boolean;
}

/** The demo's typing line after one key. */
export function typeKey(
  line: string,
  key: string,
  mods: { meta: boolean; ctrl: boolean; alt: boolean },
  max = MAX_LINE,
): string {
  if (mods.meta || mods.ctrl) return line;
  if (key === "Backspace") return [...line].slice(0, -1).join("");
  if (key === "Enter") return "";
  if ([...key].length !== 1) return line;
  return [...(line + key)].slice(-max).join("");
}

/** Whether the page should swallow a key so typing doesn't scroll or open find. */
export function shouldIntercept(e: KeyLike, ctx: { activeIsEditableOrControl: boolean; stageVisible: boolean }): boolean {
  if (ctx.activeIsEditableOrControl || !ctx.stageVisible) return false;
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  return [...e.key].length === 1;
}

/** Tracks held keys; macOS browsers drop keyups for keys pressed while ⌘ is held. */
export class HeldKeys {
  #down = new Set<string>();

  down(code: string): boolean {
    if (this.#down.has(code)) return false;
    this.#down.add(code);
    return true;
  }

  up(code: string): boolean {
    return this.#down.delete(code);
  }

  releaseAll(): string[] {
    const all = [...this.#down];
    this.#down.clear();
    return all;
  }
}

export interface TypingHandlers {
  /**
   * A key went down. `text` is the key's text for the typing line, or null when the key
   * belongs to a focused control.
   */
  press(code: string, kvk: number | undefined, text: string | null, mods: { meta: boolean; ctrl: boolean }): void;
  release(code: string, kvk: number | undefined): void;
}

/**
 * Turns key events into presses and releases: ignores auto-repeat, releases keys whose
 * keyups ⌘ swallowed, and plays Caps Lock as a tap (macOS sends its keydown when the lock
 * turns on and its keyup only when it turns off).
 */
export class KeyRouter {
  #held = new HeldKeys();
  #capsDownAt = Number.NEGATIVE_INFINITY;

  constructor(
    private readonly handlers: TypingHandlers,
    private readonly schedule: (fn: () => void, ms: number) => void = (fn, ms) => void setTimeout(fn, ms),
    private readonly now: () => number = () => performance.now(),
  ) {}

  down(e: KeyEventLike, text: string | null): void {
    if (e.repeat || e.isComposing) return;
    if (!this.#held.down(e.code)) return;
    this.handlers.press(e.code, CODE_TO_KVK[e.code], text, { meta: e.metaKey, ctrl: e.ctrlKey });
    if (e.code === "CapsLock") {
      this.#capsDownAt = this.now();
      this.schedule(() => this.#lift("CapsLock"), LOCK_TAP_MS);
    }
  }

  up(e: KeyEventLike): void {
    if (e.code === "MetaLeft" || e.code === "MetaRight") return this.releaseAll();
    if (e.code === "CapsLock") {
      if (this.now() - this.#capsDownAt < LOCK_SAME_PRESS_MS) return;
      // The press that turns the lock off arrives as a lone keyup: play it as a tap.
      if (this.#held.down("CapsLock")) {
        this.handlers.press("CapsLock", CODE_TO_KVK.CapsLock, null, { meta: false, ctrl: false });
        this.schedule(() => this.#lift("CapsLock"), LOCK_TAP_MS);
      }
      return;
    }
    this.#lift(e.code);
  }

  releaseAll(): void {
    for (const code of this.#held.releaseAll()) this.handlers.release(code, CODE_TO_KVK[code]);
  }

  #lift(code: string): void {
    if (this.#held.up(code)) this.handlers.release(code, CODE_TO_KVK[code]);
  }
}

const CONTROLS =
  "input, textarea, select, button, summary, a[href], [contenteditable], [role=switch], [role=radio], [tabindex]:not([tabindex='-1'])";

const isControl = (el: Element | null) => !!el && el !== document.body && el.matches(CONTROLS);

/** Wires document keyboard events to the demo; returns a function that unwires them. */
export function startTyping(handlers: TypingHandlers, stageVisible: () => boolean): () => void {
  const router = new KeyRouter(handlers);
  const onDown = (e: KeyboardEvent) => {
    if (e.isComposing) return;
    const focused = isControl(document.activeElement);
    // Swallow repeats too, so holding Space doesn't start scrolling.
    if (shouldIntercept(e, { activeIsEditableOrControl: focused, stageVisible: stageVisible() })) e.preventDefault();
    router.down(e, focused ? null : e.key);
  };
  const onUp = (e: KeyboardEvent) => router.up(e);
  const releaseAll = () => router.releaseAll();
  const onHide = () => {
    if (document.visibilityState === "hidden") releaseAll();
  };
  document.addEventListener("keydown", onDown);
  document.addEventListener("keyup", onUp);
  window.addEventListener("blur", releaseAll);
  document.addEventListener("visibilitychange", onHide);
  return () => {
    document.removeEventListener("keydown", onDown);
    document.removeEventListener("keyup", onUp);
    window.removeEventListener("blur", releaseAll);
    document.removeEventListener("visibilitychange", onHide);
  };
}
