/** Copy buttons for the install command, with a polite announcement for screen readers. */

export function announce(message: string): void {
  const region = document.querySelector<HTMLElement>("[data-announce]");
  if (!region) return;
  region.textContent = "";
  requestAnimationFrame(() => (region.textContent = message));
}

function selectText(el: Element | null | undefined): void {
  if (!el) return;
  const range = document.createRange();
  range.selectNodeContents(el);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
}

export function initCopy(): void {
  for (const button of document.querySelectorAll<HTMLButtonElement>("[data-copy]")) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    button.addEventListener("click", async () => {
      const text = button.dataset.copyText ?? "";
      const label = button.querySelector<HTMLElement>("[data-copy-label]");
      try {
        await navigator.clipboard.writeText(text);
      } catch {
        // No clipboard access (old browser or permissions): select the text instead.
        selectText(button.parentElement?.querySelector("code"));
        announce("Command selected. Press Command-C to copy it.");
        return;
      }
      button.classList.add("is-copied");
      if (label) label.textContent = "Copied";
      announce("Install command copied. Paste it into Terminal.");
      clearTimeout(timer);
      timer = setTimeout(() => {
        button.classList.remove("is-copied");
        if (label) label.textContent = "Copy";
      }, 2000);
    });
  }
}
