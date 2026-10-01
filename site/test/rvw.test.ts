import { expect, test } from "bun:test";
import { decodeRvw, encodeRvw, type RvwHeader } from "../src/shared/rvw";

const header: RvwHeader = { id: "t", sampleRate: 44100, clips: [[0, 3], [3, 2]], press: { "49": [0, 1] }, release: { "49": [1, 1] } };

test("round-trips header and PCM", () => {
  const pcm = Int16Array.from([1, -2, 3, 32767, -32768]);
  const bytes = encodeRvw(header, pcm);
  const decoded = decodeRvw(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
  expect(decoded.header).toEqual(header);
  expect(Array.from(decoded.pcm)).toEqual(Array.from(pcm));
});

test("PCM starts on an even offset whatever the header length", () => {
  for (const id of ["a", "ab"]) {
    const bytes = encodeRvw({ ...header, id }, Int16Array.from([7, 7, 7, 7, 7]));
    expect(() => decodeRvw(bytes.slice().buffer)).not.toThrow();
  }
});

test("rejects foreign, truncated and inconsistent files", () => {
  expect(() => decodeRvw(new TextEncoder().encode("RVPK....").buffer)).toThrow("not a web soundpack");
  const ok = encodeRvw(header, Int16Array.from([1, 2, 3, 4, 5]));
  expect(() => decodeRvw(ok.slice(0, 12).buffer)).toThrow();
  const bad = encodeRvw({ ...header, clips: [[0, 9]] }, Int16Array.from([1, 2, 3]));
  expect(() => decodeRvw(bad.slice().buffer)).toThrow("clip outside");
  const badRef = encodeRvw({ ...header, press: { "49": [1, 5] } }, Int16Array.from([1, 2, 3, 4, 5]));
  expect(() => decodeRvw(badRef.slice().buffer)).toThrow("key map");
});
