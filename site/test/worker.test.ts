import { beforeEach, describe, expect, test } from "bun:test";
import app from "../src/worker/index";
import { clearReleaseCache } from "../src/worker/manifest";
import type { Env, StoredObject } from "../src/worker/env";

const PAGE = `<!doctype html><html lang="en" data-release="soon"><body><span data-release-version>–</span><span data-release-size>–</span></body></html>`;
const manifest = {
  version: "2.0.0", file: "Rustyvibes-2.0.0.zip", size: 64, sha256: "c".repeat(64),
  teamId: "GS3QYYQ6D9", minimumSystemVersion: "13.0", published: "2026-10-02T12:00:00Z",
};
const zip = Uint8Array.from({ length: 64 }, (_, i) => i);

class FakeStore {
  objects = new Map<string, Uint8Array<ArrayBuffer>>();
  gets = 0;
  fail = false;
  async head(key: string) { return this.get(key); }
  async get(key: string, options?: { range?: Headers; onlyIf?: Headers }): Promise<StoredObject | null> {
    this.gets++;
    if (this.fail) throw new Error("R2 down");
    const data = this.objects.get(key);
    if (!data) return null;
    const etag = `"etag-${key}"`;
    const base = { size: data.length, httpEtag: etag, writeHttpMetadata: (h: Headers) => h.set("x-meta", "1") };
    if (options?.onlyIf?.get("if-none-match") === etag) return base;
    const range = options?.range?.get("range")?.match(/^bytes=(\d*)-(\d*)$/);
    if (range) {
      const [, a, b] = range;
      if (a === "") {
        const suffix = Number(b);
        return { ...base, range: { suffix }, body: new Response(data.slice(data.length - suffix)).body! };
      }
      const offset = Number(a);
      const length = b === "" ? data.length - offset : Number(b) - offset + 1;
      return { ...base, range: { offset, length }, body: new Response(data.slice(offset, offset + length)).body! };
    }
    return { ...base, body: new Response(data).body! };
  }
}

let store: FakeStore;
let env: Env;
const assets = {
  fetch: async (req: Request) =>
    new URL(req.url).pathname === "/"
      ? new Response(PAGE, { headers: { "content-type": "text/html", etag: '"page"' } })
      : new Response("asset", { status: 200, headers: { "x-asset": "1" } }),
};
const get = (path: string, init?: RequestInit) => app.request(`http://localhost${path}`, init, env);

beforeEach(() => {
  clearReleaseCache();
  store = new FakeStore();
  store.objects.set("latest.json", new TextEncoder().encode(JSON.stringify(manifest)));
  store.objects.set("Rustyvibes-2.0.0.zip", zip);
  env = { ASSETS: assets, RELEASES: store };
});

describe("/", () => {
  test("injects the release and security headers", async () => {
    const res = await get("/");
    const html = await res.text();
    expect(html).toContain('data-release="ready"');
    expect(html).toContain("<span data-release-version>2.0.0</span>");
    expect(html).toContain("<span data-release-size>1 KB</span>");
    expect(res.headers.get("content-security-policy")).toContain("default-src 'self'");
    expect(res.headers.get("etag")).toBeNull();
  });
  test("says coming soon without a release", async () => {
    store.objects.delete("latest.json");
    expect(await (await get("/")).text()).toContain('data-release="soon"');
  });
  test("degrades when R2 fails", async () => {
    store.fail = true;
    const res = await get("/");
    expect(res.status).toBe(200);
    expect(await res.text()).toContain('data-release="soon"');
  });
  test("ignores an invalid manifest", async () => {
    store.objects.set("latest.json", new TextEncoder().encode(JSON.stringify({ ...manifest, sha256: "nope" })));
    expect(await (await get("/")).text()).toContain('data-release="soon"');
  });
  test("caches the manifest for a minute", async () => {
    await get("/");
    await get("/");
    expect(store.gets).toBe(1);
  });
});

describe("/install", () => {
  test("renders the installer for this origin", async () => {
    const res = await get("/install");
    expect(res.headers.get("content-type")).toStartWith("text/plain");
    const script = await res.text();
    expect(script).toContain('URL="http://localhost/download/Rustyvibes-2.0.0.zip"');
    expect(script).not.toContain("@@");
    expect(await (await get("/install.sh")).text()).toBe(script);
  });
  test("explains itself without a release", async () => {
    store.objects.delete("latest.json");
    expect(await (await get("/install")).text()).toContain("isn't available to install yet");
  });
});

describe("/download", () => {
  test("redirects to the current file, or to the install section", async () => {
    const res = await get("/download");
    expect(res.status).toBe(302);
    expect(res.headers.get("location")).toBe("/download/Rustyvibes-2.0.0.zip");
    clearReleaseCache();
    store.objects.delete("latest.json");
    expect((await get("/download")).headers.get("location")).toBe("/#install");
  });
  test("streams the zip", async () => {
    const res = await get("/download/Rustyvibes-2.0.0.zip");
    expect(res.status).toBe(200);
    expect(new Uint8Array(await res.arrayBuffer())).toEqual(zip);
    expect(res.headers.get("content-type")).toBe("application/zip");
    expect(res.headers.get("content-disposition")).toBe('attachment; filename="Rustyvibes-2.0.0.zip"');
    expect(res.headers.get("cache-control")).toContain("immutable");
  });
  test("serves byte ranges", async () => {
    const res = await get("/download/Rustyvibes-2.0.0.zip", { headers: { range: "bytes=10-19" } });
    expect(res.status).toBe(206);
    expect(res.headers.get("content-range")).toBe("bytes 10-19/64");
    expect(Array.from(new Uint8Array(await res.arrayBuffer()))).toEqual(Array.from(zip.slice(10, 20)));
    const tail = await get("/download/Rustyvibes-2.0.0.zip", { headers: { range: "bytes=-4" } });
    expect(tail.headers.get("content-range")).toBe("bytes 60-63/64");
  });
  test("answers conditional requests with 304", async () => {
    const res = await get("/download/Rustyvibes-2.0.0.zip", { headers: { "if-none-match": '"etag-Rustyvibes-2.0.0.zip"' } });
    expect(res.status).toBe(304);
  });
  test("HEAD reports the size without a body", async () => {
    const res = await get("/download/Rustyvibes-2.0.0.zip", { method: "HEAD" });
    expect(res.status).toBe(200);
    expect(res.headers.get("content-length")).toBe("64");
  });
  test("refuses anything else", async () => {
    for (const path of ["/download/latest.json", "/download/Rustyvibes-9.9.9.zip", "/download/..%2Flatest.json"]) {
      expect((await get(path)).status).toBe(404);
    }
  });
});

test("/api/release", async () => {
  expect(await (await get("/api/release")).json()).toEqual(manifest);
  clearReleaseCache();
  store.objects.delete("latest.json");
  const res = await get("/api/release");
  expect(res.status).toBe(404);
  expect(await res.json()).toEqual({ available: false });
});

test("everything else goes to static assets", async () => {
  const res = await get("/assets/x.js");
  expect(res.headers.get("x-asset")).toBe("1");
});
