// Drives the page like a visitor and checks each interaction:
//   bun scripts/interact.ts [url]
import { chromium, type Page } from "playwright";

const base = process.argv.find((a) => a.startsWith("http")) ?? "http://localhost:8787/";
const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
await context.grantPermissions(["clipboard-read", "clipboard-write"]);
const page = await context.newPage();
const errors: string[] = [];
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
});
page.on("pageerror", (e) => errors.push(String(e)));

let failures = 0;
const check = (name: string, ok: boolean, detail = "") => {
  console.log(`${ok ? "ok  " : "FAIL"}  ${name}${detail ? `  (${detail})` : ""}`);
  if (!ok) failures++;
};
const rv = <T>(page: Page, fn: string) => page.evaluate(fn) as Promise<T>;

await page.goto(`${base}?debug`, { waitUntil: "networkidle" });
await page.waitForSelector("[data-stage].is-ready", { timeout: 15_000 });
await page.evaluate(() => {
  const w = window as unknown as { __voices: number; __rv: { engine: { onVoice(f: () => void): void } } };
  w.__voices = 0;
  w.__rv.engine.onVoice(() => w.__voices++);
});

// Typing on the page: sound, line, and Space must not scroll while the stage is in view.
const scrollBefore = await page.evaluate(() => window.scrollY);
await page.keyboard.press("h");
await page.keyboard.press("i");
await page.keyboard.press("Space");
await page.waitForTimeout(600);
check("typing line shows what was typed", (await page.textContent("[data-typing-text]")) === "hi ");
check("Space did not scroll the page", (await page.evaluate(() => window.scrollY)) === scrollBefore);
await page.keyboard.press("j");
await page.waitForTimeout(300);
const voices = await rv<number>(page, "window.__voices");
check("key presses played sounds", voices >= 2, `${voices} voices`);

// Play demo.
await page.click("[data-play-demo]");
await page.waitForTimeout(3500);
check("demo typed its phrase", (await page.textContent("[data-typing-text]")) === "Hello from Rustyvibes.");

// Focused controls keep their keys: Space on the focused Play button starts the demo.
await page.focus("[data-play-demo]");
await page.keyboard.press("Space");
await page.waitForTimeout(250);
check("Space activates a focused button", (await page.textContent("[data-play-label]")) === "Stop");
await page.keyboard.press("Space");

// Soundpack card: selects the pack and previews it.
await page.click('[data-pack-grid] [data-select-pack="cherry-mx-blue-abs"]');
await page.waitForFunction(() => (window as any).__rv.engine.packId === "cherry-mx-blue-abs" && !(window as any).__rv.engine.state.loading, null, { timeout: 10_000 });
check("pack card selects its pack", true);
check("card shows as selected", (await page.getAttribute('[data-pack-grid] [data-select-pack="cherry-mx-blue-abs"]', "aria-pressed")) === "true");
check("chip shows the pack", ((await page.textContent("[data-pack-chip] [data-pack-name]")) ?? "").includes("Cherry MX Blue"));

// Filter.
await page.click('[data-pack-filter="tactile"]');
const visibleCards = await page.$$eval("[data-pack-grid] [data-pack]", (els) => els.filter((e) => !(e as HTMLElement).hidden).length);
check("tactile filter shows 7 packs", visibleCards === 7, `${visibleCards}`);
await page.click('[data-pack-filter="all"]');

// Menu replica.
await page.click("[data-menu-enabled]");
check("menu switch turns sound off", (await rv<boolean>(page, "window.__rv.engine.enabled")) === false);
check("nav speaker follows", (await page.getAttribute("[data-sound-toggle]", "aria-pressed")) === "false");
await page.click("[data-sound-toggle]");
check("nav speaker turns it back on", (await page.getAttribute("[data-menu-enabled]", "aria-checked")) === "true");
await page.click('[data-menu-toggle="variation"]');
check("menu turns variation off", (await rv<boolean>(page, "window.__rv.engine.variation")) === false);
await page.click('[data-menu-toggle="variation"]');
await page.$eval("[data-menu-volume]", (el) => {
  const input = el as HTMLInputElement;
  input.value = "0.4";
  input.dispatchEvent(new Event("input", { bubbles: true }));
});
check("menu volume sets the engine", Math.abs((await rv<number>(page, "window.__rv.engine.volume")) - 0.4) < 1e-6);
await page.click("[data-menu-submenu-toggle]");
await page.click('[data-menu-sub] [data-select-pack="topre"]');
await page.waitForFunction(() => (window as any).__rv.engine.packId === "topre", null, { timeout: 10_000 });
check("menu submenu picks a pack", true);
check("submenu closes after picking", await page.$eval("[data-menu-sub]", (el) => (el as HTMLElement).hidden === true));

// Colourways.
await page.click('[data-colourway="graphite"]');
check("colourway swatch selects", (await page.getAttribute('[data-colourway="graphite"]', "aria-checked")) === "true");

// Install sheet: opens from the hero, copies the command, closes with Escape.
await page.click(".hero [data-open-install]");
check("Install opens the sheet", await page.$eval("[data-install-sheet]", (d) => (d as HTMLDialogElement).open));
await page.click("[data-install-sheet] [data-copy]");
const copied = await page.evaluate(() => navigator.clipboard.readText());
check("copy puts the command on the clipboard", copied === "curl -fsSL https://rustyvibes.kunalbagaria.com/install | bash");
check("copy button confirms", ((await page.textContent("[data-install-sheet] [data-copy-label]")) ?? "") === "Copied");
const overflow = await page.$eval("[data-install-sheet] .command__text", (el) => el.scrollWidth - el.clientWidth);
check("the command fits the sheet on one line", overflow <= 1, `${overflow}px`);
await page.keyboard.press("Escape");
check("Escape closes the sheet", !(await page.$eval("[data-install-sheet]", (d) => (d as HTMLDialogElement).open)));

// Holding Backspace keeps deleting, like a text field.
await page.evaluate(() => window.scrollTo(0, 0));
await page.click("body", { position: { x: 5, y: 300 } });
await page.keyboard.press("Enter");
for (const ch of "abcd") await page.keyboard.press(ch);
await page.waitForTimeout(150);
await page.keyboard.down("Backspace");
for (let i = 0; i < 3; i++) await page.keyboard.down("Backspace");
await page.keyboard.up("Backspace");
await page.waitForTimeout(150);
check("holding Backspace deletes repeatedly", (await page.textContent("[data-typing-text]")) === "", JSON.stringify(await page.textContent("[data-typing-text]")));

// Phones: the Type control brings up the keyboard, and what's typed plays.
const phone = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true, deviceScaleFactor: 2 });
const mobile = await phone.newPage();
mobile.on("pageerror", (e) => errors.push(String(e)));
await mobile.goto(`${base}?debug`, { waitUntil: "networkidle" });
await mobile.evaluate(() => {
  const w = window as unknown as { __voices: number; __rv: { engine: { onVoice(f: () => void): void } } };
  w.__voices = 0;
  w.__rv.engine.onVoice(() => w.__voices++);
});
check("phones see the Type control", await mobile.isVisible("[data-stage] .pill--type"));
await mobile.tap("[data-stage] .pill--type");
check("Type focuses the typing field", await mobile.evaluate(() => document.activeElement?.matches("[data-type-input]") ?? false));
await mobile.waitForFunction(() => !(window as any).__rv.engine.state.loading, null, { timeout: 10_000 });
await mobile.keyboard.insertText("hey");
await mobile.waitForTimeout(500);
check("phone typing fills the line", (await mobile.textContent("[data-typing-text]")) === "hey", JSON.stringify(await mobile.textContent("[data-typing-text]")));
check("phone typing plays sounds", (await mobile.evaluate(() => (window as any).__voices as number)) >= 3);
await mobile.keyboard.press("Backspace");
await mobile.waitForTimeout(300);
check("phone backspace deletes once", (await mobile.textContent("[data-typing-text]")) === "he", JSON.stringify(await mobile.textContent("[data-typing-text]")));

check("no console errors", errors.length === 0, errors.join(" | "));
await browser.close();
process.exit(failures ? 1 : 0);
