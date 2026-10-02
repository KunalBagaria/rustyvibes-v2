import { expect, test } from "bun:test";
import { renderInstaller, UNAVAILABLE } from "../src/worker/install";
import type { Release } from "../src/shared/release";

const release: Release = {
  version: "2.0.0", file: "Rustyvibes-2.0.0.zip", size: 12_900_000, sha256: "b".repeat(64),
  teamId: "GS3QYYQ6D9", minimumSystemVersion: "13.0", published: "2026-10-02T12:00:00Z",
};

const run = (cmd: string[], stdin: string) => Bun.spawnSync(cmd, { stdin: new TextEncoder().encode(stdin) });

test("fills every placeholder", () => {
  const script = renderInstaller(release, "https://rustyvibes.kunalbagaria.com");
  expect(script).not.toContain("@@");
  expect(script).toContain('VERSION="2.0.0"');
  expect(script).toContain('URL="https://rustyvibes.kunalbagaria.com/download/Rustyvibes-2.0.0.zip"');
  expect(script).toContain(`SHA256="${"b".repeat(64)}"`);
  expect(script).toContain('TEAM_ID="GS3QYYQ6D9"');
  expect(script).toContain('SIZE="12.9 MB"');
  expect(script.trimEnd().endsWith('main "$@"')).toBe(true);
});

test("refuses origins that could break the quoting", () => {
  expect(() => renderInstaller(release, 'https://x"$(id)')).toThrow();
  expect(() => renderInstaller(release, "file:///etc")).toThrow();
});

test("the rendered script and the fallback parse with bash 3.2", () => {
  for (const script of [renderInstaller(release, "http://localhost:8787"), UNAVAILABLE]) {
    expect(run(["/bin/bash", "-n"], script).exitCode).toBe(0);
  }
});

test("shellcheck is clean", () => {
  const result = run(["shellcheck", "--shell=bash", "-"], renderInstaller(release, "https://rustyvibes.kunalbagaria.com"));
  expect(result.stdout.toString()).toBe("");
  expect(result.exitCode).toBe(0);
});

test("--help prints usage without installing", () => {
  const result = run(["/bin/bash", "-s", "--", "--help"], renderInstaller(release, "http://localhost:1"));
  expect(result.exitCode).toBe(0);
  expect(result.stdout.toString()).toContain("--uninstall");
});

test("quits every running copy by default, only the destination's copy when one is given", () => {
  const script = renderInstaller(release, "http://localhost:1");
  // quit_running silences pgrep's output, so the stand-in records its calls instead.
  const probe = `${script}
calls=()
pgrep() { calls+=("pgrep $*"); return 1; }
quit_running /Applications
RUSTYVIBES_INSTALL_DIR=/tmp/rv-x quit_running /tmp/rv-x
printf '%s\\n' "\${calls[@]}"
`;
  const result = Bun.spawnSync(["/bin/bash", "-s"], {
    stdin: new TextEncoder().encode(probe),
    env: { ...process.env, RUSTYVIBES_SOURCE_ONLY: "1" },
  });
  expect(result.stdout.toString().trim().split("\n")).toEqual([
    "pgrep -x rustyvibes",
    "pgrep -f /tmp/rv-x/Rustyvibes.app/Contents/MacOS/rustyvibes",
  ]);
});
