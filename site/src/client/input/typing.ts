import { CODE_TO_KVK } from "../../shared/keys";

const MAX_LINE = 48;

export interface KeyLike {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
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

const CONTROLS =
  "input, textarea, select, button, summary, a[href], [contenteditable], [role=switch], [role=radio], [tabindex]:not([tabindex='-1'])";

const isControl = (el: Element | null) => !!el && el !== document.body && el.matches(CONTROLS);

export interface TypingHandlers {
  /**
   * A key went down. `text` is the key's text for the typing line, or null when the key
   * belongs to a focused control.
   */
  press(code: string, kvk: number | undefined, text: string | null, mods: { meta: boolean; ctrl: boolean }): void;
  release(code: string, kvk: number | undefined): void;
}

/** Wires document keyboard events to the demo; returns a function that unwires them. */
export function startTyping(handlers: TypingHandlers, stageVisible: () => boolean): () => void {
  const held = new HeldKeys();
  const releaseAll = () => {
    for (const code of held.releaseAll()) handlers.release(code, CODE_TO_KVK[code]);
  };
  const onDown = (e: KeyboardEvent) => {
    if (e.repeat || e.isComposing) return;
    const focused = isControl(document.activeElement);
    if (shouldIntercept(e, { activeIsEditableOrControl: focused, stageVisible: stageVisible() })) e.preventDefault();
    if (!held.down(e.code)) return;
    handlers.press(e.code, CODE_TO_KVK[e.code], focused ? null : e.key, { meta: e.metaKey, ctrl: e.ctrlKey });
  };
  const onUp = (e: KeyboardEvent) => {
    if (e.code === "MetaLeft" || e.code === "MetaRight") return releaseAll();
    if (held.up(e.code)) handlers.release(e.code, CODE_TO_KVK[e.code]);
  };
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
