/**
 * The install sheet. Install buttons are links to #install, so without JavaScript (or
 * <dialog>) they still lead to the instructions.
 */
export function initInstallSheet(): void {
  const sheet = document.querySelector<HTMLDialogElement>("[data-install-sheet]");
  if (!sheet || typeof sheet.showModal !== "function") return;
  for (const opener of document.querySelectorAll<HTMLAnchorElement>("[data-open-install]")) {
    opener.addEventListener("click", (e) => {
      e.preventDefault();
      sheet.showModal();
      sheet.querySelector<HTMLButtonElement>("[data-copy]")?.focus();
    });
  }
  for (const closer of sheet.querySelectorAll("[data-close-sheet]")) {
    closer.addEventListener("click", () => sheet.close());
  }
  // A click on the dimmed backdrop lands on the dialog itself.
  sheet.addEventListener("click", (e) => {
    if (e.target === sheet) sheet.close();
  });
}
