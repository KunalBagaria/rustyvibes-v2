# Rustyvibes website — design

Date: 2026-10-02 · Status: approved by Claude on the owner's behalf (owner AFK, delegated every
decision: "take all the executive decisions that you want").

## 1. Goal

An Apple-style product page for Rustyvibes at **https://rustyvibes.kunalbagaria.com** that:

- lets visitors *hear* the app: the real soundpacks, typed on a live 3D keyboard;
- installs the app with **one pasted Terminal command**, which replaces the download button.
  The owner has no paid Apple Developer membership, so the app cannot be notarized;
- is built with Bun + TypeScript and served by a Cloudflare Worker (Hono) with Workers Static
  Assets, with releases stored in R2.

### Success criteria

1. `curl -fsSL https://rustyvibes.kunalbagaria.com/install | bash` downloads the current release,
   verifies its SHA-256 and code signature, installs `Rustyvibes.app` into `/Applications` (or
   `~/Applications` when that is not writable) without the quarantine flag, and opens it: no
   Gatekeeper prompt, no admin password.
2. The page sounds like the app: same clips, ±35 cents / ±1.5 dB variation, the same pan law,
   volume² gain curve and soft-knee limiter.
3. Fast and frugal, like the app: Lighthouse performance ≥ 95 (desktop) and ≥ 90 (mobile); no
   layout shift; ≤ 350 KB transferred before the visitor interacts; **no animation frames while
   nothing moves** (render on demand, no infinite CSS animations).
4. Degrades gracefully: without WebGL the poster image stays and sound still works; without
   JavaScript all content and the install command remain readable and copyable.

## 2. Decisions

| # | Decision | Why | Cost if wrong |
|---|---|---|---|
| D1 | Site lives in `site/` of this repo | Shares the icon and soundpacks; one place to release from | Moving a directory |
| D2 | Hand-written semantic HTML/CSS + TypeScript islands, no UI framework | Smallest JS, full control of Apple-grade detail | More hand-written DOM code |
| D3 | Workers Static Assets + a Hono Worker for `/`, `/install`, `/download/*`, `/api/*`; `HTMLRewriter` injects release data into the static HTML | Static speed, live release info, no SSR framework | A few ms per HTML request |
| D4 | R2 bucket `rustyvibes-releases` holds versioned zips and `latest.json` | Releases independent of site deploys; no 25 MiB asset limit | One more service |
| D5 | Fonts: system stack (SF Pro on Apple devices) with self-hosted Inter Variable as fallback; no SF Pro webfont | Apple's font licence limits SF Pro to UI mock-ups; Apple visitors still see SF Pro | Non-Apple visitors see Inter |
| D6 | Dark page in Apple's pro-product style; palette from the app icon (rust orange, cream keycap) | Matches the icon; makes the 3D keyboard glow | Restyle |
| D7 | Procedural Three.js keyboard: a 75% Mac layout, one material and one legend atlas, rendered on demand | No model files, exact legends, colourways for free, idle = 0 frames | Bundle ~150 KB gz |
| D8 | Web soundpacks (`.rvw`) derived from the built `.rvpack` files; packs with more than 32 clips keep dedicated clips for 8 special keys and 16 shared clips for the rest | Big packs drop from 0.9–1.5 MB to 280–500 KB (the 12-clip packs stay 73–116 KB); variation hides the sharing | Slightly less per-key character on the web |
| D9 | Web default pack: Holy Panda (79 KB, has release sounds) | Smallest great-sounding first impression | The app's default is Cherry MX Brown |
| D10 | British English copy | The owner's and the README's spelling | Find/replace |
| D11 | bash 3.2–compatible installer that verifies checksum + team-ID signature, never uses sudo, strips quarantine, offers `--uninstall` | Safe, transparent "paste and go" without notarization | — |
| D12 | No analytics, cookies or third-party requests | On-brand with the app's privacy story; simpler CSP | No visitor numbers |
| D13 | No download button anywhere (owner's call); the zip URL exists for the script | Owner's instruction | — |

## 3. Architecture

```
browser ──► Cloudflare Worker "rustyvibes-site" (Hono)
              │  GET /              static index.html + HTMLRewriter(release data)
              │  GET /install(.sh)  installer script rendered from latest.json
              │  GET|HEAD /download/:file   R2 object stream (Range-aware)
              │  GET /api/release   latest.json (or 404)
              │  everything else ─► env.ASSETS (dist/: hashed JS/CSS/media, packs, fonts)
              └─► R2 "rustyvibes-releases": Rustyvibes-<version>.zip, latest.json
```

`wrangler.jsonc`: `assets.directory = ./dist`, `assets.binding = ASSETS`,
`run_worker_first = ["/", "/index.html", "/install", "/install.sh", "/download", "/download/*",
"/api/*"]`, R2 binding `RELEASES`, custom domain `rustyvibes.kunalbagaria.com`.

**Build** (`bun run build`):

1. Export web packs from `../target/packs/*.rvpack` (running `cargo xtask packs` when they are
   missing) into `dist/packs/<id>-<hash>.rvw`.
2. Generate `src/client/catalog.gen.ts` (pack metadata and hashed URLs) and the HTML partials:
   pack cards, menu items and JSON-LD.
3. `Bun.build` with `src/index.html` as the entry point, minified, hashed assets in
   `dist/assets/`.
4. Copy `public/` (icons, fonts, OG image, robots, sitemap) and write `dist/_headers`.
5. Check the size budget (§11); fail the build when it is exceeded.

**Release** (`bun run release`): `cargo xtask bundle` (universal, Developer ID signed) →
`ditto -c -k --keepParent` → zip → SHA-256, size and team ID (read from `codesign -dv`) →
`wrangler r2 object put --remote` the zip, then `latest.json`. Re-running with the same version
is allowed (it replaces the release).

## 4. Release manifest (`latest.json`)

```json
{ "version": "2.0.0", "file": "Rustyvibes-2.0.0.zip", "size": 12900000,
  "sha256": "<64 lowercase hex>", "teamId": "GS3QYYQ6D9",
  "minimumSystemVersion": "13.0", "published": "2026-10-02T12:00:00Z" }
```

Validation (Worker and release script share one module): `version` matches `^\d+\.\d+\.\d+$`;
`file` equals `Rustyvibes-<version>.zip`; `size` is a positive integer; `sha256` matches
`^[0-9a-f]{64}$`; `teamId` matches `^[A-Z0-9]{10}$` or is empty (ad-hoc builds);
`minimumSystemVersion` matches `^\d+(\.\d+)?$`. An invalid manifest counts as "no release" and is
logged. Every value placed in the shell script passes these patterns, so none can break out of
the script's quoting.

## 5. Worker behaviour

- **`GET /`**: fetch `index.html` from `ASSETS`, then rewrite: `<html data-release="ready|soon">`,
  text of `[data-release-version]` → `2.0.0`, `[data-release-size]` → `12.9 MB`. The static
  default is `soon`, so the page stays coherent without the Worker. Adds security headers:
  CSP `default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline';
  script-src 'self'; connect-src 'self'; font-src 'self'; media-src 'self' blob:;
  frame-ancestors 'none'; base-uri 'self'; form-action 'none'`, plus `X-Content-Type-Options`,
  `Referrer-Policy: strict-origin-when-cross-origin` and `Cache-Control: public, max-age=0,
  must-revalidate`.
- **`GET /install`, `/install.sh`**: `text/plain; charset=utf-8`,
  `Cache-Control: public, max-age=60`. Renders the script template with version, absolute zip
  URL (request origin + `/download/<file>`), SHA-256, team ID and minimum macOS. Without a
  release it returns a script that prints "Rustyvibes isn't available to install yet" and exits 1.
- **`GET|HEAD /download/:file`**: only names matching `^Rustyvibes-\d+\.\d+\.\d+\.zip$`; streams
  the R2 object with `Content-Type: application/zip`, `Content-Disposition: attachment`, `ETag`,
  `Content-Length` and `Cache-Control: public, max-age=31536000, immutable`; honours `Range`
  (206 + `Content-Range`) and `If-None-Match` (304). Unknown → 404.
- **`GET /download`**: 302 to the current file, or to `/#install` when there is no release.
- **`GET /api/release`**: the manifest (200), or `{"available":false}` (404).
- The manifest is memoised per isolate for 60 s; R2 errors degrade to "no release", never a 500
  on `/`.

## 6. Installer (`/install`)

bash 3.2–compatible; the whole body lives in functions with `main "$@"` on the last line, so a
truncated download never runs half a script. It never reads stdin, because stdin is the pipe.

1. Refuse non-macOS, and macOS older than `minimumSystemVersion`.
2. `--uninstall`: quit the app (`pkill -x rustyvibes`), remove `Rustyvibes.app` from
   `/Applications` and `~/Applications`, and say how to remove the Input Monitoring entry. Done.
3. Choose the destination: `$RUSTYVIBES_INSTALL_DIR` if set (tests/advanced); else
   `/Applications` when writable; else `~/Applications` (created).
4. Download the zip to a `mktemp -d` directory (removed on exit) with `curl -fL --progress-bar`.
5. Verify SHA-256 (`shasum -a 256`); a mismatch aborts before anything is installed.
6. `ditto -x -k` to unpack; `codesign --verify --deep --strict` and, when `teamId` is set,
   `-R='anchor apple generic and certificate leaf[subject.OU] = "<teamId>"'`.
7. Quit a running copy (`pkill -x rustyvibes`, wait ≤ 3 s); replace the app with `ditto`;
   `xattr -dr com.apple.quarantine` (no-op for curl downloads; clears leftovers from DMG installs).
8. `open` the app unless `--no-open` / `RUSTYVIBES_NO_OPEN=1`; print where it went and what
   happens next (the menu bar keycap, the Input Monitoring prompt).

Messages are short, one per step, with ✓/✗ marks; colour only when stdout is a TTY. Exit code 0
on success, 1 on any failure, with the reason on stderr.

## 7. Page design

**Tokens**: `--bg #000`, `--tile #161617`, `--tile-2 #1d1d1f`, `--text #f5f5f7`,
`--text-2 #86868b`, `--line #424245`, `--link #2997ff`, `--rust #e8592b`,
`--rust-hi #ff8a4c`, `--cream #f2e5c9`, and a headline gradient (cream → `#ff8a4c` → `#e8592b`).
Radii: tiles 28px, pills 980px. Easing `cubic-bezier(.28,.11,.32,1)`. Breakpoints 1068px and
734px (Apple's). Type: system stack; hero name `clamp(56px, 8vw, 96px)`/600; section headline
`clamp(40px, 5.6vw, 64px)`/600, tracking −0.015em; intro paragraphs 21px in `--text-2` with
the key phrase in `--text` (Apple's emphasis pattern); body 17px; fine print 12px. Reveal on
scroll: 32px rise + fade over 700ms, disabled under `prefers-reduced-motion`.

**Sections** (copy is final unless marked):

0. **Local nav** (sticky, 52px, blurred translucent): icon + "Rustyvibes"; links Sounds · Menu
   bar · Performance · Privacy · Install (hidden < 734px); sound toggle (speaker icon, state in
   `localStorage`); "Install" pill that scrolls to `#install`.
1. **Hero** `#overview`: app icon; h1 "Rustyvibes"; gradient tagline "The click you've been
   missing."; intro "**Real mechanical keyboard sounds** for every key you press, in every app.
   A tiny, native menu bar app for your Mac."; the **command pill** (`curl -fsSL
   https://rustyvibes.kunalbagaria.com/install | bash` + Copy button → "Copied"); note "Paste
   into Terminal. Free for macOS 13 or later, Apple silicon and Intel." Below: the 3D keyboard
   stage; the typing line ("Start typing to hear it." → what you type); and a control bar:
   ▶ Play demo · soundpack chip ("Holy Panda · Tactile", opens the pack list) · colourway
   swatches (Cream, Graphite, Silver).
2. **Soundpacks** `#sounds`: "Soundpacks" / "Pick your switch." / "Twenty-one real keyboards,
   from the crisp click of Cherry MX Blue to the deep thock of Topre. **Every pack is trimmed to
   its transient and loudness-matched**, so switching never startles you." Segmented filter All ·
   Linear · Tactile · Clicky; 21 cards (keycap glyph in the pack colour, name, variant chip,
   short character line, Play). Selecting a card sets the page's pack. Fine print: "The web
   preview uses a lighter version of each pack; the app has every key."
3. **Menu bar** `#menu-bar`: "Menu bar" / "Always a click away." / "Rustyvibes lives in your
   menu bar, not your Dock. **Switch soundpacks, set the volume or turn it off in a click.** This
   one works, too: try it." A desktop panel (brand-gradient wallpaper, menu bar with the keycap
   status item and "9:41") with an HTML replica of the real menu: header switch; Soundpack ›
   submenu grouped Linear/Tactile/Clicky with tinted keycaps; Volume slider; Key Release Sounds,
   Natural Variation, Spatial Stereo, Launch at Login; About/Quit (inert). Each control drives
   the page's sound engine.
4. **Details** `#details` (bento): "Details" / "Sounds real. Because it is." Tiles: *No two
   keystrokes alike* (±35 cents, ±1.5 dB; Natural/Robotic toggle + Play) · *Left keys left. Right
   keys right.* (spatial sweep demo; "Best with headphones") · *Hear the upstroke, too.* (release
   sounds) · *Switch outputs. It follows.* (speakers, headphones, AirPods) · *Rests when you do.*
   (releases the audio device after 15 s) · *Native on every Mac.* (universal, macOS 13+) ·
   *There when you log in.* (Launch at Login).
5. **Performance** `#performance`: "Performance" / "Silent when you are." / "Rustyvibes handles
   each keystroke in microseconds, then gets out of the way. **Stop typing and it stops
   completely**: no CPU, no wakeups, no audio device held open." Big stats: 0.0% CPU when you're
   not typing · 0.4% CPU at ten keys a second · 16 MB memory · 1.16 MB universal binary. Small
   stats: ≤ 5 µs per key event · ≤ 8 µs per audio buffer · 2.7 ms output buffer · 0 wakeups at
   rest. Pipeline diagram: key press → listen-only event tap → lock-free queue → real-time mixer →
   Core Audio. Fine print: "Measured on an M2 Max Mac running macOS 26.2 with the release build.
   Output latency also depends on your audio device."
6. **Privacy** `#privacy`: "Privacy" / "It hears keys. Never words." / "Rustyvibes needs to know
   when a key goes down, and that's all it learns. **It never sees what you type, never records
   anything and never touches the network.**" Four points: Keys, not characters · Listen-only
   (Input Monitoring through a listen-only event tap: it can observe key presses, never change
   or block them) · Nothing leaves your Mac (no network, analytics or accounts) · Secure Input
   respected (password fields are hidden from every app, Rustyvibes included).
7. **Install** `#install`: "Get Rustyvibes" / "One line. Then just type." Command pill again;
   steps (Open Terminal → paste and press Return → allow Input Monitoring when asked); "What the
   command does" list (downloads Rustyvibes {version} ({size}) from this site; checks its SHA-256
   and code signature; installs it in Applications without an admin password; opens it) with a
   "Read the script" link to `/install`; Update ("run it again") and Uninstall (`… | bash -s --
   --uninstall`) lines; FAQ (`<details>`): Why Terminal? (signed but not notarized by Apple, so a
   downloaded copy would trigger a Gatekeeper warning; the installer checks the download, then
   installs it without that prompt; macOS still asks before Rustyvibes can hear key presses) ·
   Is it free? · Which Macs?
8. **Tech specs** `#specs`: Version {version} · macOS 13 Ventura or later · Apple silicon or Intel
   (universal) · {size} download, 17.5 MB installed · 21 soundpacks (Linear 9, Tactile 7, Clicky
   5) · 16–17 MB memory · Input Monitoring only · No network access · Free · Sounds by Mechvibes
   (Hai Nguyen) and kbsim (Thomas Lai), MIT License.
9. **Footer**: maker line, sound credits, "Mac, macOS and Apple silicon are trademarks of Apple
   Inc. Rustyvibes is not affiliated with or endorsed by Apple.", © 2026 Kunal Bagaria.

## 8. 3D keyboard

- **Layout** (75% Mac, 16u wide): row 0 esc · F1–F12 (¼u gaps) · del · rotary knob; row 1
  `` ` `` 1–0 - = delete(2u) home; row 2 tab(1.5) Q–P [ ] \(1.5) page up; row 3 caps lock(1.75)
  A–L ; ' return(2.25) page down; row 4 shift(2.25) Z–/ shift(1.75) ↑ end; row 5 control(1.25)
  option(1.25) command(1.25) space(6.25) command option fn ← ↓ →. Each key carries its
  `KeyboardEvent.code`, macOS keycode and legends.
- **Geometry**: keycaps from a rounded box tapered towards the top with a cylindrical dish; one
  base geometry per width, cloned per key with UVs into that key's atlas cell. The case is a
  rounded box with a slight typing angle, plus the knob.
- **Materials**: one `MeshStandardMaterial` for every keycap, mapped to a canvas atlas drawn like
  a top view of the keyboard (each cell filled with the cap colour, with Apple-style legends:
  centred capitals, stacked number symbols, symbol + word modifiers). The case is anodised metal.
  `NeutralToneMapping`, `RoomEnvironment` reflections, warm key light and cool rim light.
- **Colourways**: Cream (caps `#efe6d3`, mods `#d9cdb2`, accents `#e8592b`, legends `#4a4237`,
  case `#2b2c30`) · Graphite (`#3a3c41` / `#2a2c30` / `#e8592b` / `#e8e8ea`, case `#1c1d20`) ·
  Silver (`#f5f5f7` / `#dcdde1` / `#e8592b` / `#3a3a3c`, case `#c8cbd0`). Accents: esc and return.
- **Motion**: per-key spring (down 0.11u on press, slight overshoot on release); pointer parallax
  ±4°/±2° (fine pointers only); the camera tilts with hero scroll progress. Everything runs
  through `requestRender()`, which schedules one frame at a time and stops once springs settle.
- **Input**: tap or click on a key (raycast) presses and releases it.
- **Loading**: a pre-rendered poster (AVIF/WebP/JPEG, same framing) holds the space; the canvas
  fades in once its first frame is drawn. On WebGL failure or context loss the poster stays.

## 9. Browser audio

- `.rvw` file: `"RVW1"`, `u32` JSON length, JSON `{id, sampleRate, clips: [[start, len]…],
  press: {kVK: [first, count]}, release: {…}}`, padding to 2 bytes, then `i16` PCM. Clips are
  capped at 250 ms with a 30 ms fade.
- Graph: per voice `AudioBufferSource` (one `AudioBuffer` per pack; `start(when, offset,
  duration)`) → `Gain` (±1.5 dB) → `StereoPanner` (the app's `pan_for_x`: `((x − 7.25) / 7.75)`
  clamped to ±1, × 0.4, with x the key centre in the app's full-size geometry) → master `Gain`
  (volume², default 0.75) → `WaveShaper` replicating the app's soft-knee limiter (knee 0.8, input
  pre-scaled so ±4 maps onto the curve) → destination. Pitch: `playbackRate = 2^(c/1200)`, c
  uniform in ±35. Maximum 32 voices (oldest stolen).
- The `AudioContext` (`latencyHint: "interactive"`) is created or resumed on the first
  keydown/pointerdown/click. `navigator.audioSession.type = "playback"` (where supported) on an
  explicit Play or tap.
- Packs load lazily: the default at idle after load, others on selection; hover prefetches.
  Decoded packs are kept for the session. A failed fetch shows a retry state.

## 10. Typing rules

- Listen on `document` for `keydown`/`keyup`; ignore `repeat`. Map `event.code` → macOS keycode
  (full table, including keys not drawn) → press clip; animate the drawn key if any.
- Intercept (`preventDefault`) only when nothing focusable has focus, no ⌘/⌃/⌥ is held and the
  stage is ≥ 50% visible: printable keys and Space, so typing doesn't scroll the page or open
  Firefox quick find. Otherwise sound plays and the browser does its usual thing.
- The typing line shows the last ~48 characters of `event.key` text; Backspace deletes; the caret
  blinks only within 5 s of the last key.
- On ⌘ keyup, window blur or page hide, release every held key (macOS browsers drop keyups for
  keys pressed with ⌘ held).
- The sound toggle off: keys still animate, nothing plays.

## 11. Performance budget and verification

Gzip budgets checked by the build: HTML ≤ 25 KB, CSS ≤ 20 KB, main JS ≤ 30 KB, 3D chunk
≤ 180 KB, default pack ≤ 100 KB raw, poster ≤ 160 KB. Verified with Lighthouse (desktop and
mobile) and an idle check: after 2 s without input the page requests no animation frames.

## 12. Accessibility

Semantic landmarks and heading order; visible focus rings; every control is a real button,
switch (`role="switch"`), checkbox or range; the canvas is `role="img"` with a description; the
typing line is `aria-live="off"` (decorative); colour contrast ≥ 4.5:1 for text; reduced motion
disables reveals, parallax and scroll tilt; the copy button announces "Copied" via a polite live
region.

## 13. Testing

- `bun test`: rvpack reader against the real packs; `.rvw` round-trip and the subset rule;
  keycode table vs the app's geometry list; pan/variation/limiter maths vs the app's constants;
  manifest validation; installer rendering (placeholders, escaping) and `bash -n`; Worker routes
  through `app.request()` with in-memory `ASSETS`/`RELEASES` (rewrite, install script, Range,
  304, 404s, degraded R2).
- Installer integration (macOS, needs the built app): serve a real release from a local server;
  install into a temp dir with `--no-open`; assert the signature is intact and quarantine is
  absent; checksum mismatch aborts with nothing installed; `--uninstall` removes it.
- `shellcheck` on the rendered installer.
- Visual QA: Playwright (system Chrome) screenshots at 1440×900, 1024×768 and 390×844; console
  free of errors; Lighthouse; live checks after deploy, including running the published command
  into a temp dir.

## 14. Out of scope

Light theme, localisation, a blog or changelog page, analytics, auto-update inside the app,
Windows/Linux.
