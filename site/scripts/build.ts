import { $ } from "bun";
import { cp, mkdir, readdir, rm } from "node:fs/promises";
import { checkBudget, type BudgetEntry } from "./budget";
import { catalogModule, renderPartials, WEB_DEFAULT_PACK } from "./partials";
import { exportPacks } from "./webpacks";

const site = new URL("..", import.meta.url).pathname;
const repo = new URL("../..", import.meta.url).pathname;
const dist = `${site}dist`;

const HEADERS = `/assets/*
  Cache-Control: public, max-age=31536000, immutable
/packs/*
  Cache-Control: public, max-age=31536000, immutable
/fonts/*
  Cache-Control: public, max-age=31536000, immutable
/*
  X-Content-Type-Options: nosniff
  Referrer-Policy: strict-origin-when-cross-origin
`;

await rm(dist, { recursive: true, force: true });
await mkdir(dist, { recursive: true });

if (!(await Bun.file(`${repo}target/packs/${WEB_DEFAULT_PACK}.rvpack`).exists())) {
  await $`cargo xtask packs`.cwd(repo);
}
const { packs } = await exportPacks({
  packsDir: `${repo}target/packs`,
  catalogPath: `${repo}assets/soundpacks/catalog.json`,
  outDir: `${dist}/packs`,
});
await Bun.write(`${site}src/client/catalog.gen.ts`, catalogModule(packs, WEB_DEFAULT_PACK));
const partials = renderPartials(packs, WEB_DEFAULT_PACK);

const result = await Bun.build({
  entrypoints: [`${site}src/index.html`, `${site}src/404.html`],
  outdir: dist,
  minify: true,
  splitting: true,
  // Fonts are served from public/ as-is: inlined, every visitor would pay for a font only
  // non-Apple devices use.
  external: ["/fonts/*"],
  naming: { entry: "[name].[ext]", chunk: "assets/[name]-[hash].[ext]", asset: "assets/[name]-[hash].[ext]" },
  plugins: [
    {
      name: "partials",
      setup(build) {
        build.onLoad({ filter: /\.html$/ }, async ({ path }) => {
          const text = (await Bun.file(path).text()).replace(/<!--@([a-z-]+)-->/g, (_, name: string) => {
            const html = partials[name];
            if (html === undefined) throw new Error(`no partial named ${name}`);
            return html;
          });
          return { contents: text, loader: "html" };
        });
      },
    },
  ],
});
if (!result.success) {
  for (const log of result.logs) console.error(log);
  process.exit(1);
}
await cp(`${site}public`, dist, { recursive: true });
await Bun.write(`${dist}/_headers`, HEADERS);

const gz = async (path: string) => Bun.gzipSync(new Uint8Array(await Bun.file(path).arrayBuffer())).byteLength;
const sum = (values: number[]) => values.reduce((a, b) => a + b, 0);
const outputs = result.outputs.map((o) => ({ path: o.path, kind: o.kind }));
const js = outputs.filter((o) => o.path.endsWith(".js"));
const entryJs = js.filter((o) => o.kind === "entry-point");
const chunks = js.filter((o) => o.kind === "chunk");
const css = outputs.filter((o) => o.path.endsWith(".css"));
const posters = (await readdir(`${dist}/assets`)).filter((f) => /^poster-.*\.(avif|webp|jpg)$/.test(f));
const defaultPack = packs.find((p) => p.id === WEB_DEFAULT_PACK)!;
const entries: BudgetEntry[] = [
  { label: "index.html (gzip)", bytes: await gz(`${dist}/index.html`), limit: 25_000 },
  { label: "CSS (gzip)", bytes: sum(await Promise.all(css.map((o) => gz(o.path)))), limit: 20_000 },
  { label: "entry JS (gzip)", bytes: sum(await Promise.all(entryJs.map((o) => gz(o.path)))), limit: 30_000 },
  { label: "largest lazy chunk (gzip)", bytes: Math.max(0, ...(await Promise.all(chunks.map((o) => gz(o.path))))), limit: 180_000 },
  { label: `default pack ${defaultPack.id}`, bytes: defaultPack.bytes, limit: 100_000 },
  ...posters.map((f) => ({ label: f, bytes: Bun.file(`${dist}/assets/${f}`).size, limit: 160_000 })),
];
const report = checkBudget(entries);
console.log(report.lines.join("\n"));
if (!report.ok) process.exit(1);
console.log(`built ${dist} (${packs.length} packs)`);
