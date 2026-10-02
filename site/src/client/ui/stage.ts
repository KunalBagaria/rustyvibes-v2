import { CODE_TO_KVK } from "../../shared/keys";
import type { AudioEngine } from "../audio/engine";
import { PACKS } from "../catalog.gen";
import { startTyping, typeKey } from "../input/typing";
import { COLOURWAYS as PALETTES, type ColourwayName } from "../keyboard/colorways";
import { LAYOUT } from "../keyboard/layout";
import type { KeyboardView } from "../keyboard/scene";
import { Typer } from "./demo";

export interface Stage {
  typer: Typer;
  /** Plays a key as if typed: sound, keycap and (with `text`) the typing line. */
  press(code: string, text: string | null): void;
  release(code: string): void;
  clearLine(): void;
}

const COLOURWAY_KEY = "rv:colourway";
const COLOURWAYS = Object.keys(PALETTES) as ColourwayName[];

/** What a tap on a drawn key types into the line. */
const TEXT_FOR_CODE = new Map<string, string>();
for (const key of LAYOUT.flat()) {
  if (!key.code) continue;
  if (key.code === "Space") TEXT_FOR_CODE.set(key.code, " ");
  else if (key.code === "Backspace" || key.code === "Enter") TEXT_FOR_CODE.set(key.code, key.code);
  else if (key.legend.kind === "letter" && /^[A-Z]$/.test(key.legend.text)) TEXT_FOR_CODE.set(key.code, key.legend.text.toLowerCase());
  else if (key.legend.kind === "stack") TEXT_FOR_CODE.set(key.code, key.legend.bottom === "–" ? "-" : key.legend.bottom);
}

export const packLabel = (id: string) => {
  const pack = PACKS.find((p) => p.id === id);
  return pack ? (pack.variant ? `${pack.name} ${pack.variant}` : pack.name) : id;
};

const categoryLabel = (id: string) => {
  const category = PACKS.find((p) => p.id === id)?.category ?? "";
  return category.charAt(0).toUpperCase() + category.slice(1);
};

/** Asks iOS to play page audio even with the ringer switch on silent; only after an explicit tap. */
export function preferPlayback(): void {
  const session = (navigator as Navigator & { audioSession?: { type: string } }).audioSession;
  if (session) session.type = "playback";
}

export function initStage(engine: AudioEngine): Stage {
  const root = document.querySelector<HTMLElement>("[data-stage]");
  const canvas = root?.querySelector<HTMLCanvasElement>("[data-keyboard]");
  const viewport = root?.querySelector<HTMLElement>(".stage__viewport");
  const typed = root?.querySelector<HTMLElement>("[data-typing-text]");
  const caret = root?.querySelector<HTMLElement>("[data-caret]");
  const params = new URLSearchParams(location.search);
  const posterMode = params.has("poster");
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  let view: KeyboardView | null = null;
  let line = "";
  let blink: ReturnType<typeof setTimeout> | undefined;

  const showLine = () => {
    if (typed) typed.textContent = line;
    if (!caret || reducedMotion) return;
    caret.classList.add("is-blinking");
    clearTimeout(blink);
    blink = setTimeout(() => caret.classList.remove("is-blinking"), 5000);
  };

  const maxLine = () => (window.innerWidth < 734 ? 22 : 44);

  const sound = (code: string) => {
    engine.press(CODE_TO_KVK[code]);
    view?.press(code);
  };
  const type = (text: string, mods = { meta: false, ctrl: false }) => {
    const next = typeKey(line, text, { ...mods, alt: false }, maxLine());
    if (next === line) return;
    line = next;
    showLine();
  };
  const press = (code: string, text: string | null) => {
    sound(code);
    if (text !== null) type(text);
  };
  const release = (code: string) => {
    engine.release(CODE_TO_KVK[code]);
    view?.release(code);
  };

  const typer = new Typer({ press, release });

  let stageVisible = true;
  if (viewport) {
    new IntersectionObserver(
      ([entry]) => {
        stageVisible = (entry?.intersectionRatio ?? 0) >= 0.5;
      },
      { threshold: [0, 0.5, 1] },
    ).observe(viewport);
  }

  startTyping(
    {
      press(code) {
        if (typer.playing) typer.cancel();
        engine.unlock();
        sound(code);
      },
      release: (code) => release(code),
      text: (key, mods) => type(key, mods),
    },
    () => stageVisible,
  );

  // Play demo.
  const playButton = document.querySelector<HTMLButtonElement>("[data-play-demo]");
  const playLabel = playButton?.querySelector<HTMLElement>("[data-play-label]");
  typer.onChange((playing) => {
    playButton?.classList.toggle("is-playing", playing);
    if (playLabel) playLabel.textContent = playing ? "Stop" : "Hear it";
  });
  playButton?.addEventListener("click", async () => {
    engine.unlock();
    preferPlayback();
    if (typer.playing) return typer.cancel();
    line = "";
    showLine();
    await engine.ready();
    typer.type("Hello from Rustyvibes.", { start: 80 });
  });

  // Soundpack chip.
  const select = root?.querySelector<HTMLSelectElement>("[data-pack-select]");
  const chip = root?.querySelector<HTMLElement>("[data-pack-chip]");
  const swatch = root?.querySelector<HTMLElement>("[data-pack-swatch]");
  const category = root?.querySelector<HTMLElement>("[data-pack-category]");
  select?.addEventListener("change", () => {
    engine.unlock();
    void engine.setPack(select.value);
  });
  engine.on((state) => {
    for (const el of document.querySelectorAll<HTMLElement>("[data-pack-name]")) el.textContent = packLabel(state.pack);
    if (category) category.textContent = state.error ? "Couldn’t load, try again" : categoryLabel(state.pack);
    if (select) select.value = state.pack;
    chip?.classList.toggle("is-loading", state.loading);
    const colour = PACKS.find((p) => p.id === state.pack)?.color;
    if (swatch && colour) swatch.style.setProperty("--cap", colour);
  });
  const initial = PACKS.find((p) => p.id === engine.packId)?.color;
  if (swatch && initial) swatch.style.setProperty("--cap", initial);

  // Colourways.
  let colourway: ColourwayName = "graphite";
  try {
    const saved = localStorage.getItem(COLOURWAY_KEY) as ColourwayName | null;
    if (saved && COLOURWAYS.includes(saved)) colourway = saved;
  } catch {
    // Storage blocked: start with graphite.
  }
  const fromUrl = params.get("colourway") as ColourwayName | null;
  if (fromUrl && COLOURWAYS.includes(fromUrl)) colourway = fromUrl;
  const swatches = [...(root?.querySelectorAll<HTMLButtonElement>("[data-colourway]") ?? [])];
  const pickColourway = (name: ColourwayName, focus = false) => {
    colourway = name;
    for (const button of swatches) {
      const on = button.dataset.colourway === name;
      button.setAttribute("aria-checked", String(on));
      button.tabIndex = on ? 0 : -1;
      if (on && focus) button.focus();
    }
    view?.setColourway(name);
    try {
      localStorage.setItem(COLOURWAY_KEY, name);
    } catch {
      // Storage blocked: the choice lasts until the page closes.
    }
  };
  swatches.forEach((button, i) => {
    button.addEventListener("click", () => pickColourway(button.dataset.colourway as ColourwayName));
    button.addEventListener("keydown", (e) => {
      const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
      if (!step) return;
      e.preventDefault();
      const next = swatches[(i + step + swatches.length) % swatches.length];
      pickColourway(next?.dataset.colourway as ColourwayName, true);
    });
  });
  pickColourway(colourway);

  // The keyboard itself, loaded after first paint.
  let lastScroll = -1;
  const onScroll = () => {
    if (!view || !viewport) return;
    const rect = viewport.getBoundingClientRect();
    const progress = Math.min(1, Math.max(0, -rect.top / rect.height));
    if (Math.abs(progress - lastScroll) < 0.002) return;
    lastScroll = progress;
    view.setScroll(progress);
  };
  window.addEventListener("scroll", onScroll, { passive: true });

  const load = async () => {
    if (!root || !canvas) return;
    try {
      const { createKeyboard } = await import("../keyboard/scene");
      view = await createKeyboard(canvas, {
        colourway,
        reducedMotion: reducedMotion || posterMode,
        onLost: () => root.classList.remove("is-ready"),
      });
      view.onKey((code, down) => {
        engine.unlock();
        preferPlayback();
        if (!down) return release(code);
        if (typer.playing) typer.cancel();
        press(code, TEXT_FOR_CODE.get(code) ?? null);
      });
      root.classList.add("is-ready");
      onScroll();
      (window as unknown as { __stageReady?: boolean }).__stageReady = true;
    } catch (error) {
      console.warn("The 3D keyboard is unavailable; showing the still image.", error);
    }
  };
  // The poster already shows the keyboard, so the live one starts on the first sign of
  // intent (or after a few quiet seconds) instead of competing with the page's first load.
  let started = false;
  const intents = ["pointermove", "pointerdown", "keydown", "touchstart", "wheel", "scroll"] as const;
  const start = () => {
    if (started) return;
    started = true;
    for (const type of intents) window.removeEventListener(type, start);
    void load();
  };
  if (posterMode) start();
  else {
    for (const type of intents) window.addEventListener(type, start, { passive: true });
    window.addEventListener("load", () => setTimeout(start, 6000));
  }

  return {
    typer,
    press,
    release,
    clearLine() {
      line = "";
      showLine();
    },
  };
}
