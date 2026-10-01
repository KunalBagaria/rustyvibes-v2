# Soundpack sources

Every bundled soundpack is converted at build time (`cargo xtask packs`) from
the original recordings in this directory. The files here are unmodified copies.

| Directory | Upstream | Commit | Licence |
|---|---|---|---|
| `mechvibes/` | https://github.com/hainguyents13/mechvibes (`src/audio`) | `326252a13e7bef4f1c35d08ef0189b5af6f8ba02` | MIT © 2021 Hai Nguyen (`mechvibes/LICENSE`) |
| `kbsim/` | https://github.com/tplai/kbsim (`src/assets/audio`) | `ba103f3b0afa9dab80447aa2e7e2ed80b6bd80e4` | MIT © Thomas Lai (`kbsim/LICENSE.md`) |

Mechvibes packs are per-key recordings stored as one sprite file sliced by
`[start_ms, duration_ms]` (`key_define_type: "single"`). kbsim packs record a
press and a release for each keyboard row (`GENERIC_R0`–`R4`) plus Space,
Enter and Backspace.

Left out on purpose: kbsim's Cherry MX Black/Blue/Brown (Mechvibes has
keycap-specific recordings of the same switches), Mechvibes' copies of kbsim
packs (`*-travel`, `holy-pandas`, `turquoise`), and Mechvibes' `nk-cream`
(kbsim's NovelKeys Cream is used instead).
