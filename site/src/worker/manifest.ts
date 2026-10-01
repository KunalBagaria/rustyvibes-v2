import { parseRelease, type Release } from "../shared/release";
import type { ReleaseStore } from "./env";

const FRESH_MS = 60_000;
const RETRY_MS = 5_000;
let cached: { release: Release | null; expires: number } | null = null;

/** The published release, memoised per isolate. Failures degrade to "no release". */
export async function currentRelease(store: ReleaseStore, now = Date.now()): Promise<Release | null> {
  if (cached && cached.expires > now) return cached.release;
  try {
    const object = await store.get("latest.json");
    const release = object?.body ? parseRelease(JSON.parse(await new Response(object.body).text())) : null;
    if (object && !release) console.error("latest.json is malformed; serving no release");
    cached = { release, expires: now + FRESH_MS };
    return release;
  } catch (error) {
    console.error("couldn't read latest.json", error);
    cached = { release: null, expires: now + RETRY_MS };
    return null;
  }
}

export function clearReleaseCache(): void {
  cached = null;
}
