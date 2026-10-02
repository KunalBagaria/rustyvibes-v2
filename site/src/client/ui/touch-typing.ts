import type { AudioEngine } from "../audio/engine";
import { strokesFor } from "./demo";
import type { Stage } from "./stage";
import { preferPlayback } from "./stage";

/** What changed between two values of the typing field: characters removed from the end, then added. */
export function diffInput(prev: string, next: string): { deleted: number; inserted: string } {
  const a = [...prev];
  const b = [...next];
  let same = 0;
  while (same < a.length && same < b.length && a[same] === b[same]) same++;
  return { deleted: a.length - same, inserted: b.slice(same).join("") };
}

const ROW = "ASDFGHJKL";

/** The key a typed character came from; characters with no key of their own get a stable letter key. */
export function strokeFor(ch: string): { code: string; shift: boolean } {
  if (ch === "\n") return { code: "Enter", shift: false };
  const stroke = strokesFor(ch)[0];
  if (stroke) return { code: stroke.code, shift: stroke.shift };
  return { code: `Key${ROW[(ch.codePointAt(0) ?? 0) % ROW.length]}`, shift: false };
}

/** Kept in the field so Backspace always has something to delete, even "empty". */
const SENTINEL = "        ";
const TAP_MS = 70;

/**
 * Typing on phones: a hidden field brings up the on-screen keyboard and every change to it
 * plays as key presses on the stage. Hardware keys already played through the page's
 * keydown handler are not played twice.
 */
export function initTouchTyping(stage: Stage, engine: AudioEngine): void {
  const input = document.querySelector<HTMLTextAreaElement>("[data-type-input]");
  const viewport = document.querySelector<HTMLElement>("[data-stage] .stage__viewport");
  if (!input) return;
  let prev = SENTINEL;
  let composing = false;
  let hardwareAt = Number.NEGATIVE_INFINITY;

  const reset = () => {
    input.value = SENTINEL;
    prev = SENTINEL;
    input.setSelectionRange(SENTINEL.length, SENTINEL.length);
  };
  const tap = (code: string, text: string, at: number) => {
    setTimeout(() => {
      stage.press(code, text);
      setTimeout(() => stage.release(code), TAP_MS);
    }, at);
  };

  input.addEventListener("keydown", (e) => {
    if (e.code && e.key !== "Unidentified") hardwareAt = performance.now();
  });
  input.addEventListener("compositionstart", () => (composing = true));
  input.addEventListener("compositionend", () => (composing = false));
  input.addEventListener("input", () => {
    const next = input.value;
    const { deleted, inserted } = diffInput(prev, next);
    prev = next;
    if (performance.now() - hardwareAt > 80) {
      let at = 0;
      for (let i = 0; i < deleted; i++, at += 45) tap("Backspace", "Backspace", at);
      for (const ch of inserted) {
        tap(strokeFor(ch).code, ch === "\n" ? "Enter" : ch, at);
        at += 45;
      }
    }
    if (!composing && (next.length < SENTINEL.length || next.length > SENTINEL.length + 40 || next.includes("\n"))) {
      reset();
    }
  });

  const open = () => {
    engine.unlock();
    preferPlayback();
    reset();
    input.focus({ preventScroll: true });
    // Keep the keyboard on screen above the phone's own keyboard.
    setTimeout(() => viewport?.scrollIntoView({ block: "center", behavior: "smooth" }), 350);
  };
  for (const trigger of document.querySelectorAll<HTMLElement>("[data-type-trigger]")) {
    trigger.addEventListener("click", open);
  }
  input.addEventListener("focus", () => document.documentElement.classList.add("is-typing"));
  input.addEventListener("blur", () => document.documentElement.classList.remove("is-typing"));
}
