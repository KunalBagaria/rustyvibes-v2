# Changelog

## 2.0.0 — 2026-10-02

A rewrite: Rustyvibes is now a native macOS menu bar app instead of a command-line tool.

### Added
- Menu bar app with an on/off switch, soundpack picker (with a short preview), volume slider,
  key release sounds, natural variation, spatial stereo and launch at login.
- 21 bundled soundpacks (Mechvibes and kbsim, both MIT), cleaned up at build time: trimmed to
  their transients, loudness-matched and mapped to every macOS key.
- A guided Input Monitoring permission window.
- A new audio engine: on-demand CoreAudio output that follows the system output device, a
  real-time voice mixer with interpolation, and an idle stop that releases the audio device.
- Signed universal (Apple Silicon + Intel) app bundle and DMG via `cargo xtask bundle` / `dmg`,
  and `cargo xtask notarize` to notarise and staple both.
- Diagnostics: `--selftest`, `--tap-test`, `--bench`, `--snapshot`, `RUSTYVIBES_LOG=1`.
- A website, [rustyvibes.kunalbagaria.com](https://rustyvibes.kunalbagaria.com), with a live 3D
  keyboard that plays the real soundpacks, and a one-line installer
  (`curl -fsSL https://rustyvibes.kunalbagaria.com/install | bash`) that checks the download's
  checksum and signature.

### Changed from 1.x
- Soundpacks are bundled; the `<soundpack_path>` argument and `-v/--volume` flag are gone
  (volume lives in the menu).
- Key handling is a listen-only event tap (Input Monitoring only), with auto-repeat ignored and
  modifier keys handled properly.

### Removed
- Linux and Windows support. Rustyvibes 2 is macOS 13+ only.
