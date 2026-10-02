// Publishes the app for the installer: builds and signs it, zips it, uploads the zip and
// `latest.json` to R2. The site picks the new release up within a minute.
//   bun scripts/release.ts [--skip-build]
import { $ } from "bun";
import { parseRelease, releaseFile, type Release } from "../src/shared/release";

const repo = new URL("../..", import.meta.url).pathname;
const site = new URL("..", import.meta.url).pathname;
const bucket = "rustyvibes-releases";

const cargo = await Bun.file(`${repo}Cargo.toml`).text();
const version = cargo.match(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/m)?.[1];
if (!version) throw new Error("no [workspace.package] version in Cargo.toml");

if (!process.argv.includes("--skip-build")) await $`cargo xtask bundle`.cwd(repo);
const app = `${repo}target/bundle/Rustyvibes.app`;
await $`codesign --verify --deep --strict ${app}`;

const zip = `${repo}target/${releaseFile(version)}`;
await $`rm -f ${zip}`;
await $`ditto -c -k --keepParent ${app} ${zip}`;
const bytes = await Bun.file(zip).bytes();
const signature = await $`codesign -dv ${app}`.nothrow().quiet();
const teamId = signature.stderr.toString().match(/^TeamIdentifier=([A-Z0-9]{10})$/m)?.[1] ?? "";

const manifest: Release = {
  version,
  file: releaseFile(version),
  size: bytes.length,
  sha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex"),
  teamId,
  minimumSystemVersion: "13.0",
  published: new Date().toISOString(),
};
if (!parseRelease(manifest)) throw new Error(`refusing to publish an invalid manifest: ${JSON.stringify(manifest)}`);
const latest = `${repo}target/latest.json`;
await Bun.write(latest, `${JSON.stringify(manifest, null, 2)}\n`);

// The zip first: latest.json must never point at a file that isn't there yet.
await $`bunx wrangler r2 object put ${bucket}/${manifest.file} --file ${zip} --content-type application/zip --remote`.cwd(site);
await $`bunx wrangler r2 object put ${bucket}/latest.json --file ${latest} --content-type application/json --cache-control no-cache --remote`.cwd(site);
console.log(
  `published Rustyvibes ${version}: ${manifest.size} bytes, sha256 ${manifest.sha256}, team ${teamId || "none (ad-hoc)"}`,
);
