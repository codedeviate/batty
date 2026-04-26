# Out of scope

Things that have been considered, intentionally not implemented, and may stay that way — or may graduate to a real plan later. Add to this list when something is deferred during design / review; remove when an item ships.

## Platform

- **Windows support.** Code uses POSIX pager invocation, libc SIGPIPE reset, and assumes Unix-style `~/.config/`. Targeting Win would require a separate code path in `pager::setup` and config-path resolution, plus a Windows CI matrix. (`term_width` already uses `crossterm::terminal::size`, so that piece is portable.)

## Rendering

- **`--diff-context` filters output.** Today it's only passed to `git2::DiffOptions`; the printer still emits all lines. Real bat shows only changed regions ± context lines when `--diff` is on.
- **Word-boundary wrapping.** `--wrap=character` and `--wrap=auto` currently break at column boundaries (mid-word if necessary). Real word-boundary wrap (break at spaces, hyphenate gracefully) is a different algorithm — fine for prose, arguably wrong for source code anyway, so deferred unless asked.
- **`--wrap=auto` adapting on stdout-not-a-tty.** Real bat treats `auto` as "wrap when stdout is a TTY, never otherwise." We currently treat `auto` and `character` identically. Could add the TTY-aware nuance if someone pipes batty to a tool that chokes on inserted line breaks.
- **Source-line ↔ rendered-line cursor mapping in markdown view.** Markdown rendering transforms source structure (headings on their own line, lists with custom bullets, code blocks with chrome), so 1:1 line correspondence is impossible. Today the two views have independent scroll state: raw view uses a source-line `cursor`, markdown view uses a rendered-row `markdown_scroll`. A future enhancement could try a best-effort mapping (e.g., walk pulldown-cmark events and record source ↔ rendered offsets) but the UX upside is small.
- **Gutter (line numbers + cursor glyph) in markdown view.** Off by design — line numbers map to source positions which don't have a 1:1 rendered counterpart. The status bar shows `rendered N/M` for orientation instead. Reconsider if source-line mapping ever ships.
- **8-bit color downsampling.** Output uses truecolor (`as_24_bit_terminal_escaped`); 256-color terminals get whatever the terminal emulator does at render time. Real bat uses `ansi_colours` to emit nearest-color escapes when truecolor isn't supported.
- **HTML output mode.** Bat's `html_for_string`/`html_for_file` APIs in syntect aren't exposed.
- **Image rendering** (kitty / iTerm2 protocols) — `--show-images` not implemented.

## Theming

- **Custom theme loading from disk.** Themes come exclusively from `two-face::theme::extra()`. A real `--theme-file path/to/theme.tmTheme` flag is not wired.

## Interactive mode

- **Search (`/pattern`)** — vim-style search not implemented.
- **Mouse support** — keyboard only. No click-to-position-cursor, no scroll-wheel.
- **Mouse-driven link follow** in markdown view — also keyboard-only.
- **Multiple files in one session.** Today `-i` rejects `>1` file. A tabstrip / `:n` `:p` switching would be a fair amount of work.
- **Persisted cursor position across runs.** `~/.local/state/batty/positions.toml` or similar — not implemented.
- **`--wrap` is forced off in interactive mode.** Long lines truncate at the terminal edge. Honoring `--wrap` here would let one source line span multiple visual rows, which breaks the cursor / viewport / status-bar math (all currently 1 source line = 1 row). A proper fix needs per-visual-row scrolling; deferred until someone asks.
- **Horizontal scroll** for long lines that exceed the terminal width when `--wrap` is off — they're truncated by the terminal.

## Follow / tail mode

- **Low-latency file watching.** v1 polls every 200 ms via `fs::metadata`. A real `notify` / `inotify` / `kqueue` / `FSEvents` watcher would deliver appended bytes immediately at the cost of a fairly heavy dep + per-platform code.
- **Highlighter state preservation across polls.** Each poll re-creates the `Highlighter`, so a multi-line construct (block comment, multi-line string) that begins in one poll's contents and ends in another may briefly miscolor at the boundary. Fix would require persisting `syntect::easy::HighlightLines` state across iterations and replaying only new lines.
- **Multiple files concurrently.** v1 enforces single file. Real `tail -f a.log b.log` interleaves output with `==> file <==` headers.

## Markdown rendering

- **Inline syntax highlighting of fenced code blocks.** termimad's support is limited; we don't pre-process fences with syntect first. Code blocks render with termimad's default code styling.
- **Configurable markdown extension list.** `is_markdown_path()` matches `.md` / `.markdown` / `.mdown` / `.mkd` literally. If you want `.rmd` or `.txt` to auto-render, pass `--markdown` explicitly.
- **Content-based markdown detection.** No first-line sniffing for markdown-y patterns. Extension-based is the user's mental model; sniffing would mis-fire on plaintext with `# headers`.
- **Configurable markdown skin.** termimad's `MadSkin::default()` is used; no `--markdown-skin` flag.

## Rhai grammar gaps

- **Closure pipe syntax** `|x| x + 1` — `|` is matched as bitwise-or, no special closure context. Disambiguating from bitwise OR requires backtracking-style context the grammar engine can't easily express; most users won't notice since identifier and operator both color reasonably.
- **Full semantic awareness** of `Fn(...)`, `this`, `global`, etc. — we tokenize them (Fn as builtin; this/global as variable.language) but don't reflect Rhai's special semantics (e.g., `this` only valid inside method-style functions). That's intelligence beyond TextMate scope.
- **Char literal scope** — single-quoted `'a'` are technically char literals in Rhai, but we color them as `string.quoted.single` for theme compatibility. Could be revisited if a theme really needs different coloring.

## Code-quality nits (intentionally tolerated)

(empty — flush 0.5.0 cleared the `PrinterConfig::wrap` placeholder.)
