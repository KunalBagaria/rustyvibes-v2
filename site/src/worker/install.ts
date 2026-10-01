import TEMPLATE from "./installer.sh";
import { formatBytes, type Release } from "../shared/release";

const ORIGIN = /^https?:\/\/[A-Za-z0-9.-]+(:\d{1,5})?$/;

/** Served when no release is published yet. */
export const UNAVAILABLE = `#!/bin/bash
echo "Rustyvibes isn't available to install yet. Check https://rustyvibes.kunalbagaria.com again soon." >&2
exit 1
`;

/** The installer for `release`, downloading from `origin` (scheme://host[:port]). */
export function renderInstaller(release: Release, origin: string): string {
  if (!ORIGIN.test(origin)) throw new Error(`refusing to embed origin ${JSON.stringify(origin)}`);
  const values: Record<string, string> = {
    VERSION: release.version,
    URL: `${origin}/download/${release.file}`,
    SHA256: release.sha256,
    TEAM_ID: release.teamId,
    MIN_MACOS: release.minimumSystemVersion,
    SIZE: formatBytes(release.size),
  };
  return TEMPLATE.replace(/@@([A-Z0-9_]+)@@/g, (_, key: string) => {
    const value = values[key];
    if (value === undefined) throw new Error(`unknown installer placeholder ${key}`);
    return value;
  });
}
