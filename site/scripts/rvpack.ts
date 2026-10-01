/** Reader for the app's `.rvpack` format (crates/rvpack/src/format.rs). */
export interface RvPack {
  sampleRate: number;
  hasRelease: boolean;
  clips: { start: number; len: number }[];
  press: { first: number; count: number }[];
  release: { first: number; count: number }[];
  meta: Record<string, string>;
  pcm: Int16Array;
}

export function readRvpack(bytes: Uint8Array): RvPack {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length < 1056 || new TextDecoder().decode(bytes.subarray(0, 4)) !== "RVPK") {
    throw new Error("not an .rvpack file");
  }
  const version = view.getUint16(4, true);
  if (version !== 1) throw new Error(`unsupported .rvpack version ${version}`);
  const u32 = (at: number) => view.getUint32(at, true);
  const [flags, sampleRate, clipCount] = [view.getUint16(6, true), u32(8), u32(12)];
  const [pcmOffset, pcmLength, metaOffset, metaLength] = [u32(16), u32(20), u32(24), u32(28)];
  if (pcmOffset + pcmLength * 2 > bytes.length || metaOffset + metaLength > bytes.length) {
    throw new Error("truncated .rvpack file");
  }
  const map = (base: number) =>
    Array.from({ length: 128 }, (_, i) => ({
      first: view.getUint16(base + 4 * i, true),
      count: view.getUint16(base + 4 * i + 2, true),
    }));
  const clips = Array.from({ length: clipCount }, (_, i) => ({ start: u32(1056 + 8 * i), len: u32(1060 + 8 * i) }));
  const meta: Record<string, string> = {};
  for (const line of new TextDecoder().decode(bytes.subarray(metaOffset, metaOffset + metaLength)).split("\n")) {
    const eq = line.indexOf("=");
    if (eq > 0) meta[line.slice(0, eq)] = line.slice(eq + 1);
  }
  const pcm = new Int16Array(pcmLength);
  new Uint8Array(pcm.buffer).set(bytes.subarray(pcmOffset, pcmOffset + pcmLength * 2));
  return { sampleRate, hasRelease: (flags & 1) !== 0, clips, press: map(32), release: map(544), meta, pcm };
}
