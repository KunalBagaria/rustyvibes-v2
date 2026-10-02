/** The release manifest stored in R2 as `latest.json` (spec §4). */
export interface Release {
  version: string;
  file: string;
  size: number;
  sha256: string;
  /** Developer ID team that signed the app; must be SIGNING_TEAM. */
  teamId: string;
  minimumSystemVersion: string;
  published: string;
}

const VERSION = /^\d+\.\d+\.\d+$/;
const SHA256 = /^[0-9a-f]{64}$/;
const MACOS = /^\d+(\.\d+)?$/;

/**
 * The Developer ID team that signs Rustyvibes. The Worker ignores a manifest naming any other
 * team and the installer checks the signature against it, so write access to the bucket
 * alone can't swap in a different app. Change it only when the signing certificate changes.
 */
export const SIGNING_TEAM = "GS3QYYQ6D9";

/** Names the Worker will serve from `/download/`. */
export const DOWNLOAD_FILE = /^Rustyvibes-\d+\.\d+\.\d+\.zip$/;

export const releaseFile = (version: string) => `Rustyvibes-${version}.zip`;

/**
 * The manifest when every field is well-formed, otherwise `null`. Values that pass
 * are safe inside the installer's double quotes.
 */
export function parseRelease(value: unknown): Release | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const { version, file, size, sha256, teamId, minimumSystemVersion, published } =
    value as Record<string, unknown>;
  if (typeof version !== "string" || !VERSION.test(version)) return null;
  if (file !== releaseFile(version)) return null;
  if (typeof size !== "number" || !Number.isSafeInteger(size) || size <= 0) return null;
  if (typeof sha256 !== "string" || !SHA256.test(sha256)) return null;
  if (teamId !== SIGNING_TEAM) return null;
  if (typeof minimumSystemVersion !== "string" || !MACOS.test(minimumSystemVersion)) return null;
  if (typeof published !== "string" || Number.isNaN(Date.parse(published))) return null;
  return { version, file, size, sha256, teamId, minimumSystemVersion, published };
}

/** Decimal sizes like Finder: "79 KB", "12.9 MB". */
export function formatBytes(bytes: number): string {
  const kb = Math.round(bytes / 1e3);
  return kb < 1000 ? `${Math.max(kb, 1)} KB` : `${(bytes / 1e6).toFixed(1)} MB`;
}
