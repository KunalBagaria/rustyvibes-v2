import type { AudioEngine } from "../audio/engine";
import type { SoundSwitch } from "./sound";

/** The menu replica: every control drives the page's sound engine, like the real menu. */
export function initMenu(engine: AudioEngine, sound: SoundSwitch): void {
  const menu = document.querySelector<HTMLElement>("[data-menu]");
  if (!menu) return;

  menu.querySelector<HTMLButtonElement>("[data-menu-enabled]")?.addEventListener("click", () => {
    engine.unlock();
    sound.set(!sound.enabled);
  });

  const volume = menu.querySelector<HTMLInputElement>("[data-menu-volume]");
  const paintVolume = () => volume?.style.setProperty("--fill", `${Number(volume.value) * 100}%`);
  volume?.addEventListener("input", () => {
    engine.volume = Number(volume.value);
    paintVolume();
  });
  // Let people hear the new level, as the app does.
  volume?.addEventListener("change", () => {
    engine.unlock();
    engine.press(0x31);
    setTimeout(() => engine.release(0x31), 90);
  });
  paintVolume();

  const toggles: Record<string, (on: boolean) => void> = {
    release: (on) => (engine.releaseSounds = on),
    variation: (on) => (engine.variation = on),
    spatial: (on) => (engine.spatial = on),
    login: () => {},
  };
  for (const button of menu.querySelectorAll<HTMLButtonElement>("[data-menu-toggle]")) {
    button.addEventListener("click", () => {
      const on = button.getAttribute("aria-pressed") !== "true";
      button.setAttribute("aria-pressed", String(on));
      toggles[button.dataset.menuToggle ?? ""]?.(on);
    });
  }

  // The Soundpack submenu: click or hover to open, Escape or a click elsewhere to close.
  const submenu = menu.querySelector<HTMLElement>("[data-menu-submenu]");
  const opener = menu.querySelector<HTMLButtonElement>("[data-menu-submenu-toggle]");
  const list = menu.querySelector<HTMLElement>("[data-menu-sub]");
  const setOpen = (open: boolean) => {
    if (!list || !opener) return;
    list.hidden = !open;
    opener.setAttribute("aria-expanded", String(open));
  };
  const hoverMode = window.matchMedia("(hover: hover) and (min-width: 901px)").matches;
  // With a mouse, hovering already opened it, so a click keeps it open (as on macOS);
  // keyboard activation (detail 0) and touch toggle it.
  opener?.addEventListener("click", (e) => setOpen(hoverMode && e.detail > 0 ? true : !!list?.hidden));
  if (hoverMode) {
    let closing: ReturnType<typeof setTimeout> | undefined;
    submenu?.addEventListener("pointerenter", () => {
      clearTimeout(closing);
      setOpen(true);
    });
    submenu?.addEventListener("pointerleave", () => {
      closing = setTimeout(() => setOpen(false), 250);
    });
  }
  list?.addEventListener("click", (e) => {
    if ((e.target as Element).closest("[data-select-pack]")) setOpen(false);
  });
  document.addEventListener("click", (e) => {
    if (submenu && !submenu.contains(e.target as Node)) setOpen(false);
  });
  menu.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && list && !list.hidden) {
      setOpen(false);
      opener?.focus();
    }
  });
}
