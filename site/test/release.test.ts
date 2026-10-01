import { describe, expect, test } from "bun:test";
import { DOWNLOAD_FILE, formatBytes, parseRelease, releaseFile } from "../src/shared/release";

const good = {
  version: "2.0.0",
  file: "Rustyvibes-2.0.0.zip",
  size: 12_900_000,
  sha256: "a".repeat(64),
  teamId: "GS3QYYQ6D9",
  minimumSystemVersion: "13.0",
  published: "2026-10-02T12:00:00Z",
};

describe("parseRelease", () => {
  test("accepts a well-formed manifest", () => {
    expect(parseRelease(good)).toEqual(good);
    expect(parseRelease({ ...good, teamId: "" })?.teamId).toBe("");
  });

  test.each([
    ["version", "2.0"],
    ["version", '2.0.0"; rm -rf ~; "'],
    ["file", "Rustyvibes-2.0.1.zip"],
    ["file", "../latest.json"],
    ["size", 0],
    ["size", 1.5],
    ["size", "12"],
    ["sha256", "A".repeat(64)],
    ["sha256", "a".repeat(63)],
    ["teamId", "gs3qyyq6d9"],
    ["teamId", "GS3QYYQ6D9$(x)"],
    ["minimumSystemVersion", "13.0; echo"],
    ["published", "yesterday"],
  ])("rejects a bad %s (%p)", (field, value) => {
    expect(parseRelease({ ...good, [field]: value })).toBeNull();
  });

  test("rejects non-objects", () => {
    for (const value of [null, 42, "x", []]) expect(parseRelease(value)).toBeNull();
  });
});

test("release file names", () => {
  expect(releaseFile("2.0.0")).toBe("Rustyvibes-2.0.0.zip");
  expect(DOWNLOAD_FILE.test("Rustyvibes-2.0.0.zip")).toBe(true);
  for (const name of ["Rustyvibes-2.0.0.dmg", "x/Rustyvibes-2.0.0.zip", "Rustyvibes-2.0.zip"]) {
    expect(DOWNLOAD_FILE.test(name)).toBe(false);
  }
});

test("formatBytes reads like Finder", () => {
  expect(formatBytes(12_900_000)).toBe("12.9 MB");
  expect(formatBytes(999_999)).toBe("1.0 MB");
  expect(formatBytes(79_400)).toBe("79 KB");
  expect(formatBytes(10)).toBe("1 KB");
});
