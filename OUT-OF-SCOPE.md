# Out of scope

Things that have been considered, intentionally not implemented, and may stay that way — or may graduate to a real plan later. Add to this list when something is deferred during design / review; remove when an item ships.

## Platform

- **Windows support.** Code uses `tput`, POSIX pager invocation, and assumes Unix-style `~/.config/`. Targeting Win would require a separate code path in `main::term_width`, `pager::setup`, and config-path resolution, plus a Windows CI matrix.

## Rendering

- **`--wrap` actually wraps.** Currently parsed (`never` / `character` / `auto`) but inert; terminals already wrap. A real implementation would need column-aware wrapping that respects ANSI escape boundaries.
- **`--diff-context` filters output.** Today it's only passed to `git2::DiffOptions`; the printer still emits all lines. Real bat shows only changed regions ± context lines when `--diff` is on.
- **`rule` and `snip` style components.** Accepted in the `--style=...` spec but emit nothing. `rule` would draw an inter-file separator; `snip` would draw an indicator where `--line-range` skipped lines.
- **Multi-byte width in `expand_tabs`.** `col` is incremented one per `char`, so CJK and emoji mis-align tab stops. Fix would use `unicode-width::UnicodeWidthChar`.
- **Source-line ↔ rendered-line cursor mapping in markdown view.** Markdown rendering transforms source structure (headings on their own line, lists with custom bullets, code blocks with chrome), so 1:1 line correspondence is impossible. Today the two views have independent scroll state: raw view uses a source-line `cursor`, markdown view uses a rendered-row `markdown_scroll`. Toggling raw → markdown resets the rendered scroll to the top of the document; markdown → raw leaves the source cursor untouched. A future enhancement could try a best-effort mapping (e.g., walk pulldown-cmark events and record source ↔ rendered offsets) but the UX upside is small.
- **Gutter (line numbers + cursor glyph) in markdown view.** Off by design — line numbers map to source positions which don't have a 1:1 rendered counterpart, and the cursor glyph would point at a meaningless row. The status bar shows `rendered N/M` for orientation instead. If we ever ship the source-line mapping above, this should be reconsidered.
- **8-bit color downsampling.** Output uses truecolor (`as_24_bit_terminal_escaped`); 256-color terminals get whatever the terminal emulator does at render time. Real bat uses `ansi_colours` to emit nearest-color escapes when truecolor isn't supported.
- **HTML output mode.** Bat's `--language=html-aware` and the html_for_string/html_for_file APIs in syntect aren't exposed.
- **Image rendering** (kitty / iTerm2 protocols) — `--show-images` not implemented.
- **Live-reload / watch mode** (`-w` / `--watch`) — not implemented.

## Theming

- **Custom theme loading from disk.** Themes come exclusively from `two-face::theme::extra()`. A real `--theme-file path/to/theme.tmTheme` flag is not wired.

## Interactive mode

- **Search (`/pattern`)** — vim-style search not implemented.
- **Mouse support** — keyboard only. No click-to-position-cursor, no scroll-wheel.
- **Mouse-driven link follow** in markdown view — also keyboard-only.
- **Multiple files in one session.** Today `-i` rejects `>1` file. A tabstrip / `:n` `:p` switching would be a fair amount of work.
- **Persisted cursor position across runs.** `~/.local/state/batty/positions.toml` or similar — not implemented.
- **Horizontal scroll** for long lines that exceed the terminal width — they're truncated by the terminal.

## Markdown rendering

- **Inline syntax highlighting of fenced code blocks.** termimad's support is limited; we don't pre-process fences with syntect first. Code blocks render with termimad's default code styling.
- **Auto-enable markdown for `.md` extensions.** Currently opt-in only via `--markdown` / `-m` or `markdown = true` in config. Auto-detection-with-opt-out (`--no-markdown` to disable) was explicitly considered and rejected.
- **Configurable markdown skin.** termimad's `MadSkin::default()` is used; no `--markdown-skin` flag.

## Rhai grammar gaps

- **Backtick template strings** with `${name}` interpolation — Rhai supports them; our grammar doesn't tokenize the interpolation.
- **`#{}` map literals** — not specifically scoped.
- **Range operators** `..`, `..=` — not in the operator alternation.
- **Bitwise operators** `&`, `|`, `^`, `~`, `<<`, `>>` — not matched.
- **Nullable / Elvis-like `?` and `??`** — not matched.
- **Closure pipe syntax** `|x| x + 1` — `|` not specifically handled.
- **Nested block comments.** `/* outer /* inner */ outer */` ends at the first `*/` — Rhai allows nesting.
- **`Fn(...)` keyword and `this`/`global` semantics** — `this`/`global` are tagged `variable.language` but the broader semantics aren't reflected.

## Code-quality nits (intentionally tolerated)

- `Cli::parse_from_args` helper is unused; kept for API symmetry, marked with `#[allow(dead_code)]` candidate.
- `StyleFlags::any()` is only used in `#[cfg(test)]`; emits a `dead_code` warning at non-test builds.
- `PrinterConfig::wrap` field is currently a placeholder and emits a dead-code warning.
- `term_width()` shells out to `tput cols` per invocation rather than reading `TIOCGWINSZ` directly — works on macOS/Linux, slightly fragile under unusual TTY setups.
- No SIGPIPE handler — `batty bigfile.rs | head -3` may print a "broken pipe" complaint to stderr depending on timing.
