import { AudioEngine } from "./audio/engine";
import { initCopy } from "./ui/copy";
import { initDetails } from "./ui/details";
import { initMenu } from "./ui/menu";
import { initPacks } from "./ui/packs";
import { initReveal } from "./ui/reveal";
import { initInstallSheet } from "./ui/sheet";
import { initSoundSwitch } from "./ui/sound";
import { initStage } from "./ui/stage";
import { initTouchTyping } from "./ui/touch-typing";

const engine = new AudioEngine();

initReveal();
initCopy();
const sound = initSoundSwitch(engine);
const stage = initStage(engine);
initPacks(engine, stage);
initMenu(engine, sound);
initDetails(engine, stage);
initInstallSheet();
initTouchTyping(stage, engine);

// Browsers start audio only after a gesture: the first click or key press anywhere.
const unlock = () => engine.unlock();
document.addEventListener("pointerdown", unlock, { capture: true, passive: true });
document.addEventListener("keydown", unlock, { capture: true });

// Fetch the default soundpack once the page has settled, so the first key press sounds.
window.addEventListener("load", () => {
  const preload = () => engine.preload();
  if ("requestIdleCallback" in window) requestIdleCallback(preload, { timeout: 3000 });
  else setTimeout(preload, 1200);
});

if (new URLSearchParams(location.search).has("debug")) {
  (window as unknown as { __rv: unknown }).__rv = { engine, stage };
}
