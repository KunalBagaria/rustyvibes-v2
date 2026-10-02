# rustyvibes.kunalbagaria.com

The Rustyvibes website and installer. Static pages are bundled with Bun and served by a Cloudflare
Worker (Hono) through Workers Static Assets; releases live in the R2 bucket `rustyvibes-releases`.

- `GET /` is the page, with the current release filled in by `HTMLRewriter`.
- `GET /install` is the installer script, rendered from `latest.json`.
- `GET /download/Rustyvibes-<version>.zip` streams a release from R2 (Range and ETag aware).
- `GET /api/release` returns `latest.json`.

The design is in `docs/superpowers/specs/2026-10-02-rustyvibes-site-design.md`.

## Develop

```sh
bun install
bun run build                  # web soundpacks from ../target/packs, then the page into dist/
bunx wrangler dev              # http://localhost:8787 (local R2 is empty, so it says "coming soon")
bun run check                  # type-check and unit tests (installer integration tests need ../target/bundle)
bun scripts/shoot.ts [url]     # screenshots at three sizes; fails on console errors, overflow or idle frames
bun scripts/interact.ts [url]  # drives the page like a visitor: typing, demos, packs, menu, copy
```

Restart `wrangler dev` after a build: it does not notice the rebuilt `dist/`.

## Release and deploy

```sh
bun run release   # cargo xtask bundle → zip → R2 (the zip first, then latest.json)
bun run deploy    # build, then wrangler deploy (Worker rustyvibes-site on rustyvibes.kunalbagaria.com)
```

The site and `/install` pick up a new release within a minute; no redeploy is needed.

## Images

```sh
bun scripts/icons.ts    # page icons from assets/icon/AppIcon-1024.png
bun scripts/poster.ts   # keyboard poster and social card (run after a build)
```

## Layout

| Path | What |
|---|---|
| `src/index.html`, `src/styles/` | The page and its design system |
| `src/client/keyboard/` | The procedural Three.js keyboard (layout, legend atlas, geometry, scene) |
| `src/client/audio/` | The Web Audio engine: the app's variation, pan law, gain curve and limiter |
| `src/client/ui/` | Hero stage, soundpack gallery, menu replica, demos, reveals |
| `src/shared/` | Code shared by the page, the Worker and the build: key tables, sound maths, `.rvw`, release manifest |
| `src/worker/` | The Worker and the installer template |
| `scripts/` | Build, web-pack export, release, posters, screenshots |
