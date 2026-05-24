# Changelog

All notable changes to `batty` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/).

While at `0.x`:
- `0.x.y → 0.(x+1).0` for any new feature **or** breaking change.
- `0.x.y → 0.x.(y+1)` for bug fixes only.

## [Unreleased]

## [0.11.0] — 2026-05-24

### Changed

- `--wrap=auto` (the default) is now TTY-aware: when stdout is not a
  terminal (piped to a file or another command), batty no longer inserts
  line breaks or continuation prefixes. Matches
  [`bat`](https://github.com/sharkdp/bat)'s long-standing behavior.
  Explicit `--wrap=character` continues to wrap regardless of stdout
  type; `--wrap=never` is unchanged.
- Rhai single-quoted character literals now carry the more specific
  TextMate scope `string.quoted.single.char.rhai` (was
  `string.quoted.single.rhai`). Prefix matching means every existing
  theme rule that styled `string.quoted.single` still applies — no
  visible change in the bundled themes — but custom themes can now
  specialize on char literals.

## [0.10.1] — 2026-05-22

### Added

- Crate-root `#![doc = include_str!("../README.md")]` so [docs.rs](https://docs.rs/batty-cat)
  renders the full README on the crate landing page instead of a bare module
  list. No behavior change to the binary.

## [0.10.0] — 2026-05-19

### Added

- Colorized `--examples` flag: curated, copy-pasteable usage scenarios for
  every common flag. Mirrors the pattern used in
  [`codedeviate/recon`](https://github.com/codedeviate/recon) for
  cross-repo uniformity. Honors `NO_COLOR` and TTY detection; short-circuits
  before pager / file validation so it works without any arguments.
- Hand-written man page at `man/batty.1` covering every CLI flag, the
  interactive keybindings, the config schema, and the environment
  variables. Install instructions added to the README.
- CLAUDE.md conventions requiring both `man/batty.1` and `src/examples.rs`
  to stay in sync with `src/cli.rs` on every change.

## [0.9.1] — 2026-05-17

### Changed

- Published to crates.io as **`batty-cat`** (the `batty` name was taken).
  The binary, repo, and Homebrew formula all remain `batty`; only the
  crate name on crates.io differs.
- Added shields.io badge header (GitHub, latest release, crates.io,
  Homebrew tap, Rust edition / MSRV, license) to README.md for cross-repo
  uniformity.

## [0.9.0] — Earlier

### Added

- `--encoding` flag (also `encoding = "..."` in config) selecting how
  file/stdin bytes are decoded. Values: `auto` (default; tries UTF-8,
  falls back to ISO-8859-1 on decode failure), `utf-8` (strict — errors
  with a hint on invalid sequences), `iso-8859-1` / `latin1` (every byte
  `0x00`–`0xFF` → `U+0000`–`U+00FF`). Decoder is dependency-free.

### Changed

- Reads are now always done as bytes via `fs::read` + `input::decode`,
  not `fs::read_to_string`.

### Behavior change

- Default `auto` encoding means older files that happened to be Latin-1
  now open silently instead of erroring with a UTF-8 decode error.

## [0.8.0] — Earlier

### Added

- Markdown gutter: source-line numbers (and optional grid bar) next to
  rendered blocks. New `markdown::render_with_gutter` reuses the 0.7.0
  source map — each block's first rendered row carries its source line;
  continuation rows are blank in the number column with the grid bar
  repeating. Static path honors `--style=numbers,grid` (and `--no-gutter`
  strips). Interactive markdown view honors the live `n` toggle.

### Fixed

- Row-counting bug in `render_with_map` that over-counted rows when a
  block-render ended in `\n`, miscalculating later block-start indices.

### Notes

- Diff markers and the `▶` cursor glyph remain absent in markdown view
  (block-granular mapping doesn't make them meaningful).

## [0.7.1] — Earlier

### Fixed

- Markdown rendering truncated documents containing inline tags
  (`**bold**`, `*emph*`, `` `code` ``). The block-walker was decrementing
  depth on every `End` event including inline ones but only incrementing
  on block-level `Start`s, so inline tags would unbalance the counter
  and prematurely close the outer paragraph. Regression test added.

## [0.7.0] — Earlier

### Added

- Source-line ↔ rendered-line mapping in markdown view (per-block
  rendering via `pulldown-cmark` + `termimad`), so `m`-toggle in
  interactive mode preserves scroll position both directions. Status bar
  shows `rendered N/M ↔ src K`.
- New `--gutter` / `--no-gutter` flag and matching `n` key in interactive
  mode to live-toggle the gutter (line numbers + cursor glyph).
- Added `pulldown-cmark` as a direct dep (already transitive via
  `termimad`).

## [0.6.0] — Earlier

### Added

- `--markdown-on-extension` (and `markdown-on-extension = true` config
  key): renders `.md` / `.markdown` / `.mdown` / `.mkd` files as
  markdown automatically while leaving source files raw. Precedence:
  `--no-markdown` > `--markdown` > `--markdown-on-extension` > default.

## [0.5.2] — Earlier

### Added

- Live `+` / `-` keys in interactive mode adjust `--top-pad` on the fly.
  Useful when a terminal (Warp) overlays UI on the alt-screen's top rows
  and the right pad value varies tab-to-tab or after pane resizes.
  Status bar shows `pad=N` when nonzero.

## [0.5.1] — Earlier

### Changed

- `--help` flags are now alphabetical by long-flag name. `Cli` struct in
  `src/cli.rs` reordered to match; convention captured in CLAUDE.md.

## [0.5.0] — Earlier

### Added

- `--wrap` actually wraps. `character` / `auto` break long lines at the
  terminal-width boundary with a continuation prefix that keeps the
  gutter intact.
- ANSI escape sequences tracked across breaks so colors / `INVERT`
  resume on each continuation.
- Wide CJK / emoji chars count via `unicode-width`.
- Forced to `never` in interactive mode (viewport math assumes 1 source
  line = 1 visual row).

### Behavior change

- Users on the default `--wrap=auto` now get proper wrapping (with
  gutter) rather than terminal-level overflow.

## [0.4.1] — Earlier

### Added

- Rhai grammar polish: backtick template strings with `${expr}`
  interpolation, `#{}` map-literal prefix, `??` / `?.` operators, `::`
  module accessor, leading-dot floats, expanded builtin list (`Fn`,
  `call`, `curry`, `is_def_var`, `is_def_fn`, `is_shared`, `eval`,
  `parse_int`, `parse_float`, `to_int`, `to_float`, `to_blob`,
  `to_array`, `to_map`). Function-name scope no longer includes
  trailing whitespace.

## [0.4.0] — Earlier

### Added

- `--follow` / `-f` tail mode (`tail -f` semantics with highlighting).
- `--tail-lines` for count, `--no-follow` opt-out, `follow = true`
  config key.

### Changed

- `rule` and `snip` style components now functional (previously parsed
  but inert).
- `expand_tabs` honors Unicode column width.
- Rhai grammar gains range / bitwise / nested-block-comment support.
- Internal cleanup: dropped `Cli::parse_from_args`, `StyleFlags::any` is
  `cfg(test)`-gated, `term_width` uses `crossterm::terminal::size`,
  SIGPIPE reset on Unix.

## [0.3.1] — Earlier

### Fixed

- Scrolling works in interactive markdown view (j/k/g/G/Ctrl-d/u/PgUp/
  PgDn). Independent rendered-row scroll counter; status bar shows
  `rendered N/M`.

### Added

- `OUT-OF-SCOPE.md`.

## [0.3.0] — Earlier

### Added

- `--markdown` / `-m` renders Markdown to terminal escapes (via
  `termimad`).
- `--no-markdown` opt-out.
- `markdown = true` config key.
- `m` key toggles raw ↔ rendered in interactive mode (status bar shows
  `[md]`).

## [0.2.3] — Earlier

### Fixed

- `--paging=never` also disables interactive mode (treats `--paging` as
  a global flat-output signal).

## [0.2.2] — Earlier

### Fixed

- `--no-interactive` shows in short help (`-h`), not just long help.

## [0.2.1] — Earlier

### Fixed

- Duplicate-flag conflict when config + CLI overlap.

### Added

- `--no-interactive` to override config.
- `--list-*` short-circuits before interactive.
- `BATTY_CONFIG_PATH` env var.

## [0.2.0] — Earlier

### Added

- Interactive TUI (`-i`).
- Relative line numbers.
- `--top-pad`.
- **TOML config** at `~/.config/batty/config.toml`.

### Breaking

- TOML config replaces the previous line-based config format.

## [0.1.0] — Earlier

### Added

- Initial release: full `bat`-parity (highlighting, git diff, pager,
  config, themes), bundled Rhai grammar, 36 tests, 2.5 MB binary.

[Unreleased]: https://github.com/codedeviate/batty/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/codedeviate/batty/compare/v0.10.1...v0.11.0
[0.10.1]: https://github.com/codedeviate/batty/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/codedeviate/batty/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/codedeviate/batty/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/codedeviate/batty/releases/tag/v0.9.0
