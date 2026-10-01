import { DOWNLOAD_FILE } from "../shared/release";
import type { ReleaseStore, StoredObject } from "./env";

function rangeOf(range: NonNullable<StoredObject["range"]>, size: number): { offset: number; length: number } {
  if (range.suffix !== undefined) {
    const length = Math.min(range.suffix, size);
    return { offset: size - length, length };
  }
  const offset = range.offset ?? 0;
  return { offset, length: range.length ?? size - offset };
}

/** Streams a release zip from R2, honouring Range and If-None-Match. */
export async function serveDownload(store: ReleaseStore, file: string, request: Request): Promise<Response> {
  if (!DOWNLOAD_FILE.test(file)) return new Response("Not found", { status: 404 });
  const head = request.method === "HEAD";
  const object = head
    ? await store.head(file)
    : await store.get(file, { range: request.headers, onlyIf: request.headers });
  if (!object) return new Response("Not found", { status: 404 });

  const headers = new Headers();
  object.writeHttpMetadata(headers);
  headers.set("etag", object.httpEtag);
  headers.set("content-type", "application/zip");
  headers.set("content-disposition", `attachment; filename="${file}"`);
  headers.set("cache-control", "public, max-age=31536000, immutable");
  headers.set("accept-ranges", "bytes");
  if (head) {
    headers.set("content-length", String(object.size));
    return new Response(null, { status: 200, headers });
  }
  if (!object.body) return new Response(null, { status: 304, headers });
  if (object.range && request.headers.has("range")) {
    const { offset, length } = rangeOf(object.range, object.size);
    headers.set("content-range", `bytes ${offset}-${offset + length - 1}/${object.size}`);
    headers.set("content-length", String(length));
    return new Response(object.body, { status: 206, headers });
  }
  headers.set("content-length", String(object.size));
  return new Response(object.body, { status: 200, headers });
}
