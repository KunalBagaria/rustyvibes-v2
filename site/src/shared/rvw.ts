/**
 * `.rvw`, a web soundpack: "RVW1" | u32 LE JSON length | JSON header | zero padding
 * to an even offset | mono i16 LE PCM. (JavaScript hosts are little-endian.)
 */
export interface RvwHeader {
  id: string;
  sampleRate: number;
  /** [start, length] in samples within the PCM block. */
  clips: [number, number][];
  /** macOS keycode → [first clip, candidate count]. */
  press: Record<string, [number, number]>;
  release: Record<string, [number, number]>;
}

const MAGIC = "RVW1";
const pcmOffsetFor = (jsonLength: number) => (8 + jsonLength + 1) & ~1;

export function encodeRvw(header: RvwHeader, pcm: Int16Array): Uint8Array<ArrayBuffer> {
  const json = new TextEncoder().encode(JSON.stringify(header));
  const offset = pcmOffsetFor(json.length);
  const out = new Uint8Array(offset + pcm.byteLength);
  out.set(new TextEncoder().encode(MAGIC), 0);
  new DataView(out.buffer).setUint32(4, json.length, true);
  out.set(json, 8);
  out.set(new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength), offset);
  return out;
}

export function decodeRvw(buffer: ArrayBuffer): { header: RvwHeader; pcm: Int16Array } {
  const bytes = new Uint8Array(buffer);
  if (bytes.length < 8 || new TextDecoder().decode(bytes.subarray(0, 4)) !== MAGIC) {
    throw new Error("not a web soundpack");
  }
  const jsonLength = new DataView(buffer).getUint32(4, true);
  if (8 + jsonLength > bytes.length) throw new Error("truncated web soundpack");
  const header = JSON.parse(new TextDecoder().decode(bytes.subarray(8, 8 + jsonLength))) as RvwHeader;
  const offset = pcmOffsetFor(jsonLength);
  const pcm = new Int16Array(buffer, offset, Math.max(0, (bytes.length - offset) >> 1));
  for (const [start, length] of header.clips) {
    if (!(start >= 0 && length > 0 && start + length <= pcm.length)) {
      throw new Error("clip outside the PCM block");
    }
  }
  for (const map of [header.press, header.release]) {
    for (const [first, count] of Object.values(map)) {
      if (!(first >= 0 && count > 0 && first + count <= header.clips.length)) {
        throw new Error("key map refers to a missing clip");
      }
    }
  }
  return { header, pcm };
}
