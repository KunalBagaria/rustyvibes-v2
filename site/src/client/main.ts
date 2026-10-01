// Temporary boot for Task 9: render the keyboard. Task 10 replaces this with the full page.
const stage = document.querySelector<HTMLElement>("[data-stage]");
const canvas = stage?.querySelector<HTMLCanvasElement>("[data-keyboard]");
if (stage && canvas) {
  void import("./keyboard/scene").then(async ({ createKeyboard }) => {
    const view = await createKeyboard(canvas, {
      colourway: "cream",
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
    });
    stage.classList.add("is-ready");
    (window as unknown as { __keyboard: unknown }).__keyboard = view;
  });
}
