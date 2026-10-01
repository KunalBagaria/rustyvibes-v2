import { expect, test } from "bun:test";
import { BLURBS, catalogModule, escapeHtml, renderMenuPacks, renderPackCards, renderPartials, WEB_DEFAULT_PACK } from "../scripts/partials";
import type { WebPackInfo } from "../scripts/webpacks";

const pack = (id: string, category: string, variant = ""): WebPackInfo => ({
  id, name: id.toUpperCase(), variant, category, color: "#123456", credit: "c", url: `/packs/${id}.rvw`, bytes: 1, hasRelease: false,
});
const packs = [pack("holy-panda", "tactile"), pack("cherry-mx-blue-abs", "clicky", "ABS"), pack("alpaca", "linear")];

test("escapes HTML", () => expect(escapeHtml(`<a href="x">&'`)).toBe("&lt;a href=&quot;x&quot;&gt;&amp;&#39;"));

test("pack cards carry data, colour and selection", () => {
  const html = renderPackCards(packs, "holy-panda");
  expect(html.match(/data-pack="/g)).toHaveLength(3);
  expect(html).toContain('data-category="clicky"');
  expect(html).toContain("--cap: #123456");
  expect(html).toMatch(/data-pack="holy-panda"[^>]*>\s*<button[^>]*aria-pressed="true"/);
  expect(html).toContain(">ABS<");
});

test("menu groups packs Linear, Tactile, Clicky in that order", () => {
  const html = renderMenuPacks(packs, "holy-panda");
  expect(html.indexOf("Linear")).toBeLessThan(html.indexOf("Tactile"));
  expect(html.indexOf("Tactile")).toBeLessThan(html.indexOf("Clicky"));
});

test("menu items are plain toggle buttons, not orphaned ARIA menu items", () => {
  const html = renderMenuPacks(packs, "holy-panda");
  expect(html).not.toContain("role=\"menuitemradio\"");
  expect(html).toMatch(/aria-pressed="true" data-select-pack="holy-panda"/);
  expect(html).toMatch(/aria-pressed="false" data-select-pack="alpaca"/);
});

test("every catalog pack has a blurb", async () => {
  const catalog = await Bun.file(new URL("../../assets/soundpacks/catalog.json", import.meta.url)).json();
  for (const p of catalog.packs) expect(BLURBS[p.id]).toBeTruthy();
  expect(catalog.packs.some((p: { id: string }) => p.id === WEB_DEFAULT_PACK)).toBe(true);
});

test("partials cover every marker the page uses", async () => {
  const page = await Bun.file(new URL("../src/index.html", import.meta.url)).text();
  const markers = [...page.matchAll(/<!--@([a-z-]+)-->/g)].map((m) => m[1]);
  expect([...new Set(markers)].sort()).toEqual(Object.keys(renderPartials(packs, "holy-panda")).sort());
});

test("catalog module is valid TypeScript data", () => {
  const source = catalogModule(packs, "holy-panda");
  expect(source).toContain('export const DEFAULT_PACK = "holy-panda";');
  expect(source).toContain('"url":"/packs/alpaca.rvw"');
});
