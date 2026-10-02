// Renders the keyboard poster (shown while WebGL loads, or instead of it) and the social card.
// Run after the keyboard's look changes:  bun run build && bun scripts/poster.ts
import { chromium } from "playwright";
import sharp from "sharp";

const dist = new URL("../dist/", import.meta.url).pathname;
const media = new URL("../src/media/", import.meta.url).pathname;
const pub = new URL("../public/", import.meta.url).pathname;

const server = Bun.serve({
  port: 0,
  async fetch(req) {
    const path = new URL(req.url).pathname;
    const file = Bun.file(`${dist}${path === "/" ? "index.html" : path.slice(1)}`);
    return (await file.exists()) ? new Response(file) : new Response("not found", { status: 404 });
  },
});

const browser = await chromium.launch();
// A tall viewport, so the stage keeps its natural 2.35:1 shape.
const page = await browser.newPage({ viewport: { width: 1440, height: 1400 }, deviceScaleFactor: 2 });
await page.goto(`http://localhost:${server.port}/?poster&colourway=graphite`, { waitUntil: "networkidle" });
await page.waitForFunction(() => (window as unknown as { __stageReady?: boolean }).__stageReady === true, null, {
  timeout: 20_000,
});
await page.waitForTimeout(300);
const shot = await (await page.$("[data-keyboard]"))!.screenshot({ omitBackground: true });

const poster = sharp(shot).resize({ width: 1920 });
const { width, height } = await poster.clone().png().toBuffer({ resolveWithObject: true }).then((r) => r.info);
for (const [suffix, size] of [["", 1920], ["-960", 960]] as const) {
  const sized = sharp(shot).resize({ width: size });
  await sized.clone().avif({ quality: 55, effort: 6 }).toFile(`${media}poster-graphite${suffix}.avif`);
  await sized.clone().webp({ quality: 74, alphaQuality: 90, effort: 6 }).toFile(`${media}poster-graphite${suffix}.webp`);
  await sized.clone().flatten({ background: "#000000" }).jpeg({ quality: 80, mozjpeg: true }).toFile(`${media}poster-graphite${suffix}.jpg`);
}
console.log(`poster ${width}×${height}`);

// The social card: icon, name, tagline and the keyboard.
const icon = (await Bun.file(new URL("../../assets/icon/AppIcon-1024.png", import.meta.url).pathname).bytes()).toBase64();
const keyboard = (await sharp(shot).resize({ width: 1400 }).png().toBuffer()).toBase64();
const card = await browser.newPage({ viewport: { width: 1200, height: 630 }, deviceScaleFactor: 1 });
await card.setContent(`<!doctype html><html><body style="margin:0;width:1200px;height:630px;overflow:hidden;background:#000;
  font-family:-apple-system,BlinkMacSystemFont,'Helvetica Neue',sans-serif;color:#f5f5f7;display:flex;flex-direction:column;align-items:center;position:relative">
  <div style="position:absolute;left:50%;top:62%;width:1000px;height:420px;transform:translate(-50%,-50%);
    background:radial-gradient(50% 50% at 50% 50%,rgba(232,89,43,.38),rgba(232,89,43,0) 70%);filter:blur(30px)"></div>
  <div style="margin-top:58px;display:flex;align-items:center;gap:18px;position:relative">
    <img src="data:image/png;base64,${icon}" width="76" height="76">
    <span style="font-size:70px;font-weight:700;letter-spacing:-0.03em">Rustyvibes</span>
  </div>
  <div style="margin-top:6px;font-size:44px;font-weight:700;letter-spacing:-0.03em;position:relative;
    background:linear-gradient(95deg,#f7ead2 0%,#ffc28f 38%,#ff8a4c 70%,#e8592b 100%);-webkit-background-clip:text;color:transparent">Click. Clack. Mac.</div>
  <img src="data:image/png;base64,${keyboard}" style="width:1120px;margin-top:22px;position:relative">
</body></html>`);
await card.waitForTimeout(200);
const og = await card.screenshot();
await sharp(og).jpeg({ quality: 86, mozjpeg: true }).toFile(`${pub}og.jpg`);
console.log("social card written");

await browser.close();
server.stop(true);
