import { Hono, type Context } from "hono";
import { serveDownload } from "./download";
import type { Env } from "./env";
import { renderInstaller, UNAVAILABLE } from "./install";
import { currentRelease } from "./manifest";
import { withRelease } from "./rewrite";

const app = new Hono<{ Bindings: Env }>();

app.get("/", async (c) => {
  const [page, release] = await Promise.all([
    c.env.ASSETS.fetch(new Request(new URL("/", c.req.url))),
    currentRelease(c.env.RELEASES),
  ]);
  return page.ok ? withRelease(page, release) : page;
});

app.get("/index.html", (c) => c.redirect("/", 301));

const installer = async (c: Context<{ Bindings: Env }>) => {
  const release = await currentRelease(c.env.RELEASES);
  const script = release ? renderInstaller(release, new URL(c.req.url).origin) : UNAVAILABLE;
  return c.text(script, 200, { "cache-control": "public, max-age=60", "x-content-type-options": "nosniff" });
};
app.get("/install", installer);
app.get("/install.sh", installer);

app.get("/download", async (c) => {
  const release = await currentRelease(c.env.RELEASES);
  return c.redirect(release ? `/download/${release.file}` : "/#install", 302);
});
app.on(["GET", "HEAD"], "/download/:file", (c) => serveDownload(c.env.RELEASES, c.req.param("file"), c.req.raw));

app.get("/api/release", async (c) => {
  const release = await currentRelease(c.env.RELEASES);
  return release
    ? c.json(release, 200, { "cache-control": "public, max-age=60" })
    : c.json({ available: false }, 404);
});

app.all("*", (c) => c.env.ASSETS.fetch(c.req.raw));

export default app;
