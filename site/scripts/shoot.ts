// Screenshots the running site at three sizes and reports console errors:
//   bun scripts/shoot.ts [url] [--only=desktop,phone] [--full] [--sections]
import { mkdir } from "node:fs/promises";
import { chromium } from "playwright";

const url = process.argv.find((a) => a.startsWith("http")) ?? "http://localhost:8787/";
const only = process.argv.find((a) => a.startsWith("--only="))?.slice(7).split(",");
const full = process.argv.includes("--full");
const out = new URL("../.shots/", import.meta.url).pathname;
await mkdir(out, { recursive: true });

const sizes = [
  ["desktop", 1440, 900],
  ["tablet", 1024, 768],
  ["phone", 390, 844],
] as const;

const browser = await chromium.launch();
let failed = false;
for (const [name, width, height] of sizes) {
  if (only && !only.includes(name)) continue;
  const touch = name === "phone";
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 2, hasTouch: touch, isMobile: touch });
  // Count animation frames so we can prove the page idles when nothing moves.
  await page.addInitScript(() => {
    const w = window as unknown as { __frames: number };
    w.__frames = 0;
    const raf = window.requestAnimationFrame.bind(window);
    window.requestAnimationFrame = (cb) => {
      w.__frames++;
      return raf(cb);
    };
  });
  const errors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto(url, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForSelector("[data-stage].is-ready", { timeout: 15_000 }).catch(() => errors.push("keyboard never became ready"));
  await page.evaluate(() => {
    for (const el of document.querySelectorAll("[data-reveal]")) el.classList.add("is-visible");
  });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${out}${name}-top.png` });
  if (full) await page.screenshot({ path: `${out}${name}-full.png`, fullPage: true });
  if (process.argv.includes("--sections")) {
    const ids = await page.$$eval("main > section[id], footer", (els) => els.map((e) => e.id || "footer"));
    for (const id of ids) {
      const el = await page.$(id === "footer" ? "footer" : `#${id}`);
      await el?.screenshot({ path: `${out}${name}-section-${id}.png` });
    }
  }
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  await page.evaluate(() => ((window as unknown as { __frames: number }).__frames = 0));
  await page.waitForTimeout(2000);
  const idleFrames = await page.evaluate(() => (window as unknown as { __frames: number }).__frames);
  if (idleFrames > 0) errors.push(`${idleFrames} animation frames while idle`);
  console.log(`${name}: ${errors.length ? `errors: ${errors.join(" | ")}` : "no console errors"}${overflow > 0 ? `, horizontal overflow ${overflow}px` : ""}`);
  if (errors.length || overflow > 0) failed = true;
  await page.close();
}
await browser.close();
process.exit(failed ? 1 : 0);
