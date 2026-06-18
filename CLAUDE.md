# CLAUDE.md — batty

Project context for future Claude sessions. Keep concise; expand only when something is non-obvious.

## Pickup notes (battle-testing handoff)

**Last touched:** 0.14.0 — feat: live `w` toggle that soft-wraps long lines in interactive/live mode (Approach A — source-line-anchored viewport, render-then-clip to `body_rows` visual rows). The viewport (`cursor`, `viewport_top`) stays source-line indexed; when wrap is on, `render_frame` renders from `viewport_top` with the printer's `Character`/`Word` wrap and clips the buffer to `body_rows` *visual* rows, so the top row stays a numbered source line and continuation rows keep a blank gutter. Initial state keys off the **raw** `--wrap` (`character`/`word` start on; `auto`/`never` start off = the 0.13.2 truncate path, untouched). New pieces: `printer::visual_row_count` (counts wrapped rows by reusing `wrap_with_continuation` so it can't drift), `interactive::scroll_viewport_wrapped` + `step_by_rows` (visual-row-aware scroll/paging), a `rows_of_line` helper, and a `wrap` status-bar tag. The wrap-on clip also runs each row through `truncate_to_visible_width(term_w)` as a belt-and-suspenders clamp for the `body_width == 0` (gutter wider than terminal) case where the printer can't wrap. Released across all four surfaces (tag `v0.14.0`, GitHub release, crates.io `batty-cat 0.14.0`, Homebrew formula bumped + `brew install` verified to build & report 0.14.0).

**State at handoff:**
- Branch `main`, all commits pushed to `origin` (`git@github-codedv8:codedeviate/batty.git` — SSH alias carries the `codedeviate` identity; repo is under `~/Development/Thomas/` so gh/push want that account).
- 150 tests passing (`cargo test`).
- Release binary ~3 MB (`cargo build --release`); two tolerated dead-code warnings (`markdown.rs` `render_to_string` + `RenderedMarkdownWithGutter.gutter_width`).
- No untracked content other than `.claude/` (user-local). Note: `CLAUDE.md` itself carries local uncommitted pickup-note edits.

**Most likely sources of issues during battle-testing** (prioritized by recency / surface area):

0. **Interactive/live soft-wrap (0.14.0) + long-line truncation (0.13.2).** The viewport math assumes 1 source line = 1 visual row in the WRAP-OFF path; `render_frame` enforces it by clipping each body line to `term_w` via `printer::truncate_to_visible_width` (`wrap=Never` does NOT truncate, so this clip is load-bearing). In the WRAP-ON path the model changes: lines wrap and the buffer is clipped to `body_rows` *visual* rows, with scroll/paging made visual-aware via `scroll_viewport_wrapped`/`step_by_rows`. Suspects if the top lines ever scroll off again: (a) wrap-off — the `term_w` clip or the value it's given; (b) wrap-on — the `body_width`/gutter geometry in `run()` (`line_no_width + 1 + 2` when `gutter_visible`) must stay identical to what `render_frame` computes, or `visual_row_count` will miscount and `render_end`/clip will drift. Edge cases to exercise: wide CJK/emoji near the right edge; cursor on a line taller than the whole screen (clamp = top=cursor); gutter toggled off mid-session (changes `body_width`); a sub-7-column terminal (`body_width == 0`, exercises the wrap-on truncate clamp); and the markdown view (rows are pre-wrapped to width, `w` is a no-op there).

1. **Markdown gutter (0.8.0).** Block-walker uses pulldown_cmark's `into_offset_iter`. Edge cases I haven't exercised: deeply nested lists, tables (especially with multi-line cells), HTML blocks, footnote definitions, definition lists, setext headings (`Title\n====`), code blocks containing markdown-y syntax. Also: very long files where `line_no_width` exceeds 4 (gutter widens) and resizes mid-render. If a doc looks truncated, the depth-tracking bug could be back — see commit `cc22d6f` for the inline-tag pattern. If line numbers in the gutter don't line up with block starts, the row-counting fix from 0.8.0 may need extending — check `render_with_map`'s row_count formula and verify against `chunk_out`'s actual `\n` shape.

2. **Warp + interactive `--top-pad`.** Static is stable; interactive uses crossterm's alt-screen, and Warp's UI overlay shifts depending on tab age, divider position, etc. Live `+`/`-` keys are the user's escape hatch. If a different terminal exhibits a similar issue, check `TERM_PROGRAM` and consider an auto-detect.

3. **`--follow` long-running stability.** Polled at 200 ms. Highlighter is rebuilt per poll, so multi-line constructs spanning a poll boundary may briefly miscolor. Truncation is detected by size shrink — file rotation that *replaces* the inode (mv + new file with same name) may not be picked up cleanly.

4. **Large files.** No streaming; we read the whole file into memory. Reasonable up to ~50 MB. Beyond that the `cargo build --release` profile (`opt-level="z"`) may show its trade-off.

5. **Color themes.** Default is `Monokai Extended`. If user picks a theme that defines INVERT or DIM differently, the gutter / cursor highlight / wrap-continuation logic in `printer.rs::wrap_with_continuation` may surprise — it only tracks the most recent FG escape plus a `persistent_sgr` (INVERT). Other SGR attributes (BOLD, UNDERLINE) won't survive wrap continuations; usually fine but worth knowing.

6. **Multi-file invocations.** `--style=rule` separator is ungated by tests beyond a 2-file smoke. `batty src/*.rs` should work but may turn up oddities.

**Debug recipes:**

```bash
# Capture a render exactly as the user sees it
BATTY_CONFIG_PATH=/dev/null ./target/release/batty -m --color=always README.md > /tmp/render.txt
# Inspect ANSI sequences
od -c /tmp/render.txt | head -20
# Strip ANSI for content survival check
sed $'s/\x1b\\[[0-9;]*m//g' /tmp/render.txt | head -20
```

To bypass the user's config and run hermetically: `BATTY_CONFIG_PATH=/dev/null`.

**Where ongoing artifacts live:**
- Specs / plans / reports: `~/Development/Starweb/superpowers/batty/{specs,plans,reports}/`
- This file (CLAUDE.md): local only, gitignored per user's global `~/.gitignore_global`.

**Most-near-shippable items in OUT-OF-SCOPE.md** (if user requests an improvement and you need a starting point):
- Word-boundary wrapping (`--wrap=word`) — ~150 LoC.
- `--wrap=auto` TTY-awareness (skip wrap when piped) — ~10 LoC.
- `--diff-context` filtering (only show changed regions ± N) — ~100 LoC.
- Custom theme file loading — depends on syntect's API surface.
- Sub-paragraph row precision in markdown gutter — requires custom markdown renderer (~800-1500 LoC; covered in OUT-OF-SCOPE.md).

**Don't break this list lives below in `## Don't`. Re-read it before structural changes.**

## What this is

`batty` is a Rust clone of `bat` (the cat replacement) with syntax highlighting, git diff markers, pager integration, and bundled Rhai grammar support. Plus an interactive TUI mode (`-i`) with vim-style navigation and relative line numbering.

Targets macOS + Linux. No Windows support.

## Architecture

Linear pipeline: **Input → Syntax Detection → Highlighting → Formatting → Output**, split across nine modules in `src/`:

| Module | Responsibility |
|---|---|
| `cli.rs` | clap derive `Cli` struct, all CLI flags |
| `config.rs` | TOML config loader at `~/.config/batty/config.toml` |
| `git.rs` | git2 diff vs HEAD, per-line `LineChange` map |
| `highlight.rs` | syntect `HighlightLines` wrapper, theme resolution |
| `input.rs` | `InputKind` (File/Stdin), `LineRange` |
| `interactive.rs` | TUI event loop, `TerminalGuard` (RAII), `scroll_viewport` |
| `pager.rs` | spawn `less`/`$PAGER` |
| `printer.rs` | decorations, render loop, `line_number_label` |
| `syntax.rs` | `SyntaxSet` builder (two-face + bundled Rhai) |
| `main.rs` | wiring; static path or `run_interactive` branch |

The interactive layer **reuses the printer per frame** rather than forking the render path. State lives in `interactive::run`'s loop; the printer is invoked with `cursor: Some(n)` and a viewport `LineRange`.

## Key dependencies

- `clap` 4 (derive)
- `syntect` 5 with `parsing` + `regex-onig` + `yaml-load` (the last is required for the bundled Rhai grammar)
- `two-face` 0.4 — bundled Sublime grammar + theme pack (same as bat)
- `git2` 0.19 (no default features) — diff against HEAD
- `crossterm` 0.28 — interactive raw mode + events
- `toml` 0.8 (parse-only) — config file
- `anstyle`, `anstyle-query`, `dirs`, `anyhow`, `pager`

## Build / test

```bash
cargo test                  # all unit + integration tests
cargo build                 # debug build — fast, used during iteration
cargo build --release       # size-optimized binary, ~2.6 MB on macOS arm64
./target/release/batty foo  # render foo with full decorations
./target/release/batty -i foo
```

**After any code change, always run `cargo test` and `cargo build --release`.** The release profile (LTO + `opt-level="z"` + `panic="abort"` + `codegen-units=1`) exercises code paths the debug build doesn't — type-level monomorphization differences, dead-code elimination, panic-strategy interactions — so it's not unusual for `cargo build` to succeed while `cargo build --release` fails (or vice versa).

**When building the release target, skip the debug target.** Don't run `cargo build` alongside `cargo build --release` as a matter of course — the release build is the one that ships and is the stricter check. The debug target is only built when explicitly requested or when it's needed for a specific reason (e.g., faster iteration on a non-shipping investigation); in that case build it separately.

Release profile: `opt-level="z"`, `lto=true`, `codegen-units=1`, `strip=true`, `panic="abort"`. Don't relax these without good reason — they extract significant size from `syntect`/`git2`/`crossterm`.

## Versioning

This project follows **[Semantic Versioning](https://semver.org/)** (semver). While at `0.x`:

- `0.x.y → 0.(x+1).0` for any new feature **or breaking change** (config format, CLI flag rename, removed flag, changed default behavior).
- `0.x.y → 0.x.(y+1)` for bug fixes only.

Once we hit `1.0.0`:

- `MAJOR` for breaking changes
- `MINOR` for backwards-compatible features
- `PATCH` for bug fixes

**Bump `Cargo.toml`'s `version` field as part of the same commit (or PR) that lands the change.** Don't ship features that drift from the declared version — readers grep `Cargo.toml` to know what they're running.

**Tagging implies three downstream publishes — same release, no exceptions.** When you push a `vX.Y.Z` tag, the release isn't complete until all three surfaces below are updated. A tag with only one or two updated leaves install paths out of sync: shields.io badges turn stale, `cargo install batty-cat` and `brew upgrade batty` keep returning the old version, and anyone following the README installs an older binary. Treat all three as part of the tag — same flow, no follow-up commits needed on this repo itself.

### 1. GitHub release

```sh
gh release create vX.Y.Z --generate-notes
```

### 2. crates.io

From the repo root, with the new version already in `Cargo.toml`:

```sh
cargo publish
```

The crate name is **`batty-cat`** (per `Cargo.toml [package] name`). The binary inside the crate is still **`batty`**. Requires `cargo login` to have been run once (token stored in `~/.cargo/credentials.toml`). `cargo publish` runs its own checks (clean working tree, no path-dependencies, etc.) and aborts cleanly if anything is wrong — fix the underlying issue rather than passing `--allow-dirty`.

The crate root has `#![doc = include_str!("../README.md")]` so docs.rs renders the README on the `batty-cat` landing page (added in 0.10.1 — binary crates otherwise show an empty module list). Keep that pragma in place when restructuring `src/lib.rs` or `src/main.rs`.

### 3. Homebrew tap (`../homebrew-cli/`)

The tap repo at `../homebrew-cli/` carries `Formula/batty.rb`. Update two fields:

- `url "https://github.com/codedeviate/batty/archive/refs/tags/vX.Y.Z.tar.gz"`
- `sha256 "<new-tarball-sha256>"`

Compute the sha256 from the GitHub-generated tarball after the release exists. **Always pass `-H "Cache-Control: no-cache"` so the fetch bypasses any intermediate caches (your ISP, corporate proxy, local resolver) and goes through to GitHub's origin:**

```sh
curl -sL -H "Cache-Control: no-cache" \
    https://github.com/codedeviate/batty/archive/refs/tags/vX.Y.Z.tar.gz \
    | shasum -a 256
```

**Important: GitHub's auto-generated tarball CDN can serve a transient/incomplete payload for the first minute or two after the tag is pushed.** Run the `shasum` command twice with a short pause between, and only proceed if both runs return the same hash. If they differ, wait 30–60 seconds and re-check until the hash stabilises. Using an unstable hash is the single most common cause of "homebrew reports wrong checksum" reports after a release — `batty 0.10.1 — fix sha256` in the tap log was exactly this scenario, and recon hit the same race twice (v0.82.0 and v0.85.0). v0.85.0's recheck without `Cache-Control: no-cache` re-read the same cached payload twice (a "stable" but wrong hash) and the mismatch surfaced only when users ran `brew install`.

The `Cache-Control: no-cache` rule isn't optional even on a "fresh" shell. Network paths cache aggressively; a single `curl` without the header is allowed to return whatever was last cached for that URL — which can be an early CDN payload that no longer matches what GitHub serves to homebrew clients.

Then commit and push the tap repo:

```sh
cd ../homebrew-cli
git add Formula/batty.rb
git commit -m "batty X.Y.Z"
git push origin main   # tap default branch is `main`, not `master`
```

(Tap commits follow the convention `<formula> X.Y.Z` — see `git log --oneline` in `../homebrew-cli` for examples.)

If `shasum` produces a hash that, after pasting into `batty.rb`, makes `brew install --build-from-source batty` fail with "SHA256 mismatch", recompute against the URL the formula points to (case matters — `vX.Y.Z` not `VX.Y.Z`) and amend with a follow-up commit like the existing `batty 0.10.1 — fix sha256` precedent in the tap log.

Don't forget the `man1.install "man/batty.1"` line in `Formula/batty.rb` (added in 0.10.0). If the man-page filename ever changes, the formula must change with it in the same release.

Recent history:
- `0.1.0` — initial release: full bat-parity (highlighting, git diff, pager, config, themes), bundled Rhai grammar, 36 tests, 2.5 MB binary.
- `0.2.0` — interactive TUI (`-i`), relative line numbers, `--top-pad`, **TOML config** (breaking — replaces line-based config format).
- `0.2.1` — fix: duplicate-flag conflict when config + CLI overlap; `--no-interactive` to override config; `--list-*` short-circuits before interactive; `BATTY_CONFIG_PATH` env var.
- `0.2.2` — fix: show `--no-interactive` in short help (`-h`), not just long help.
- `0.2.3` — fix: `--paging=never` also disables interactive mode (treats `--paging` as a global flat-output signal).
- `0.3.0` — feat: `--markdown` / `-m` renders Markdown to terminal escapes (via termimad); `--no-markdown` opt-out; `markdown = true` config key; `m` key toggles raw↔rendered in interactive mode (status bar shows `[md]`).
- `0.3.1` — fix: scrolling works in interactive markdown view (j/k/g/G/Ctrl-d/u/PgUp/PgDn). Independent rendered-row scroll counter; status bar shows `rendered N/M`. Adds OUT-OF-SCOPE.md.
- `0.4.0` — feat: `--follow` / `-f` tail mode (`tail -f` semantics with highlighting; `--tail-lines` for count, `--no-follow` opt-out, `follow = true` config key). Plus several limitations graduating to features in the same release: `rule` and `snip` style components functional; `expand_tabs` honors unicode column width; Rhai grammar gains range / bitwise / nested-block-comment support. Internal cleanup: dropped `Cli::parse_from_args`, `StyleFlags::any` is `cfg(test)`-gated, `term_width` uses `crossterm::terminal::size`, SIGPIPE reset on Unix.
- `0.4.1` — feat: Rhai grammar polish — backtick template strings with `${expr}` interpolation, `#{}` map-literal prefix, `??` / `?.` operators, `::` module accessor, leading-dot floats, expanded builtin list (`Fn`, `call`, `curry`, `is_def_var`, `is_def_fn`, `is_shared`, `eval`, `parse_int`, `parse_float`, `to_int`, `to_float`, `to_blob`, `to_array`, `to_map`), function-name scope no longer includes trailing whitespace.
- `0.5.0` — feat: `--wrap` actually wraps. `character` / `auto` break long lines at the terminal-width boundary with a continuation prefix that keeps the gutter intact. ANSI escape sequences are tracked across breaks so colors / `INVERT` resume on each continuation. Wide CJK / emoji chars count via `unicode-width`. Forced to `never` in interactive mode (viewport math assumes 1 source line = 1 visual row). **Behavior change** for users on the default `--wrap=auto`: long lines now wrap with a proper gutter rather than overflowing.
- `0.5.1` — chore: `--help` flags are alphabetical by long-flag name. `Cli` struct in `src/cli.rs` reordered to match; new convention captured in CLAUDE.md.
- `0.5.2` — feat: live `+` / `-` keys in interactive mode adjust `--top-pad` on the fly. Useful when a terminal (Warp) overlays UI on the alt-screen's top rows and the right pad value varies tab-to-tab or after pane resizes. Status bar shows `pad=N` when nonzero.
- `0.6.0` — feat: `--markdown-on-extension` (and `markdown-on-extension = true` config key) renders `.md` / `.markdown` / `.mdown` / `.mkd` files as markdown automatically while leaving source files raw. Precedence: `--no-markdown` > `--markdown` > `--markdown-on-extension` > default. Common config recipe: `markdown-on-extension = true` so `batty README.md` auto-renders without forcing it on `.rs` files.
- `0.7.0` — feat: source-line ↔ rendered-line mapping in markdown view (per-block rendering via pulldown-cmark + termimad), so `m`-toggle in interactive mode preserves scroll position both directions. Status bar shows `rendered N/M ↔ src K`. Plus new `--gutter` / `--no-gutter` flag and matching `n` key in interactive mode to live-toggle the gutter (line numbers + cursor glyph). Adds pulldown-cmark as a direct dep (already transitive via termimad).
- `0.7.1` — fix: markdown rendering truncated documents containing inline tags (`**bold**`, `*emph*`, `` `code` ``). The block-walker was decrementing depth on every `End` event including inline ones but only incrementing on block-level `Start`s, so inline tags would unbalance the counter and prematurely close the outer paragraph. Now increments on every `Start` and only sets `current_start` for top-level block tags. Regression test added.
- `0.8.0` — feat: gutter in markdown view shows source-line numbers (and optional grid bar) next to rendered blocks. New `markdown::render_with_gutter` reuses the 0.7.0 source map: each block's first rendered row carries its source line; continuation rows are blank in the number column with the grid bar repeating. Static path honors `--style=numbers,grid` (and `--no-gutter` strips). Interactive markdown view honors the live `n` toggle. Diff markers and the `▶` cursor glyph remain absent in markdown view (block-granular mapping doesn't make them meaningful). Also fixes a row-counting bug in `render_with_map` that over-counted rows when a block-render ended in `\n`, miscalculating later block-start indices.
- `0.9.0` — feat: `--encoding` (also `encoding = "..."` in config) selects how file/stdin bytes are decoded. Values: `auto` (default; tries UTF-8, falls back to ISO-8859-1 on decode failure), `utf-8` (strict — errors with a hint on invalid sequences), `iso-8859-1` / `latin1` (every byte 0x00–0xFF → U+0000–U+00FF). Previously `batty` always used strict UTF-8 (via `fs::read_to_string`), which made it unusable for Latin-1 logs and source files. Decoder is dependency-free; the default-`auto` change means older files that happened to be Latin-1 now open silently instead of erroring — a behavior change worth flagging. Reads are now always done as bytes via `fs::read` + `input::decode`.
- `0.9.1` — chore: publish to crates.io as `batty-cat` (binary stays `batty`). README badge header added.
- `0.10.1` — chore: add `#![doc = include_str!("../README.md")]` at the crate root so docs.rs renders the README on the `batty-cat` landing page (binary crates otherwise show an empty module list there). No behavior change in the binary.
- `0.10.0` — feat: colorized `--examples` flag (curated copy-pasteable scenarios, mirroring recon's pattern); short-circuits before pager / file validation; honors `NO_COLOR` + TTY detection. Plus first hand-written `man/batty.1` page covering all flags, the interactive keybindings, config schema, and environment variables. New CLAUDE.md conventions require both surfaces to stay in sync with `src/cli.rs`.
- `0.11.0`–`0.13.1` — see `CHANGELOG.md` (this local list wasn't backfilled for those releases; the changelog is authoritative). `0.13.0` shipped `--live`; `0.13.1` was the flicker fix.
- `0.13.2` — fix: interactive/live modes truncate long lines at the terminal edge (`printer::truncate_to_visible_width`) instead of letting the terminal soft-wrap them and scroll the top of the alt-screen off.
- `0.14.0` — feat: live `w` toggle soft-wraps long lines in interactive/live mode (source-line-anchored viewport, render-then-clip to `body_rows` visual rows; `visual_row_count` + `scroll_viewport_wrapped` + `step_by_rows`). Initial state from raw `--wrap`; continuation rows keep a blank gutter; status bar shows a `wrap` tag.

## Conventions

- **Commit messages:** Conventional-Commits-style prefixes (`feat:`, `fix:`, `refactor:`, `test:`, `chore:`). One feature per commit when practical.
- **Tests:** unit tests live alongside code in `#[cfg(test)] mod tests`; cross-binary tests in `tests/integration.rs` use `env!("CARGO_BIN_EXE_batty")`.
- **No dead code in main:** if a method isn't used, either remove it or `#[allow(dead_code)]` with a comment explaining why it stays. Two pre-existing dead-code warnings are tolerated (`src/markdown.rs`'s `render_to_string` fn and the `RenderedMarkdownWithGutter.gutter_width` field) — don't add more without justification.
- **Don't add deps speculatively.** Each new dependency adds binary weight even with LTO. Justify in the commit message.
- **Keep CLI flags alphabetical in `src/cli.rs`.** The `Cli` struct's flag fields are ordered alphabetically by their **long-flag name** (e.g. `--color` before `--decorations` before `--diff`). The positional `files` argument stays at the top of the struct; everything below it is sorted. Short flags ride along wherever their long form lands — they have no separate sort. clap renders `--help` in source order, so reordering the struct directly reorders `--help`. When adding a new flag, drop it into the right alphabetical slot rather than appending at the bottom. The existing `// NOTE:` comment at the top of `cli.rs` reminds future contributors.
- **Pure functions over IO-heavy helpers** where possible — `line_number_label` and `scroll_viewport` are unit-tested directly without setting up a full render or terminal.
- **Update README.md whenever a user-visible change lands.** The `README.md` is the public-facing doc and must stay in sync with the actual binary. Update it in the same commit as the change. User-visible changes that require a README update include:
  - Any new, removed, or renamed CLI flag (or change in default value)
  - Any change to interactive-mode keybindings
  - Any change to the config file format or schema
  - Any change to environment-variable behavior (`BATTY_CONFIG_PATH`, `PAGER`, `LESS`, `NO_COLOR`, etc.)
  - Promotion of a documented "limitation" to a real feature, or addition of a new known limitation
  - Any new platform supported / dropped
  - Any changes that affect the installation steps, build commands, or quick-start examples
  - Version bumps that introduce user-visible behavior (link to the relevant section if behavior changed)

  Internal refactors, test-only changes, and dependency upgrades that are user-invisible do **not** require a README update.

- **Keep the man page in sync.** `man/batty.1` is hand-written troff that mirrors `src/cli.rs`, the interactive keybindings, the config schema, and the environment variables. Treat it as a third public-facing surface alongside `README.md` and `OUT-OF-SCOPE.md`: update it in the **same commit** as any of these changes:
  - Any new, removed, or renamed CLI flag (or change in default value / short alias) → update the matching `.TP` entry and bump the version + date in the `.TH` line at the top.
  - Any change to interactive-mode keybindings → update the `INTERACTIVE MODE` section.
  - Any change to the config schema → update the `CONFIG` example block.
  - Any change to environment-variable behavior (`BATTY_CONFIG_PATH`, `PAGER`, `LESS`, `NO_COLOR`) → update the `ENVIRONMENT` section.
  - Every release (version bump in `Cargo.toml`) → bump the version and date in the `.TH` header line.

  Verify after edits with `man ./man/batty.1` (the local file path works directly on macOS / Linux). Don't auto-generate the man page from clap — the curated narrative sections (INTERACTIVE MODE, CONFIG, ENVIRONMENT) are not derivable from the `Cli` struct, and clap's autogen would lose them. The trade-off is that drift is easy; the rule above is the mitigation.

- **Keep `CHANGELOG.md` in sync.** Every version bump must add a matching `## [X.Y.Z] — YYYY-MM-DD` section (Keep-a-Changelog style: `Added` / `Changed` / `Fixed` / `Removed` / `Breaking`) in the same commit as the `Cargo.toml` bump. Reuse the language from the version-history bullet in this file rather than writing the changelog twice with different wording. Update the comparison links at the bottom of `CHANGELOG.md` (`[X.Y.Z]: …compare/vA.B.C...vX.Y.Z`) so each entry links to a real diff on GitHub. The `[Unreleased]` link should always point at `compare/vLATEST...HEAD`.

- **Keep `src/examples.rs` in sync.** The `--examples` output is curated, copy-pasteable scenarios — not a flag enumeration. When a flag's behavior changes (new value, new default, changed precedence) or a new flag lands that's interesting enough to demo, add or revise the matching example in the same commit. The output mirrors recon's `--examples` for cross-repo uniformity; preserve the `section` / `example` / `note` helper structure when extending it. Tests in the module verify it renders without panicking in both colored and uncolored modes, but they don't check content — that's on you.

- **Keep the README badge header in sync.** The top of `README.md` carries a row of shields.io badges (GitHub, latest release, crates.io, Homebrew tap, Rust edition / MSRV, license). Whenever the underlying fact changes, update the badge in the same commit:
  - Version bump in `Cargo.toml` → bump the `release-vX.Y.Z` badge.
  - MSRV change (`rust-version` in `Cargo.toml`) or edition change → update the Rust badge.
  - License change → update the license badge (and `Cargo.toml`'s `license` field).
  - Crate rename on crates.io → update the crates.io badge label + link.
  - Homebrew tap or formula rename → update the Homebrew badge label + link.
  - Repo move/rename → update the GitHub badge and all repo links.

  Badges are public-facing metadata; a stale badge misleads readers about what they're installing. Treat them like the version field — drift is a bug.

- **Keep `OUT-OF-SCOPE.md` current.** That file is the single canonical place for "considered but not implemented" items — review notes, deferred features, code-quality concessions, design alternatives the team rejected. The rules:
  - **Add** to it whenever a design decision or code review surfaces something we deliberately aren't doing. Don't bury that signal in a commit message — write it in `OUT-OF-SCOPE.md` so it's discoverable.
  - **Remove** an item the moment a change implements it. The file is the ground-truth list of what's missing; an entry that's no longer missing is misinformation.
  - Move items between sections as appropriate (e.g., a Rhai grammar gap that gets fixed leaves "Rhai grammar gaps"; a deferred CLI flag that ships leaves "Rendering" or wherever it lived).
  - Update the file in the same commit as the change that triggers the add or removal — same rule as `README.md`.

## Config file

`~/.config/batty/config.toml` (XDG-style, even on macOS — bat does the same). Top-level keys map to CLI long flag names with hyphens preserved:

```toml
theme        = "Dracula"
tabs         = 2
top-pad      = 2
line-numbers = "relative"
highlight-line = [10, 20]
```

- `bool true` → `--key`; `bool false` → omitted
- arrays → repeated `--key=item` flags
- malformed TOML logs a stderr warning and proceeds without config
- unknown keys are forwarded to clap, which rejects them with a clear error
- `BATTY_CONFIG_PATH=/path/to/other.toml` overrides the default path; set it
  to `/dev/null` to opt out of any config (used by the integration tests)
- bool flags use `overrides_with` so config + CLI overlap doesn't error;
  `--no-interactive` exists as the explicit negation of `--interactive`

## Known limitations / out of scope

- `--wrap` parsed but doesn't wrap (terminals wrap by default)
- `--diff-context` parsed but doesn't filter to changed regions
- `rule` and `snip` style components parsed, no inter-file separator emitted
- No Windows (uses `tput`, POSIX pager invocation)
- Interactive mode: keyboard only, single file, no search, no mouse, no persisted cursor position

## Specs / plans / reports

Spec, plan, and per-session reports live outside the repo at:

```
~/Development/Starweb/superpowers/batty/{specs,plans,reports}/
```

Read these when picking up unfamiliar context, but trust the code first — `git log` and the actual modules are authoritative.

## Don't

- Don't restructure the printer to remove its `&InputKind` argument without checking that the diff-marker lookup still works (printer uses `InputKind::File(p)` to call `git::diff_for_file`).
- Don't switch syntect's regex engine away from `regex-onig` — Sublime grammars rely on Oniguruma-specific constructs.
- Don't drop the `yaml-load` syntect feature; the bundled Rhai grammar requires it.
- Don't enter raw mode without going through `interactive::TerminalGuard` — the Drop impl is what restores the terminal on panic/Ctrl-c.
- Don't print bare `\n` from the interactive render path — translate to `\r\n` (raw mode otherwise produces a staircase).
