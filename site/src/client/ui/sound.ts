import type { AudioEngine } from "../audio/engine";

/** The page's on/off state, shown by the nav speaker and the menu replica's switch. */
export interface SoundSwitch {
  readonly enabled: boolean;
  set(enabled: boolean): void;
}

const KEY = "rv:sound";

export function initSoundSwitch(engine: AudioEngine): SoundSwitch {
  const navButton = document.querySelector<HTMLButtonElement>("[data-sound-toggle]");
  const menuSwitch = document.querySelector<HTMLButtonElement>("[data-menu-enabled]");
  let enabled = true;
  try {
    enabled = localStorage.getItem(KEY) !== "off";
  } catch {
    // Storage blocked: keep the default.
  }

  const render = () => {
    engine.enabled = enabled;
    navButton?.setAttribute("aria-pressed", String(enabled));
    navButton?.setAttribute("aria-label", enabled ? "Sound on" : "Sound off");
    menuSwitch?.setAttribute("aria-checked", String(enabled));
  };

  const api: SoundSwitch = {
    get enabled() {
      return enabled;
    },
    set(value) {
      enabled = value;
      render();
      try {
        localStorage.setItem(KEY, value ? "on" : "off");
      } catch {
        // Storage blocked: the choice lasts until the page closes.
      }
    },
  };

  navButton?.addEventListener("click", () => api.set(!enabled));
  render();
  return api;
}
