import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { renderInstaller } from "../src/worker/install";

const app = new URL("../../target/bundle/Rustyvibes.app", import.meta.url).pathname;
const hasApp = await Bun.file(`${app}/Contents/Info.plist`).exists();

describe.skipIf(!hasApp)("installer against the real app", () => {
  let work: string;
  let server: ReturnType<typeof Bun.serve>;
  let zip: Uint8Array<ArrayBuffer>;
  let sha256: string;

  beforeAll(async () => {
    work = await mkdtemp(join(tmpdir(), "rv-install-"));
    Bun.spawnSync(["ditto", "-c", "-k", "--keepParent", app, join(work, "Rustyvibes.zip")]);
    zip = new Uint8Array(await Bun.file(join(work, "Rustyvibes.zip")).arrayBuffer());
    sha256 = new Bun.CryptoHasher("sha256").update(zip).digest("hex");
    server = Bun.serve({ port: 0, fetch: () => new Response(zip) });
  });
  afterAll(async () => {
    server.stop(true);
    await rm(work, { recursive: true, force: true });
  });

  const script = (overrides: Partial<{ sha256: string; teamId: string }> = {}) =>
    renderInstaller(
      {
        version: "2.0.0", file: "Rustyvibes-2.0.0.zip", size: zip.length, sha256: overrides.sha256 ?? sha256,
        teamId: overrides.teamId ?? "GS3QYYQ6D9", minimumSystemVersion: "13.0", published: "2026-10-02T00:00:00Z",
      },
      `http://localhost:${server.port}`,
    );
  // Async spawn: a synchronous one would block the event loop serving the download.
  const install = async (dest: string, text: string, ...args: string[]) => {
    const proc = Bun.spawn(["/bin/bash", "-s", "--", "--no-open", ...args], {
      stdin: new Blob([text]),
      stdout: "pipe",
      stderr: "pipe",
      env: { ...process.env, RUSTYVIBES_INSTALL_DIR: dest },
    });
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ]);
    return { stdout, stderr, exitCode };
  };

  test("installs a verified, unquarantined copy", async () => {
    const dest = join(work, "Applications");
    const result = await install(dest, script());
    expect(result.stderr).toBe("");
    expect(result.exitCode).toBe(0);
    const target = join(dest, "Rustyvibes.app");
    expect(Bun.spawnSync(["codesign", "--verify", "--deep", "--strict", target]).exitCode).toBe(0);
    expect(Bun.spawnSync(["xattr", "-p", "com.apple.quarantine", target]).exitCode).not.toBe(0);
  }, 60_000);

  test("a checksum mismatch installs nothing", async () => {
    const dest = join(work, "Mismatch");
    const result = await install(dest, script({ sha256: "0".repeat(64) }));
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("Checksum mismatch");
    expect(await Bun.file(join(dest, "Rustyvibes.app/Contents/Info.plist")).exists()).toBe(false);
  }, 60_000);

  test("a different signing team installs nothing", async () => {
    const dest = join(work, "OtherTeam");
    const result = await install(dest, script({ teamId: "ABCDE12345" }));
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("expected developer");
  }, 60_000);

  test("reports a destination it cannot write", async () => {
    const dest = join(work, "Locked");
    await mkdir(join(dest, "Rustyvibes.app"), { recursive: true });
    await chmod(dest, 0o555);
    const result = await install(dest, script());
    await chmod(dest, 0o755);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("Couldn't replace");
  }, 60_000);

  test("--uninstall removes it from the destination only", async () => {
    const dest = join(work, "Applications");
    const result = await install(dest, script(), "--uninstall");
    expect(result.exitCode).toBe(0);
    expect(await Bun.file(join(dest, "Rustyvibes.app/Contents/Info.plist")).exists()).toBe(false);
  }, 60_000);
});
