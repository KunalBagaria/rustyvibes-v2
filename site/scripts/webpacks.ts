import { createHash } from "node:crypto";
import { mkdir } from "node:fs/promises";
import { CODE_TO_KVK } from "../src/shared/keys";
import { encodeRvw, type RvwHeader } from "../src/shared/rvw";
import { readRvpack, type RvPack } from "./rvpack";

/** Space, return, delete, tab, caps lock, both shifts, escape: their sounds stay their own. */
export const SPECIAL_KEYS = [0x31, 0x24, 0x33, 0x30, 0x39, 0x38, 0x3c, 0x35] as const;
export const SHARED_CLIPS = 16;
export const SUBSET_THRESHOLD = 32;
export const MAX_CLIP_MS = 250;
export const FADE_MS = 30;

type Ref = { first: number; count: number };
const isSpecial = (k: number) => (SPECIAL_KEYS as readonly number[]).includes(k);

/** Reduces one key map; returns keycode → source clip indices. */
function reduceMap(map: Ref[], keycodes: readonly number[], subset: boolean): Map<number, number[]> {
  const out = new Map<number, number[]>();
  const candidates = (k: number) => {
    const ref = map[k];
    return ref && ref.count > 0 ? Array.from({ length: ref.count }, (_, i) => ref.first + i) : [];
  };
  const shared: number[] = [];
  for (const k of keycodes) {
    const clips = candidates(k);
    if (clips.length === 0) continue;
    if (!subset || isSpecial(k)) out.set(k, clips);
    else if (!shared.includes(clips[0]!)) shared.push(clips[0]!);
  }
  if (subset) {
    // Deal the ordinary keys' distinct clips round-robin into SHARED_CLIPS slots, each
    // played by the slot's first clip: keys that sit next to each other (and come next
    // to each other in keycode order) land in different slots and keep distinct sounds.
    for (const k of keycodes) {
      if (isSpecial(k)) continue;
      const clips = candidates(k);
      if (clips.length > 0) out.set(k, [shared[shared.indexOf(clips[0]!) % SHARED_CLIPS]!]);
    }
  }
  return out;
}

export function toWebPack(pack: RvPack, id: string, keycodes: readonly number[]): { header: RvwHeader; pcm: Int16Array } {
  const used = new Set<number>();
  for (const map of [pack.press, pack.release]) {
    for (const k of keycodes) {
      const ref = map[k];
      if (ref) for (let i = 0; i < ref.count; i++) used.add(ref.first + i);
    }
  }
  const subset = used.size > SUBSET_THRESHOLD;
  const maps = { press: reduceMap(pack.press, keycodes, subset), release: reduceMap(pack.release, keycodes, subset) };

  const cap = Math.round((MAX_CLIP_MS * pack.sampleRate) / 1000);
  const fade = Math.round((FADE_MS * pack.sampleRate) / 1000);
  // Place kept clips in source order: a key's candidates are consecutive source
  // clips, so they stay consecutive here and [first, count] still describes them.
  const kept = new Set<number>();
  for (const map of [maps.press, maps.release]) for (const sources of map.values()) for (const s of sources) kept.add(s);
  const index = new Map<number, number>();
  const clips: [number, number][] = [];
  const chunks: Int16Array[] = [];
  let cursor = 0;
  for (const source of [...kept].sort((a, b) => a - b)) {
    const { start, len } = pack.clips[source]!;
    const audio = pack.pcm.slice(start, start + Math.min(len, cap));
    if (len > cap) {
      for (let i = 0; i < fade; i++) {
        const at = audio.length - fade + i;
        audio[at] = Math.round(audio[at]! * 0.5 * (1 + Math.cos((Math.PI * (i + 1)) / fade)));
      }
    }
    index.set(source, clips.length);
    clips.push([cursor, audio.length]);
    chunks.push(audio);
    cursor += audio.length;
  }
  const toRecord = (map: Map<number, number[]>) => {
    const record: Record<string, [number, number]> = {};
    for (const [k, sources] of map) record[String(k)] = [index.get(sources[0]!)!, sources.length];
    return record;
  };
  const press = toRecord(maps.press);
  const release = toRecord(maps.release);
  const pcm = new Int16Array(cursor);
  let at = 0;
  for (const chunk of chunks) {
    pcm.set(chunk, at);
    at += chunk.length;
  }
  return { header: { id, sampleRate: pack.sampleRate, clips, press, release }, pcm };
}

export interface WebPackInfo {
  id: string;
  name: string;
  variant: string;
  category: string;
  color: string;
  credit: string;
  url: string;
  bytes: number;
  hasRelease: boolean;
}

interface CatalogEntry {
  id: string;
  name: string;
  variant?: string;
  category: string;
  color: string;
  credit: string;
}

/** Exports every catalog pack as `<outDir>/<id>-<hash>.rvw`, in catalog order. */
export async function exportPacks(opts: {
  packsDir: string;
  catalogPath: string;
  outDir: string;
}): Promise<{ packs: WebPackInfo[]; appDefault: string }> {
  const catalog = (await Bun.file(opts.catalogPath).json()) as { default: string; packs: CatalogEntry[] };
  const keycodes = [...new Set(Object.values(CODE_TO_KVK))];
  await mkdir(opts.outDir, { recursive: true });
  const packs: WebPackInfo[] = [];
  for (const entry of catalog.packs) {
    const pack = readRvpack(new Uint8Array(await Bun.file(`${opts.packsDir}/${entry.id}.rvpack`).arrayBuffer()));
    const web = toWebPack(pack, entry.id, keycodes);
    const bytes = encodeRvw(web.header, web.pcm);
    const hash = createHash("sha256").update(bytes).digest("hex").slice(0, 10);
    const name = `${entry.id}-${hash}.rvw`;
    await Bun.write(`${opts.outDir}/${name}`, bytes);
    packs.push({
      id: entry.id,
      name: entry.name,
      variant: entry.variant ?? "",
      category: entry.category,
      color: entry.color,
      credit: entry.credit,
      url: `/packs/${name}`,
      bytes: bytes.byteLength,
      hasRelease: Object.keys(web.header.release).length > 0,
    });
  }
  return { packs, appDefault: catalog.default };
}
