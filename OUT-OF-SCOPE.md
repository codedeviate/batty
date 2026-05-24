# Out of Scope & Wishlist

A living list of items raised during design, implementation, or feature sweeps
that are either explicitly deferred, decided against, or noted as "maybe
later". Also doubles as a wishlist — items under "Waiting" are things worth
building once someone explicitly asks. Kept here so ideas don't disappear
into the black hole of spec files after each release.

Organized into four buckets by reason for non-inclusion. When an item ships,
remove it from this file and note the shipping version in the CHANGELOG
entry rather than leaving a crossed-out line here.

- **Waiting** — can be done; nobody's asked for it.
- **Deferred** — possible to implement; actively put off (scope/complexity
  trade-off or waiting on a concrete use case).
- **Not yet supported** — blocked by upstream / ecosystem maturity; may ship
  when the blocker clears.
- **Out of scope** — fundamentally can't be implemented, architecturally
  mismatched, or intentionally declined by policy.

---

## Waiting

### Rendering

- **Word-boundary wrapping.** `--wrap=character` and `--wrap=auto` currently break at column boundaries (mid-word if necessary). Real word-boundary wrap (break at spaces, hyphenate gracefully) is a different algorithm — fine for prose, arguably wrong for source code anyway, so deferred unless asked.

### Theming

- **Custom theme loading from disk.** Themes come exclusively from `two-face::theme::extra()`. A real `--theme-file path/to/theme.tmTheme` flag is not wired.

### Interactive mode

- **Persisted cursor position across runs.** `~/.local/state/batty/positions.toml` or similar — not implemented.

### Markdown rendering

- **Configurable markdown skin.** termimad's `MadSkin::default()` is used; no `--markdown-skin` flag.

---

## Deferred

### Rendering

- **`--diff-context` filters output.** Today it's only passed to `git2::DiffOptions`; the printer still emits all lines. Real bat shows only changed regions ± context lines when `--diff` is on.
- **Sub-paragraph row precision in markdown.** The source ↔ rendered map is block-granular: a wrapped 6-row paragraph shows the source-line on row 1 with continuations blank in the gutter. Per-row precision would require replacing termimad with a custom renderer (we walk pulldown_cmark events but defer rendering to termimad for layout).
- **8-bit color downsampling.** Output uses truecolor (`as_24_bit_terminal_escaped`); 256-color terminals get whatever the terminal emulator does at render time. Real bat uses `ansi_colours` to emit nearest-color escapes when truecolor isn't supported.
- **HTML output mode.** Bat's `html_for_string`/`html_for_file` APIs in syntect aren't exposed.
- **Image rendering** (kitty / iTerm2 protocols) — `--show-images` not implemented.

### Interactive mode

- **Search (`/pattern`)** — vim-style search not implemented.
- **Mouse support** — keyboard only. No click-to-position-cursor, no scroll-wheel.
- **Mouse-driven link follow** in markdown view — also keyboard-only.
- **Multiple files in one session.** Today `-i` rejects `>1` file. A tabstrip / `:n` `:p` switching would be a fair amount of work.
- **`--wrap` is forced off in interactive mode.** Long lines truncate at the terminal edge. Honoring `--wrap` here would let one source line span multiple visual rows, which breaks the cursor / viewport / status-bar math (all currently 1 source line = 1 row). A proper fix needs per-visual-row scrolling; deferred until someone asks.
- **Horizontal scroll** for long lines that exceed the terminal width when `--wrap` is off — they're truncated by the terminal.

### Follow / tail mode

- **Low-latency file watching.** v1 polls every 200 ms via `fs::metadata`. A real `notify` / `inotify` / `kqueue` / `FSEvents` watcher would deliver appended bytes immediately at the cost of a fairly heavy dep + per-platform code.
- **Highlighter state preservation across polls.** Each poll re-creates the `Highlighter`, so a multi-line construct (block comment, multi-line string) that begins in one poll's contents and ends in another may briefly miscolor at the boundary. Fix would require persisting `syntect::easy::HighlightLines` state across iterations and replaying only new lines.
- **Multiple files concurrently.** v1 enforces single file. Real `tail -f a.log b.log` interleaves output with `==> file <==` headers.

### Markdown rendering

- **Inline syntax highlighting of fenced code blocks.** termimad's support is limited; we don't pre-process fences with syntect first. Code blocks render with termimad's default code styling.

---

## Not yet supported

### Rhai grammar gaps

- **Closure pipe syntax** `|x| x + 1` — `|` is matched as bitwise-or, no special closure context. Disambiguating from bitwise OR requires backtracking-style context the grammar engine can't easily express; most users won't notice since identifier and operator both color reasonably.

---

## Out of scope

### Platform

- **Windows support.** Code uses POSIX pager invocation, libc SIGPIPE reset, and assumes Unix-style `~/.config/`. Targeting Win would require a separate code path in `pager::setup` and config-path resolution, plus a Windows CI matrix. (`term_width` already uses `crossterm::terminal::size`, so that piece is portable.)

### Markdown rendering

- **Configurable markdown extension list.** `is_markdown_path()` matches `.md` / `.markdown` / `.mdown` / `.mkd` literally. If you want `.rmd` or `.txt` to auto-render, pass `--markdown` explicitly.
- **Content-based markdown detection.** No first-line sniffing for markdown-y patterns. Extension-based is the user's mental model; sniffing would mis-fire on plaintext with `# headers`.

### Rhai grammar gaps

- **Full semantic awareness** of `Fn(...)`, `this`, `global`, etc. — we tokenize them (Fn as builtin; this/global as variable.language) but don't reflect Rhai's special semantics (e.g., `this` only valid inside method-style functions). That's intelligence beyond TextMate scope.
