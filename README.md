# batty

A from-scratch Rust clone of [`bat`](https://github.com/sharkdp/bat) — `cat` with syntax highlighting, git diff markers, and a pager. Plus:

- **Bundled Rhai grammar** — `.rhai` scripts highlight out of the box, including template strings with `${interpolation}`, `#{}` map literals, `??` / `?.` operators, `::` module paths, and a broad builtin list.
- **Interactive TUI mode** (`-i`) — vim-style navigation, line by line.
- **Vim-style relative line numbers** — center distances on a cursor.
- **Markdown rendering** (`-m`) — render Markdown like `glow`, with a `m` toggle in interactive mode.
- **Tail / follow mode** (`-f`) — `tail -f` semantics with syntax highlighting.
- **TOML config** — `~/.config/batty/config.toml`.
- **Small release binary** — ~2.8 MB with full grammar/theme bundles.

Targets macOS and Linux.

---

## Installation

```bash
git clone <repo> batty && cd batty
cargo build --release
# Binary lands at target/release/batty
```

Move it onto your `$PATH` however you like (e.g. `cp target/release/batty ~/.local/bin/`).

---

## Quick start

```bash
batty src/main.rs                    # full decorations
batty -p src/main.rs                 # plain output
batty -i src/main.rs                 # interactive TUI
batty -m README.md                   # render Markdown (glow-style)
batty -f error.log                   # tail -f with highlighting
batty --line-range 10:30 file.rs     # only lines 10–30
echo 'fn main() {}' | batty -l rust  # stdin with language hint
batty --diff modified_file.rs        # gutter diff markers
batty --list-languages               # show all bundled languages
batty --list-themes                  # show all bundled themes
```

---

## Flag reference

`batty` accepts the flags below. Many can also be set in the [config file](#config-file).

### Input

| Flag | Description |
|---|---|
| `<FILES>...` | One or more files. Use `-` for stdin (default when no files given). |
| `-l, --language <NAME>` | Override syntax detection. Use any name shown by `--list-languages`. |

### Display

| Flag | Description |
|---|---|
| `-p, --plain` | No decorations (header / grid / numbers / changes). Equivalent to `--style=plain`. |
| `-n, --number` | Show line numbers. Equivalent to `--style=numbers`. |
| `-d, --diff` | Show git diff markers in the gutter (`+` / `~` / `-`). |
| `--diff-context <N>` | Lines of context for diff display. *Default: 2.* (Parsed but currently doesn't filter to changed regions; see [Limitations](#limitations).) |
| `-A, --show-all` | Show non-printable characters: `→` for tab, `·` for space, `•` for control chars. |
| `--style <SPEC>` | Comma-separated style components: `full`, `plain`, `numbers`, `grid`, `header`, `rule`, `changes`, `snip`. *Default: `full`.* |
| `--line-range <RANGE>` | Show only the given range, e.g. `10:20`, `:15`, `30:`, `42`. Rejects `0` and inverted ranges. |
| `-H, --highlight-line <N>` | Highlight specific line(s) with inverse video. Repeatable. The first one acts as the **cursor reference** for relative numbering. |
| `--tabs <N>` | Tab expansion width. *Default: 4.* |
| `--wrap <MODE>` | `never` / `character` / `auto`. *Default: `auto`.* `character` and `auto` break long lines at the terminal-width boundary with a continuation prefix that preserves the gutter (line numbers / cursor / change marker remain blank on continuation rows; the grid bar repeats). Wide CJK / emoji chars count their actual display width. `--wrap=never` emits each source line in one shot, letting the terminal soft-wrap (or truncate). Forced to `never` in interactive mode. |
| `--decorations <WHEN>` | `always` / `auto` / `never`. *Default: `auto`.* `never` collapses to plain output. |

### Theme & color

| Flag | Description |
|---|---|
| `--theme <NAME>` | Color theme. Default: `Monokai Extended`. See `--list-themes`. |
| `--color <WHEN>` | `always` / `auto` / `never`. *Default: `auto`.* `auto` enables color when stdout is a TTY and `NO_COLOR` is unset. |
| `--line-numbers <STYLE>` | `absolute` (default) or `relative`. With `relative`, the cursor line shows its absolute number, others show distance. Falls back to `absolute` if no cursor is set. |

### Markdown rendering

| Flag | Description |
|---|---|
| `-m, --markdown` | Render Markdown to terminal escapes instead of showing the raw source. Uses `termimad` under the hood. Works on any file, not just `.md`. |
| `--no-markdown` | Disable Markdown rendering. Overrides `markdown = true` in the config. |

When `--markdown` is on, per-line decorations (line numbers, diff markers, cursor indicator, line range filtering) are skipped — Markdown rendering produces its own block structure where they don't apply. The header still prints with the language label `Markdown (rendered)`.

### Interactive mode

| Flag | Description |
|---|---|
| `-i, --interactive` | Enter the TUI. See [Interactive mode](#interactive-mode) for keys. |
| `--no-interactive` | Disable interactive mode. Overrides `interactive = true` in the config. |
| `--top-pad <N>` | Reserve N rows at the top of the screen. *Default: 0.* Use `2` in [Warp](https://www.warp.dev/) to dodge its UI overlay. |

### Tail / follow

| Flag | Description |
|---|---|
| `-f, --follow` | `tail -f` semantics: render the last `--tail-lines` lines, then poll the file every 200 ms and render appended content as it arrives. Single file only; no stdin; bypasses the pager. Mutually exclusive with `--interactive`. Ctrl-C exits. |
| `--no-follow` | Disable follow mode. Overrides `follow = true` in the config. |
| `--tail-lines <N>` | Number of trailing lines to show on launch. *Default: 10.* |

Truncation / rotation: when the file shrinks (e.g. `> error.log` or logrotate), batty prints a one-line notice and re-renders the last `--tail-lines` of the new content.

### Pager

| Flag | Description |
|---|---|
| `--paging <WHEN>` | `always` / `auto` / `never`. *Default: `auto`.* Uses `$PAGER` or `less -RF`. **`never` also disables interactive mode** — treat it as a global "flat output" override. |

### Discovery

| Flag | Description |
|---|---|
| `-L, --list-languages` | Print every supported language (one per line) and exit. |
| `--list-themes` | Print every bundled theme and exit. |
| `-h, --help` | Short help. |
| `--help` | Long help with full descriptions. |
| `-V, --version` | Print version. |

### Flag-conflict semantics

- All boolean flags use `overrides_with` so config + CLI can both set a flag without `cannot be used multiple times` errors.
- `--interactive` and `--no-interactive` are mutual overrides — last occurrence wins.
- `--paging=never` implies `--no-interactive`.

---

## Config file

`~/.config/batty/config.toml` (XDG-style on every platform, including macOS).

Top-level keys mirror CLI long flag names with hyphens preserved:

```toml
# ~/.config/batty/config.toml
theme          = "Dracula"
tabs           = 2
top-pad        = 2
line-numbers   = "relative"
interactive    = true
markdown       = false       # set true to default-render markdown
follow         = false       # set true to default to tail mode
tail-lines     = 10
highlight-line = [10, 20]
```

### Mapping rules

| TOML | argv |
|---|---|
| `key = "string"` | `--key=string` |
| `key = 42` | `--key=42` |
| `key = true` | `--key` |
| `key = false` | (omitted) |
| `key = [a, b]` | `--key=a --key=b` |

- Comments (`#`) are allowed.
- Malformed TOML logs a warning to stderr and proceeds without config.
- Unknown keys are forwarded to clap, which rejects them with a clear error — typos surface as `unrecognized argument`.

### `BATTY_CONFIG_PATH`

Set this env var to override the default location:

```bash
BATTY_CONFIG_PATH=/path/to/other.toml batty foo.rs
BATTY_CONFIG_PATH=/dev/null batty foo.rs            # opt out of any config
```

---

## Interactive mode

```bash
batty -i src/main.rs
```

Enters raw mode in the alternate screen. A `▶` glyph in the gutter marks the cursor line; the bottom row is a status bar (`file  line N/M  (abs|rel mode)  vim-keys: ...  q quit`).

| Key | Action |
|---|---|
| `j` / `Down` | Cursor down one line |
| `k` / `Up` | Cursor up one line |
| `g` / `Home` | Jump to first line |
| `G` / `End` | Jump to last line |
| `Ctrl-d` | Half-page down |
| `Ctrl-u` | Half-page up |
| `PageDown` | Full page down |
| `PageUp` | Full page up |
| `m` | Toggle rendered Markdown view ↔ raw source. Active when the file has a `.md` / `.markdown` / `.mdown` / `.mkd` extension, or when `--markdown` was passed on launch. Status bar shows `[md]` while in rendered mode. |
| `+` / `=` | Increase `--top-pad` by 1 row (live). Status bar shows `pad=N` when nonzero. |
| `-` | Decrease `--top-pad` by 1 row (saturates at 0). |
| `q` / `Esc` / `Ctrl-c` | Quit |

If your terminal hides the top of the alt-screen behind its own UI (Warp does this in some panes / after resizing), the live `+` / `-` bindings let you tune `top-pad` until content is visible — no need to exit, edit config, and retry. The value resets to whatever `--top-pad` (or `top-pad` in config) said next time you launch.

Restrictions:

- One file at a time — multiple files are rejected with an error.
- No stdin input — interactive mode requires a real file path.
- Pager is bypassed.
- Color is forced on (interactive mode in a TTY always wants color).

If the top of your screen is hidden behind a terminal-host overlay (e.g. Warp), pass `--top-pad=2` (or whatever number works) — or set `top-pad = 2` in your config.

---

## Environment variables

| Variable | Effect |
|---|---|
| `BATTY_CONFIG_PATH` | Override the config file path. `/dev/null` disables config entirely. |
| `PAGER` | Pager binary (default `less`). |
| `LESS` | Pager arguments. batty sets `LESS=-RF` if unset before spawning the pager. |
| `NO_COLOR` | When set (any value), disables color output in `--color=auto` mode. |

---

## Examples

```bash
# Render a README in your terminal
batty -m README.md

# Mix: render markdown AND open it interactively (m toggles to raw)
batty -m -i README.md

# Tail a log file with syntax highlighting
batty -f error.log

# Tail with the last 50 lines visible on launch
batty -f --tail-lines=50 error.log

# Highlight a Rhai script
batty path/to/script.rhai

# Show lines 50–80 with line numbers, no other decorations
batty --style=numbers --line-range 50:80 src/lib.rs

# Cursor-centric relative numbering for line 42
batty --line-numbers=relative --highlight-line=42 src/main.rs

# Show only the modified portion of a file with diff markers
batty --diff src/main.rs

# Render plain text from stdin as Python
cat data.py | batty --language python --plain

# Force interactive even though config says interactive = false
batty -i README.md

# One-off non-interactive run despite `interactive = true` in config
batty --no-interactive README.md
batty --paging=never README.md     # equivalent

# Skip the user's config for one run
BATTY_CONFIG_PATH=/dev/null batty README.md

# List bundled themes
batty --list-themes
```

---

## Limitations

The full list lives in [`OUT-OF-SCOPE.md`](OUT-OF-SCOPE.md). Highlights:

- `--diff-context` doesn't restrict output to changed regions; it only sets the diff window passed to git2.
- `--wrap` breaks at column boundaries, not word boundaries — fine for source code, less ideal for prose. Forced off in interactive mode (cursor / viewport / status-bar all assume 1 source line = 1 visual row).
- No Windows support (POSIX pager invocation, etc.).
- Interactive mode is keyboard-only: no search, no mouse, no persistent cursor across runs.
- Follow mode polls every 200 ms; a real `inotify`/`kqueue` watcher would be lower-latency but adds platform code.
- Follow mode rebuilds the syntax highlighter on every poll, so multi-line constructs (block comments, multi-line strings) that span a poll boundary may briefly miscolor.

---

## License

MIT.
