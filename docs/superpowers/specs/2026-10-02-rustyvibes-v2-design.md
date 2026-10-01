# Rustyvibes 2 — Design Spec

Date: 2026-10-02 · Status: approved (executive decisions delegated by the owner, who is AFK)

## 1. Intent

**What the owner asked for**

- Build version 2 of `rustyvibes` from a clean slate.
- Keep the *core idea* — mechanical-keyboard sounds on every key press, system-wide — but
  everything else (mechanics, specifics, soundpacks) may be completely new.
- A hyper-optimized, super-efficient **GUI app written in Rust**, packaged as a **macOS app** that
  can be shipped directly.
- All soundpacks bundled inside the app.
- Claude makes every decision; the owner wants to come back to a finished project.

**Assumptions made (owner can override later)**

- macOS only (Apple Silicon first, Intel supported through a universal binary). v1 was cross-platform;
  v2 trades that for native macOS integration and efficiency.
- "GUI app" = a native menu bar app (the natural shape for an always-on utility), plus a
  permission/onboarding window. No Dock icon.
- Minimum macOS 13 Ventura (needed for `SMAppService` launch-at-login; covers every Mac sold since 2017
  that still gets updates).
- No user-imported packs in 2.0 (curated bundle only). The pack format and converter are designed so
  import can be added later.

## 2. Goals and non-goals

Goals

1. Feels instant: key-to-sound latency dominated by the audio device, not by Rustyvibes.
2. Costs nothing when idle: 0% CPU, no timers, no wakeups, audio device released, no sleep prevention.
3. Small: tiny binary, small resident memory, no runtime decoders, no web view, no GPU context.
4. Native: AppKit menu bar UI that looks and behaves like a first-party macOS utility (dark mode,
   accessibility, Liquid Glass on macOS 26 for free).
5. Great sound: 21 curated soundpacks, cleaned up at build time (latency trimmed, loudness matched),
   with release sounds, natural variation and spatial stereo.
6. Shippable: one command produces a signed, universal `Rustyvibes.app` and a `.dmg`.

Non-goals (2.0)

- Windows/Linux support.
- Importing arbitrary Mechvibes packs at runtime.
- Mouse-click sounds, per-app rules, auto-update, analytics, network access of any kind.

## 3. Success criteria (measured, reported in README)

| Metric | Target |
|---|---|
| Idle CPU (sounds on, not typing) | 0.0% |
| Idle wakeups | 0/s once the audio idle timeout has passed |
| Physical memory footprint | < 25 MB |
| Release binary (universal, stripped) | < 3 MB |
| Event-to-voice processing in the tap callback | < 20 µs |
| Audio buffer | 128 frames requested (≈2.7 ms @ 48 kHz) |
| Leading silence in any bundled clip | ≤ 1 ms |
| Loudness spread across packs (median clip) | ≤ ±1.5 dB |

## 4. User experience

### 4.1 Menu bar menu (native `NSMenu`)

```
┌───────────────────────────────────────┐
│  Rustyvibes                  [ ●═══ ] │  header: bold title + NSSwitch (sounds on/off)
├───────────────────────────────────────┤
│  ⚠ Allow Keyboard Access…             │  only when Input Monitoring is missing
├───────────────────────────────────────┤
│  Soundpack                            │  section header
│  ▣ Cherry MX Brown · PBT           ▸  │  current pack → submenu grouped Linear/Tactile/Clicky
├───────────────────────────────────────┤
│  Volume                               │
│  🔈 ━━━━━━━━━━●━━━━━━━━ 🔊            │  NSSlider custom view, live preview click
├───────────────────────────────────────┤
│  Key Release Sounds              ✓    │  disabled when the pack has no release sounds
│  Natural Variation               ✓    │  ±35 cents pitch, ±1.5 dB gain per keystroke
│  Spatial Stereo                  ✓    │  pans each key by its position on the keyboard
│  Launch at Login                      │  SMAppService.mainApp
├───────────────────────────────────────┤
│  About Rustyvibes                     │
│  Quit Rustyvibes                 ⌘Q   │
└───────────────────────────────────────┘
```

- Pack submenu: three section headers (Linear, Tactile, Clicky), each pack row has a small
  keycap glyph tinted with the switch's stem colour, and a plain variant suffix (e.g.
  "Cherry MX Brown · PBT"; coloured attributed titles are avoided because AppKit does not invert
  them on highlight).
  Selecting a pack checks it, persists it, and plays a short typing flourish (≈0.8 s) as a preview.
- Volume slider plays a single keystroke at the new level while dragging (throttled to ≤ 8/s).
- When sounds are off the status item icon renders dimmed (`appearsDisabled`).
- Previews only play when sounds are on.

### 4.2 Permission onboarding window

Shown at launch when Input Monitoring is not granted (and from the ⚠ menu item).
Explains why permission is needed, states privacy guarantees (only key positions are observed,
nothing is recorded, no network), and offers **Open System Settings**. The button calls
`CGRequestListenEventAccess()` (system prompt on first use) and opens the Input Monitoring pane.
While access is missing (window open or not) the app polls `CGPreflightListenEventAccess()` once per
second; when access appears it starts the event tap, shows a success state and closes the window if it
is open. If the tap still cannot be created, it offers **Relaunch Rustyvibes**. No polling happens once
access is granted.

### 4.3 About panel

Standard AppKit About panel with version, copyright and credits for every soundpack source
(Mechvibes — MIT © 2021 Hai Nguyen; kbsim — MIT © Thomas Lai).

## 5. Architecture

```
            ┌──────────────────────── main thread (AppKit) ────────────────────────┐
            │ AppDelegate · StatusItem/NSMenu · Onboarding window · NSUserDefaults │
            │      writes Settings atomics · pushes preview commands (UI ring)      │
            └───────────────┬───────────────────────────────────────┬───────────────┘
                            │ atomics (enabled, volume, pack, ...)  │ SPSC ring (UI)
┌── event-tap thread ──┐    ▼                                       ▼
│ CFRunLoop + listen-  │  Engine (shared, 'static)          ┌─ CoreAudio IO thread ─┐
│ only CGEventTap      ├─ SPSC ring (input) ───────────────►│ render callback:      │
│ keycode→clip lookup, │                                    │ drain rings → voices, │
│ variation, panning   │──unpark (only if audio stopped)──┐ │ cubic resample + mix, │
└──────────────────────┘                                  │ │ master gain, limiter  │
                                                          ▼ └──────────▲────────────┘
                                             ┌─ audio-control thread ─┐│ AudioOutputUnit
                                             │ start/stop AUHAL;      ├┘ Start/Stop
                                             │ stop after 8 s silence │
                                             └────────────────────────┘
```

### 5.1 Crates (Cargo workspace)

| Crate | Kind | Purpose | Dependencies |
|---|---|---|---|
| `rvpack` | lib | `.rvpack` format (reader + writer), macOS keycode tables (names, rows, x-positions), Mechvibes→macOS keycode map | none |
| `rustyvibes` | bin | The app: UI, input, audio engine | `rvpack`, `objc2`, `objc2-foundation`, `objc2-app-kit`, `block2`, `libc` |
| `xtask` | bin | Build tooling: soundpack conversion, `.app` bundling, signing, DMG | `rvpack`, `symphonia`, `serde_json` |

The runtime app contains **no audio decoder**: decoding happens at build time in `xtask`.

### 5.2 `.rvpack` format (version 1, little-endian)

```
0    magic "RVPK"            4 bytes
4    version u16 = 1
6    flags   u16             bit0 = has release sounds
8    sample_rate u32         Hz (mono)
12   clip_count u32
16   pcm_offset u32          byte offset of PCM, 16-byte aligned
20   pcm_samples u32         number of i16 samples in the PCM block
24   meta_offset u32
28   meta_len u32            UTF-8 "key=value\n" lines (id, name, variant, category, color, credit)
32   press   [ClipRef; 128]  ClipRef = { first: u16, count: u16 }, count 0 = silent
544  release [ClipRef; 128]
1056 clips   [Clip; clip_count]   Clip = { start: u32, len: u32 } in samples
...  meta
...  PCM i16 mono; every clip has ≥ 2 zero guard samples before and ≥ 3 after
```

- Indexed directly by macOS virtual keycode (0–127): lookup is two array reads, no hashing.
- `count > 1` means "pick one at random, avoiding immediate repeats" (supported, used where a source
  provides variants).
- Guard samples let the cubic interpolator read `i-1 … i+2` without bounds checks.
- The app `mmap`s every pack read-only at startup (only headers are touched). The active pack's
  PCM is prefaulted and `mlock`ed (best effort) so the audio thread never page-faults. Packs are
  never unmapped, so clip slices are `&'static [i16]` and pack switching needs no synchronisation
  beyond one atomic.

### 5.3 Soundpack conversion (`cargo xtask packs`)

Input: `assets/soundpacks/catalog.json` + original source files (unchanged, with their licences).
Supported source layouts: Mechvibes `single` (one sprite + `[start_ms, dur_ms]` per key),
Mechvibes `multiple` (file per key), kbsim (press/release folders, per-row generic sounds).

Pipeline per clip:

1. Decode (symphonia: Vorbis, MP3, WAV) → f32, downmix to mono (all sources are measured mono:
   L/R correlation 1.000).
2. Slice (sprites) and DC-block (one-pole high-pass ≈ 20 Hz, applied to whole source before slicing).
   Slices are capped at 1.5× the pack's median slice length, which neutralises an upstream config typo
   (Cherry MX Red PBT defines F6 as 1194 ms, which would play several neighbouring keys). Mechvibes
   slices keep the release click recorded ~70–150 ms after the press; that is how those packs sound.
3. Onset trim: first sample above max(−55 dBFS, peak − 45 dB), keep 0.5 ms pre-roll with a linear
   fade-in. (Measured: original sprites have 0–54 ms of leading silence; Mechvibes-split copies had up
   to 283 ms.)
4. Tail trim: last sample above peak − 50 dB, plus a 6 ms raised-cosine fade-out.
5. Loudness: per pack, the median of per-clip RMS over a 60 ms window from onset is normalised to a
   common target; any clip whose peak would exceed −1 dBFS is attenuated individually.
6. Quantise to i16 with TPDF dither; dedupe identical slices; write `.rvpack`.

Key mapping:

- Mechvibes packs: each macOS keycode maps to its libuiohook code; if the pack lacks it, a fallback
  chain picks a similar key (right modifier → left, numpad digit → top-row digit, F13+ → F12, else a
  same-row letter).
- kbsim packs: Space/Return/Delete(Backspace) use their dedicated recordings, keypad Enter uses Return;
  every other key uses the generic recording for its keyboard row (`GENERIC_R0` function row …
  `GENERIC_R4` bottom rows), exactly as kbsim intends. Release: dedicated for Space/Return/Backspace,
  `GENERIC` otherwise.

### 5.4 Input capture

- Listen-only `CGEventTap` (`kCGSessionEventTap`, head insert) for `keyDown`, `keyUp`, `flagsChanged`,
  on a dedicated thread with its own `CFRunLoop` and `QOS_CLASS_USER_INTERACTIVE`.
- Requires only Input Monitoring (not Accessibility).
- Auto-repeat events (`kCGKeyboardEventAutorepeat`) are ignored; a 128-bit pressed set prevents
  double triggers.
- Modifiers: press vs. release decided from device-dependent flag bits (left/right Shift, Control,
  Option, Command), `SecondaryFn` for Fn; toggle fallback when device bits are absent; Caps Lock plays
  a press on every change.
- Re-enables the tap on `kCGEventTapDisabledByTimeout` / `…ByUserInput`.
- Callback work: two table reads, a xorshift RNG step, a few multiplies, one ring push. No allocation,
  no locks, no Objective-C.

### 5.5 Audio engine

- Output: AUHAL `kAudioUnitSubType_DefaultOutput` (follows the system default device automatically),
  f32 non-interleaved stereo at the device's nominal rate (re-queried on every start), 128-frame
  buffer requested.
- Commands: two lock-free SPSC rings (input thread, UI thread), 256 entries each. A command carries
  the clip slice, source rate, pitch factor, left/right gains and a start delay in frames (used by
  previews).
- Mixer: 32 voices, oldest voice stolen when full. Per voice, 32.32 fixed-point position, 4-point
  cubic Hermite interpolation (handles 44.1 kHz → device rate and pitch variation). Master gain
  ramps per block (no zipper noise). A soft-knee limiter above −1.9 dBFS keeps overlapping voices
  from clipping.
- Silence: when no voices are active the callback zero-fills and sets
  `kAudioUnitRenderAction_OutputIsSilence`.
- Lifecycle: the unit starts on the first command after idle and stops after 8 s of silence via a
  dedicated control thread (parked, zero CPU, while stopped). Stopping releases the device, so
  Rustyvibes never keeps the Mac awake.
- Real-time safety: the render callback never allocates, locks, logs or calls Objective-C.

### 5.6 Settings

`NSUserDefaults` keys: `Enabled` (true), `Pack` (`cherry-mx-brown-pbt`), `Volume` (0.6),
`ReleaseSounds` (true), `Variation` (true), `Spatial` (true). Values are mirrored into atomics read by
the input thread. Launch at login is never stored — it is read from `SMAppService`.

Volume curve: gain = slider². Spatial: equal-power pan, keyboard x-position mapped to ±0.45 pan width,
normalised so a centred key keeps unity gain.

## 6. Soundpack catalog (21 packs)

| Category | Pack (variant) | Source |
|---|---|---|
| Linear | Cherry MX Black (ABS), Cherry MX Black (PBT), Cherry MX Red (ABS), Cherry MX Red (PBT) | Mechvibes |
| Linear | Gateron Black Ink, Gateron Red Ink, NovelKeys Cream, Alpaca, Turquoise Tealios | kbsim |
| Tactile | Cherry MX Brown (ABS), Cherry MX Brown (PBT), Topre Purple Hybrid (PBT), Everglide Crystal Purple, Everglide Oreo | Mechvibes |
| Tactile | Holy Panda, Topre | kbsim |
| Clicky | Cherry MX Blue (ABS), Cherry MX Blue (PBT) | Mechvibes |
| Clicky | Kailh Box Navy, SKCM Blue Alps, Buckling Spring | kbsim |

Sources (MIT): `hainguyents13/mechvibes` @ `326252a`, `tplai/kbsim` @ `ba103f3`. Redundant duplicates
(kbsim's Cherry MX recordings, Mechvibes' copies of kbsim packs, and the re-encoded "NK Cream") are
left out so each switch appears once (Cherry MX keeps both keycap materials).

## 7. Packaging

`cargo xtask bundle`:

1. Builds `rustyvibes` for `aarch64-apple-darwin` and `x86_64-apple-darwin`
   (`MACOSX_DEPLOYMENT_TARGET=13.0`, LTO, `codegen-units=1`, `panic=abort`, stripped) and `lipo`s them.
2. Converts the soundpacks (cached by input hash).
3. Assembles `target/bundle/Rustyvibes.app` (`Info.plist` with `LSUIElement`, icon, packs).
4. Signs with the first "Developer ID Application" identity in the keychain (hardened runtime,
   secure timestamp), falling back to ad-hoc. A stable Developer ID signature also keeps the
   Input Monitoring grant across rebuilds.

`cargo xtask dmg` wraps the app in a compressed DMG with an `/Applications` link and signs it.
Notarisation is documented (one `notarytool` command) but not run, since it uploads to Apple with the
owner's account.

Bundle id `io.github.kb24x7.rustyvibes`, version 2.0.0.

## 8. Error handling

| Situation | Behaviour |
|---|---|
| No Input Monitoring | Onboarding window + ⚠ menu item; menu otherwise fully usable |
| Tap creation fails after grant | Offer relaunch |
| Tap disabled by system | Re-enabled immediately inside the callback |
| No packs found / pack corrupt | Corrupt packs are skipped (logged); menu shows "No soundpacks found" if none remain |
| Audio unit fails to start | Logged; retried on the next keystroke; never crashes |
| Saved pack id no longer exists | Falls back to the default pack |

Logging: `RUSTYVIBES_LOG=1` prints to stderr; silent otherwise.

## 9. Testing and verification

- `rvpack`: writer→reader round trip, header/bounds validation against corrupted input, keymap
  completeness (every ANSI key has a row, position and Mechvibes mapping).
- `xtask`: onset/tail trimming, fades, loudness normalisation and peak ceiling on synthetic signals;
  kbsim row mapping; Mechvibes fallback chain; end-to-end conversion of the real catalog with
  invariants (leading silence ≤ 1 ms, peaks ≤ −1 dBFS, every printable key mapped).
- Engine: SPSC ring (single and multi-threaded stress), mixer exactness at rate 1.0, interpolation
  continuity, delay scheduling, voice stealing, limiter bounds, idle detection.
- Live checks (from a terminal that already has Input Monitoring): run the binary, inject harmless
  Shift key events with `CGEventPost`, verify the tap → engine → audio path via counters in the log,
  measure CPU/memory/wakeups with `ps`, `top` and `footprint`.
- UI: a `--snapshot <dir>` diagnostic renders custom views, icons and the onboarding window to PNG
  for visual review (no screen-recording permission needed).
- `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean.

## 10. Risks and mitigations

| Risk | Mitigation |
|---|---|
| objc2 API details differ from memory | Read the generated bindings in the cargo registry before use; compile early and often |
| AUHAL start latency on first keystroke after idle | Measure; 8 s idle timeout keeps a typing session warm |
| TCC identity changes on every ad-hoc rebuild | Prefer Developer ID signing (available on this machine) |
| Cannot listen to audio | Objective metrics (onset, loudness, peaks, spectra) checked in tests; sources are known-good recordings |
| No screen recording permission for visual QA | Self-rendered snapshots via `cacheDisplayInRect:` |
| Liquid Glass / SDK-version gated AppKit behaviour | Verify `LC_BUILD_VERSION` SDK field; pass `-platform_version` if needed |

## 11. Revisions during implementation (2026-10-02)

Measurements during implementation changed these values; the implementation follows this section
where it differs from the text above.

| Item | Was | Now | Why |
|---|---|---|---|
| Loudness target (attack-window RMS) | −22 dBFS | −30 dBFS | At −22 the −1 dBFS ceiling limited up to 70% of a pack's clips and medians spread 3 dB; at −30 the spread is 0.0 dB and 2.6% of clips are limited |
| Default volume | 0.6 | 0.75 | Compensates the quieter packs: typical output peaks ≈ −14 dBFS |
| Audio idle stop | 8 s | 15 s | A USB interface measured ~50 ms to start IO from cold; a longer window keeps typing pauses responsive |
| Output unit lifecycle | stopped when idle | created and initialised at launch; stopped **and uninitialised** when idle | First sound after launch 135 → 22 ms; idle wakeups → 0/s; restart latency unchanged |
| Device buffer size | set on every start | set only when it differs | Setting it reconfigured the device (+40 ms per start) |
| `input::start` error type | `Voicer` | `Box<Voicer>` | `clippy::result_large_err` (824-byte variant) |
| Sprite slices | as defined | capped at 1.5× the pack's median length | Upstream config typo (F6 = 1194 ms in Cherry MX Red PBT) |
| Logging | `eprintln!` | error-ignoring writer with timestamps | A closed stderr crashed the app |
| objc2 features | crate defaults | only the features in use | Drops six unneeded framework links and seven runtime crates |
