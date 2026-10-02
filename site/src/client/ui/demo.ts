/** Plays text as human-timed key strokes, for the demos. */

export interface Stroke {
  code: string;
  shift: boolean;
  text: string;
}

const SYMBOLS: Record<string, [code: string, shift: boolean]> = {
  " ": ["Space", false],
  ".": ["Period", false],
  ",": ["Comma", false],
  "'": ["Quote", false],
  "’": ["Quote", false],
  "-": ["Minus", false],
  "/": ["Slash", false],
  ";": ["Semicolon", false],
  "!": ["Digit1", true],
  "?": ["Slash", true],
  ":": ["Semicolon", true],
};

/** Key strokes for `text`; characters a US keyboard can't type in one stroke are skipped. */
export function strokesFor(text: string): Stroke[] {
  const out: Stroke[] = [];
  for (const ch of text) {
    if (/^[a-z]$/.test(ch)) out.push({ code: `Key${ch.toUpperCase()}`, shift: false, text: ch });
    else if (/^[A-Z]$/.test(ch)) out.push({ code: `Key${ch}`, shift: true, text: ch });
    else if (/^[0-9]$/.test(ch)) out.push({ code: `Digit${ch}`, shift: false, text: ch });
    else if (SYMBOLS[ch]) out.push({ code: SYMBOLS[ch][0], shift: SYMBOLS[ch][1], text: ch });
  }
  return out;
}

export interface TyperTarget {
  press(code: string, text: string | null): void;
  release(code: string): void;
}

/** Schedules strokes with human-ish timing; a new phrase or `cancel()` stops the old one. */
export class Typer {
  #timers: ReturnType<typeof setTimeout>[] = [];
  #held = new Set<string>();
  #listeners = new Set<(playing: boolean) => void>();
  playing = false;

  constructor(private readonly target: TyperTarget) {}

  /** Called when playback starts or stops; returns a function that unsubscribes. */
  onChange(listener: (playing: boolean) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  #set(playing: boolean): void {
    if (this.playing === playing) return;
    this.playing = playing;
    for (const l of this.#listeners) l(playing);
  }

  #at(ms: number, fn: () => void): void {
    this.#timers.push(setTimeout(fn, ms));
  }

  #down(code: string, text: string | null): void {
    this.#held.add(code);
    this.target.press(code, text);
  }

  #up(code: string): void {
    if (!this.#held.delete(code)) return;
    this.target.release(code);
  }

  type(text: string, opts: { start?: number; gap?: [number, number] } = {}): void {
    this.cancel();
    const [min, max] = opts.gap ?? [70, 150];
    let t = opts.start ?? 0;
    for (const stroke of strokesFor(text)) {
      const hold = 55 + Math.random() * 55;
      if (stroke.shift) this.#at(t - 30, () => this.#down("ShiftLeft", null));
      this.#at(t, () => this.#down(stroke.code, stroke.text));
      this.#at(t + hold, () => {
        this.#up(stroke.code);
        if (stroke.shift) this.#up("ShiftLeft");
      });
      t += min + Math.random() * (max - min) + (stroke.text === " " ? 60 : 0);
    }
    this.#at(t + 120, () => this.#set(false));
    this.#set(true);
  }

  /** Plays the same key `times` times, `every` ms apart. */
  repeat(code: string, times: number, every: number, onEach?: (i: number) => void): void {
    this.cancel();
    for (let i = 0; i < times; i++) {
      this.#at(i * every, () => {
        onEach?.(i);
        this.#down(code, null);
      });
      this.#at(i * every + 70, () => this.#up(code));
    }
    this.#at(times * every, () => this.#set(false));
    this.#set(true);
  }

  /** Plays each code once, left to right. */
  sweep(codes: string[], every: number): void {
    this.cancel();
    codes.forEach((code, i) => {
      this.#at(i * every, () => this.#down(code, null));
      this.#at(i * every + 60, () => this.#up(code));
    });
    this.#at(codes.length * every + 100, () => this.#set(false));
    this.#set(true);
  }

  cancel(): void {
    for (const timer of this.#timers) clearTimeout(timer);
    this.#timers = [];
    for (const code of [...this.#held]) this.#up(code);
    this.#set(false);
  }
}

export interface DemoSettings {
  variation?: boolean;
  spatial?: boolean;
}

/**
 * Runs a demo with engine settings forced for its duration, then puts the visitor's own
 * settings back. A demo already playing is stopped first, so its forced values are
 * restored before they could be mistaken for the visitor's.
 */
export async function runWithSettings(
  target: { variation: boolean; spatial: boolean; ready(): Promise<void> },
  typer: Typer,
  settings: DemoSettings,
  run: () => void,
): Promise<void> {
  typer.cancel();
  await target.ready();
  const saved = { variation: target.variation, spatial: target.spatial };
  Object.assign(target, settings);
  run();
  if (!typer.playing) {
    Object.assign(target, saved);
    return;
  }
  const off = typer.onChange((playing) => {
    if (playing) return;
    off();
    Object.assign(target, saved);
  });
}
