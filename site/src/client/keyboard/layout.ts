/** A 75% keyboard with Mac legends: 16 units wide, six rows. */

export type Legend =
  | { kind: "letter"; text: string }
  | { kind: "stack"; top: string; bottom: string }
  | { kind: "mod"; symbol?: string; word: string; align: "left" | "right" }
  | { kind: "small"; text: string }
  | { kind: "none" };

export interface KeyDef {
  /** `KeyboardEvent.code`; absent for the knob. */
  code?: string;
  /** Width in key units. */
  w: number;
  /** Empty space before the key, in key units. */
  gap?: number;
  legend: Legend;
  role: "alpha" | "mod" | "accent" | "knob";
}

export interface PlacedKey {
  def: KeyDef;
  row: number;
  /** Left edge, key units. */
  x: number;
  /** Top edge, key units; the function row sits a quarter unit above the rest. */
  y: number;
}

const letter = (code: string, text: string): KeyDef => ({ code, w: 1, legend: { kind: "letter", text }, role: "alpha" });
const stack = (code: string, top: string, bottom: string): KeyDef => ({
  code,
  w: 1,
  legend: { kind: "stack", top, bottom },
  role: "alpha",
});
const mod = (
  code: string,
  w: number,
  word: string,
  align: "left" | "right",
  symbol?: string,
  role: KeyDef["role"] = "mod",
): KeyDef => ({ code, w, legend: { kind: "mod", word, align, ...(symbol ? { symbol } : {}) }, role });
const fkey = (n: number, gap = 0): KeyDef => ({
  code: `F${n}`,
  w: 1,
  ...(gap ? { gap } : {}),
  legend: { kind: "small", text: `F${n}` },
  role: "mod",
});
const arrow = (code: string, text: string): KeyDef => ({ code, w: 1, legend: { kind: "letter", text }, role: "mod" });

export const LAYOUT: KeyDef[][] = [
  [
    mod("Escape", 1, "esc", "left", undefined, "accent"),
    fkey(1, 0.25),
    fkey(2),
    fkey(3),
    fkey(4),
    fkey(5, 0.25),
    fkey(6),
    fkey(7),
    fkey(8),
    fkey(9, 0.25),
    fkey(10),
    fkey(11),
    fkey(12),
    { ...mod("Delete", 1, "del", "right"), gap: 0.25 },
    { w: 1, legend: { kind: "none" }, role: "knob" },
  ],
  [
    stack("Backquote", "~", "`"),
    stack("Digit1", "!", "1"),
    stack("Digit2", "@", "2"),
    stack("Digit3", "#", "3"),
    stack("Digit4", "$", "4"),
    stack("Digit5", "%", "5"),
    stack("Digit6", "^", "6"),
    stack("Digit7", "&", "7"),
    stack("Digit8", "*", "8"),
    stack("Digit9", "(", "9"),
    stack("Digit0", ")", "0"),
    stack("Minus", "_", "–"),
    stack("Equal", "+", "="),
    mod("Backspace", 2, "delete", "right"),
    mod("Home", 1, "home", "right"),
  ],
  [
    mod("Tab", 1.5, "tab", "left"),
    ...["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"].map((c) => letter(`Key${c}`, c)),
    stack("BracketLeft", "{", "["),
    stack("BracketRight", "}", "]"),
    { ...stack("Backslash", "|", "\\"), w: 1.5 },
    mod("PageUp", 1, "pg up", "right"),
  ],
  [
    mod("CapsLock", 1.75, "caps lock", "left"),
    ...["A", "S", "D", "F", "G", "H", "J", "K", "L"].map((c) => letter(`Key${c}`, c)),
    stack("Semicolon", ":", ";"),
    stack("Quote", '"', "'"),
    mod("Enter", 2.25, "return", "right", undefined, "accent"),
    mod("PageDown", 1, "pg dn", "right"),
  ],
  [
    mod("ShiftLeft", 2.25, "shift", "left", "⇧"),
    ...["Z", "X", "C", "V", "B", "N", "M"].map((c) => letter(`Key${c}`, c)),
    stack("Comma", "<", ","),
    stack("Period", ">", "."),
    stack("Slash", "?", "/"),
    mod("ShiftRight", 1.75, "shift", "right", "⇧"),
    arrow("ArrowUp", "↑"),
    mod("End", 1, "end", "right"),
  ],
  [
    mod("ControlLeft", 1.25, "control", "left", "⌃"),
    mod("AltLeft", 1.25, "option", "left", "⌥"),
    mod("MetaLeft", 1.25, "command", "left", "⌘"),
    { code: "Space", w: 6.25, legend: { kind: "none" }, role: "alpha" },
    mod("MetaRight", 1, "cmd", "right", "⌘"),
    mod("AltRight", 1, "opt", "right", "⌥"),
    mod("Fn", 1, "fn", "right"),
    arrow("ArrowLeft", "←"),
    arrow("ArrowDown", "↓"),
    arrow("ArrowRight", "→"),
  ],
];

/** Every key with its position; row 0 sits a quarter unit above the rest. */
export function placedKeys(): PlacedKey[] {
  const out: PlacedKey[] = [];
  LAYOUT.forEach((row, r) => {
    let x = 0;
    for (const def of row) {
      x += def.gap ?? 0;
      out.push({ def, row: r, x, y: r === 0 ? 0 : r + 0.25 });
      x += def.w;
    }
  });
  return out;
}

/** Width and depth of the key field, key units. */
export const FIELD = { width: 16, depth: 6.25 } as const;
