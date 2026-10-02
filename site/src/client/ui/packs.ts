import type { AudioEngine } from "../audio/engine";
import { prefetchPack } from "../audio/packs";
import { PACKS } from "../catalog.gen";
import type { Stage } from "./stage";
import { preferPlayback } from "./stage";

const PREVIEW = "the quick brown fox";

/** The soundpack gallery and every other pack picker on the page. */
export function initPacks(engine: AudioEngine, stage: Stage): void {
  const grid = document.querySelector<HTMLElement>("[data-pack-grid]");
  const filters = [...document.querySelectorAll<HTMLButtonElement>("[data-pack-filter]")];
  const cards = [...(grid?.querySelectorAll<HTMLElement>("[data-pack]") ?? [])];

  for (const filter of filters) {
    filter.addEventListener("click", () => {
      const category = filter.dataset.packFilter;
      for (const f of filters) f.setAttribute("aria-pressed", String(f === filter));
      for (const card of cards) card.hidden = category !== "all" && card.dataset.category !== category;
    });
  }

  for (const button of document.querySelectorAll<HTMLButtonElement>("[data-select-pack]")) {
    const id = button.dataset.selectPack ?? "";
    const inGallery = !!grid?.contains(button);
    button.addEventListener("pointerenter", () => {
      const url = PACKS.find((p) => p.id === id)?.url;
      if (url) prefetchPack(url);
    });
    button.addEventListener("click", async () => {
      engine.unlock();
      preferPlayback();
      await engine.setPack(id);
      if (inGallery && engine.packId === id && !engine.state.error) {
        stage.clearLine();
        stage.typer.type(PREVIEW, { gap: [60, 120] });
      }
    });
  }

  engine.on((state) => {
    for (const button of document.querySelectorAll<HTMLButtonElement>("[data-select-pack]")) {
      button.setAttribute("aria-pressed", String(button.dataset.selectPack === state.pack));
    }
    for (const card of cards) card.classList.toggle("is-loading", state.loading && card.dataset.pack === state.pack);
  });
}
