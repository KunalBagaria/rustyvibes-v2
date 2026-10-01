import { expect, test } from "bun:test";
import { CODE_TO_KVK } from "../src/shared/keys";
import { decodeRvw, encodeRvw } from "../src/shared/rvw";
import { readRvpack, type RvPack } from "../scripts/rvpack";
import { FADE_MS, MAX_CLIP_MS, SHARED_CLIPS, SPECIAL_KEYS, toWebPack } from "../scripts/webpacks";

const packs = new URL("../../target/packs/", import.meta.url);
const load = async (id: string) => readRvpack(new Uint8Array(await Bun.file(new URL(`${id}.rvpack`, packs)).arrayBuffer()));
const keycodes = [...new Set(Object.values(CODE_TO_KVK))];
const SPACE = 0x31;

test("reads a real pack", async () => {
  const pack = await load("holy-panda");
  expect(pack.sampleRate).toBe(44100);
  expect(pack.hasRelease).toBe(true);
  expect(pack.meta.id).toBe("holy-panda");
  expect(pack.press).toHaveLength(128);
  for (const clip of pack.clips) expect(clip.start + clip.len).toBeLessThanOrEqual(pack.pcm.length);
});

test("small packs keep every clip", async () => {
  const pack = await load("holy-panda");
  const web = toWebPack(pack, "holy-panda", keycodes);
  expect(web.header.clips).toHaveLength(12);
  expect(Object.keys(web.header.release).length).toBeGreaterThan(0);
  const bytes = encodeRvw(web.header, web.pcm);
  expect(bytes.byteLength).toBeLessThan(100_000);
});

test("big packs share 16 clips but keep the special keys' own", async () => {
  const pack = await load("cherry-mx-brown-pbt");
  const web = toWebPack(pack, "cherry-mx-brown-pbt", keycodes);
  expect(web.header.clips.length).toBeLessThanOrEqual(SPECIAL_KEYS.length + SHARED_CLIPS);
  // Space keeps its original audio.
  const src = pack.clips[pack.press[SPACE]!.first]!;
  const [start, len] = web.header.clips[web.header.press[String(SPACE)]![0]]!;
  expect(len).toBe(Math.min(src.len, Math.round((MAX_CLIP_MS * pack.sampleRate) / 1000)));
  expect(Array.from(web.pcm.subarray(start, start + 100))).toEqual(Array.from(pack.pcm.subarray(src.start, src.start + 100)));
  // Every key that sounded in the app still sounds on the web.
  for (const k of keycodes) {
    if (pack.press[k]!.count > 0) expect(web.header.press[String(k)]).toBeDefined();
  }
  expect(encodeRvw(web.header, web.pcm).byteLength).toBeLessThan(400_000);
});

test("release maps are reduced the same way", async () => {
  const pack = await load("cherry-mx-black-abs");
  const web = toWebPack(pack, "cherry-mx-black-abs", keycodes);
  expect(Object.keys(web.header.release).length).toBeGreaterThan(0);
  expect(web.header.clips.length).toBeLessThanOrEqual(2 * (SPECIAL_KEYS.length + SHARED_CLIPS));
});

test("long clips are capped with a fade", () => {
  const rate = 1000;
  const long = new Int16Array(600).fill(10000);
  const pack: RvPack = {
    sampleRate: rate, hasRelease: false,
    clips: [{ start: 0, len: 600 }],
    press: Array.from({ length: 128 }, (_, k) => (k === SPACE ? { first: 0, count: 1 } : { first: 0, count: 0 })),
    release: Array.from({ length: 128 }, () => ({ first: 0, count: 0 })),
    meta: { id: "x" }, pcm: long,
  };
  const web = toWebPack(pack, "x", [SPACE]);
  const [, len] = web.header.clips[0]!;
  expect(len).toBe(MAX_CLIP_MS);
  expect(Math.abs(web.pcm[len - 1]!)).toBeLessThan(200);
  expect(web.pcm[len - FADE_MS - 1]).toBe(10000);
  expect(decodeRvw(encodeRvw(web.header, web.pcm).slice().buffer).header.clips).toEqual(web.header.clips);
});
