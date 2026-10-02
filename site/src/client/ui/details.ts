import type { AudioEngine } from "../audio/engine";
import { PAN_WIDTH } from "../../shared/sound-math";
import { runWithSettings, type DemoSettings } from "./demo";
import type { Stage } from "./stage";
import { preferPlayback } from "./stage";

const SWEEP = ["Backquote", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus", "Equal", "Backspace"];
const BARS = [0.34, 0.62, 0.88, 0.52, 0.74, 0.44, 0.66, 0.3];

/** The variation and spatial-stereo demos, and their live visuals. */
export function initDetails(engine: AudioEngine, stage: Stage): void {
  const waves = [...document.querySelectorAll<HTMLElement>("[data-waves] span")];
  const dot = document.querySelector<HTMLElement>("[data-pan-dot]");

  // Every sound on the page moves the visuals: pitch and level reshape the bars, pan moves the dot.
  engine.onVoice((voice) => {
    if (voice.kind !== "press") return;
    waves.forEach((bar, i) => {
      const lean = (voice.rate - 1) * 14 * (i % 2 ? 1 : -1);
      const height = BARS[i]! * voice.gain ** 3 * (1 + lean);
      bar.style.setProperty("--h", `${Math.round(Math.min(1, Math.max(0.12, height)) * 100)}%`);
    });
    if (dot) dot.style.left = `${50 + (voice.pan / PAN_WIDTH) * 50}%`;
  });

  const variation = document.querySelector<HTMLElement>('[data-demo="variation"]');
  const modes = [...(variation?.querySelectorAll<HTMLButtonElement>("[data-demo-mode]") ?? [])];
  let natural = true;
  for (const mode of modes) {
    mode.addEventListener("click", () => {
      natural = mode.dataset.demoMode === "natural";
      for (const m of modes) m.setAttribute("aria-pressed", String(m === mode));
    });
  }

  const withSettings = (settings: DemoSettings, run: () => void) => {
    engine.unlock();
    preferPlayback();
    return runWithSettings(engine, stage.typer, settings, run);
  };

  variation?.querySelector<HTMLButtonElement>("[data-demo-play]")?.addEventListener("click", () => {
    void withSettings({ variation: natural }, () => stage.typer.repeat("KeyJ", 7, 160));
  });

  const spatial = document.querySelector<HTMLElement>('[data-demo="spatial"]');
  spatial?.querySelector<HTMLButtonElement>("[data-demo-play]")?.addEventListener("click", () => {
    void withSettings({ spatial: true }, () => stage.typer.sweep(SWEEP, 105));
  });
}
