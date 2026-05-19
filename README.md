# batty

[![GitHub](https://img.shields.io/badge/github-codedeviate%2Fbatty-181717?logo=github)](https://github.com/codedeviate/batty)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust edition 2021](<https://img.shields.io/badge/rust-2021_edition_(MSRV_1.86)-dea584?logo=rust>)](https://www.rust-lang.org)

[![Latest release](https://img.shields.io/badge/release-v0.10.0-blue)](https://github.com/codedeviate/batty/releases)
[![crates.io](https://img.shields.io/badge/crates.io-batty--cat-fc8d62?logo=rust)](https://crates.io/crates/batty-cat)
[![Homebrew](<https://img.shields.io/badge/dynamic/json?url=https://raw.githubusercontent.com/codedeviate/homebrew-cli/main/api/formula.json&query=$[?(@.name=='batty')].versions.stable&label=batty&logo=homebrew>)](https://github.com/codedeviate/homebrew-cli)

A from-scratch Rust clone of [`bat`](https://github.com/sharkdp/bat) — `cat` with syntax highlighting, git diff markers, and a pager. Plus:

- **Bundled Rhai grammar** — `.rhai` scripts highlight out of the box, including template strings with `${interpolation}`, `#{}` map literals, `??` / `?.` operators, `::` module paths, and a broad builtin list.
- **Interactive TUI mode** (`-i`) — vim-style navigation, line by line.
- **Vim-style relative line numbers** — center distances on a cursor.
- **Markdown rendering** (`-m`) — render Markdown like `glow`, with a `m` toggle in interactive mode.
- **Tail / follow mode** (`-f`) — `tail -f` semantics with syntax highlighting.
- **Colorized `--examples`** — curated, copy-pasteable scenarios for every common flag.
- **Man page** — install `man/batty.1` to your `MANPATH` for `man batty`.
- **TOML config** — `~/.config/batty/config.toml`.
- **Small release binary** — ~2.8 MB with full grammar/theme bundles.

Targets macOS and Linux.

---

## Installation

### Homebrew (macOS / Linux)

```bash
brew tap codedeviate/cli
brew install batty
```

The formula builds from source via `cargo`, so a Rust toolchain is pulled in as a build-time dependency and removed afterwards.

### Cargo (crates.io)

```bash
cargo install batty-cat
```

The crate is published as `batty-cat` on crates.io because the `batty` and `batty-cli` names were already taken — the installed binary is still `batty`.

### Build from source

```bash
git clone https://github.com/codedeviate/batty.git && cd batty
cargo build --release
# Binary lands at target/release/batty
```

Move it onto your `$PATH` however you like (e.g. `cp target/release/batty ~/.local/bin/`).

A `man` page lives at `man/batty.1` in the repo. To install it locally:

```bash
# macOS (Homebrew default MANPATH)
cp man/batty.1 /usr/local/share/man/man1/

# Linux (typical)
sudo cp man/batty.1 /usr/local/share/man/man1/
sudo mandb            # rebuild index if your distro caches
```

Then `man batty` works. You can also preview without installing: `man ./man/batty.1`.

Requires Rust **1.86** or newer.

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

| Flag                    | Description                                                                                                                                                                                                                                                                                                                                  |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `<FILES>...`            | One or more files. Use `-` for stdin (default when no files given).                                                                                                                                                                                                                                                                          |
| `-l, --language <NAME>` | Override syntax detection. Use any name shown by `--list-languages`.                                                                                                                                                                                                                                                                         |
| `--encoding <ENC>`      | Input file encoding: `auto` (default), `utf-8`, `iso-8859-1` (alias `latin1`). `auto` decodes as UTF-8 when valid and silently falls back to ISO-8859-1 otherwise; `utf-8` is strict and errors on invalid byte sequences; `iso-8859-1` always decodes byte-for-byte (each byte 0x00–0xFF → U+0000–U+00FF). Applies to file reads and stdin. |

### Display

| Flag                       | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `-p, --plain`              | No decorations (header / grid / numbers / changes). Equivalent to `--style=plain`.                                                                                                                                                                                                                                                                                                                                                                                               |
| `-n, --number`             | Show line numbers. Equivalent to `--style=numbers`.                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `-d, --diff`               | Show git diff markers in the gutter (`+` / `~` / `-`).                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `--diff-context <N>`       | Lines of context for diff display. _Default: 2._ (Parsed but currently doesn't filter to changed regions; see [Limitations](#limitations).)                                                                                                                                                                                                                                                                                                                                      |
| `-A, --show-all`           | Show non-printable characters: `→` for tab, `·` for space, `•` for control chars.                                                                                                                                                                                                                                                                                                                                                                                                |
| `--style <SPEC>`           | Comma-separated style components: `full`, `plain`, `numbers`, `grid`, `header`, `rule`, `changes`, `snip`. _Default: `full`._                                                                                                                                                                                                                                                                                                                                                    |
| `--gutter` / `--no-gutter` | Force the left-side gutter (line numbers + diff markers + grid bar) on or off, regardless of `--style`. `--no-gutter` is a "cleaner reading" preset that's less aggressive than `--plain` (header / rule / snip stay on). `--gutter` cancels `no-gutter = true` from config.                                                                                                                                                                                                     |
| `--line-range <RANGE>`     | Show only the given range, e.g. `10:20`, `:15`, `30:`, `42`. Rejects `0` and inverted ranges.                                                                                                                                                                                                                                                                                                                                                                                    |
| `-H, --highlight-line <N>` | Highlight specific line(s) with inverse video. Repeatable. The first one acts as the **cursor reference** for relative numbering.                                                                                                                                                                                                                                                                                                                                                |
| `--tabs <N>`               | Tab expansion width. _Default: 4._                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `--wrap <MODE>`            | `never` / `character` / `auto`. _Default: `auto`._ `character` and `auto` break long lines at the terminal-width boundary with a continuation prefix that preserves the gutter (line numbers / cursor / change marker remain blank on continuation rows; the grid bar repeats). Wide CJK / emoji chars count their actual display width. `--wrap=never` emits each source line in one shot, letting the terminal soft-wrap (or truncate). Forced to `never` in interactive mode. |
| `--decorations <WHEN>`     | `always` / `auto` / `never`. _Default: `auto`._ `never` collapses to plain output.                                                                                                                                                                                                                                                                                                                                                                                               |

### Theme & color

| Flag                     | Description                                                                                                                                                         |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--theme <NAME>`         | Color theme. Default: `Monokai Extended`. See `--list-themes`.                                                                                                      |
| `--color <WHEN>`         | `always` / `auto` / `never`. _Default: `auto`._ `auto` enables color when stdout is a TTY and `NO_COLOR` is unset.                                                  |
| `--line-numbers <STYLE>` | `absolute` (default) or `relative`. With `relative`, the cursor line shows its absolute number, others show distance. Falls back to `absolute` if no cursor is set. |

### Markdown rendering

| Flag                      | Description                                                                                                                                                                                                                                                                                                                |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `-m, --markdown`          | Render Markdown to terminal escapes instead of showing the raw source. Uses `termimad` under the hood. Works on any file, not just `.md`.                                                                                                                                                                                  |
| `--markdown-on-extension` | Render as Markdown only when the file extension is `.md` / `.markdown` / `.mdown` / `.mkd`. Lower priority than `--markdown` (which forces on for any file) and `--no-markdown` (which disables). Useful as a config default — set `markdown-on-extension = true` and `.md` files auto-render while source files stay raw. |
| `--no-markdown`           | Disable Markdown rendering. Overrides `markdown = true` and `markdown-on-extension = true` in the config.                                                                                                                                                                                                                  |

When `--markdown` is on, the gutter shows the **source-line number** of each top-level block on its first rendered row, with continuation rows blank in the gutter (matching how raw view handles wrapped lines). The grid bar repeats on every row when `--style` includes `grid`. Use `--no-gutter` to strip the gutter and read flush against the left margin. Diff markers (`changes`) and the cursor glyph (`▶`) don't appear in markdown view — block-granular mapping doesn't make them meaningful, and the status bar covers position info in interactive mode. The header prints with the language label `Markdown (rendered)`.

### Interactive mode

| Flag                | Description                                                                                                              |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `-i, --interactive` | Enter the TUI. See [Interactive mode](#interactive-mode) for keys.                                                       |
| `--no-interactive`  | Disable interactive mode. Overrides `interactive = true` in the config.                                                  |
| `--top-pad <N>`     | Reserve N rows at the top of the screen. _Default: 0._ Use `2` in [Warp](https://www.warp.dev/) to dodge its UI overlay. |

### Tail / follow

| Flag               | Description                                                                                                                                                                                                                                  |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `-f, --follow`     | `tail -f` semantics: render the last `--tail-lines` lines, then poll the file every 200 ms and render appended content as it arrives. Single file only; no stdin; bypasses the pager. Mutually exclusive with `--interactive`. Ctrl-C exits. |
| `--no-follow`      | Disable follow mode. Overrides `follow = true` in the config.                                                                                                                                                                                |
| `--tail-lines <N>` | Number of trailing lines to show on launch. _Default: 10._                                                                                                                                                                                   |

Truncation / rotation: when the file shrinks (e.g. `> error.log` or logrotate), batty prints a one-line notice and re-renders the last `--tail-lines` of the new content.

### Pager

| Flag              | Description                                                                                                                                                            |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--paging <WHEN>` | `always` / `auto` / `never`. _Default: `auto`._ Uses `$PAGER` or `less -RF`. **`never` also disables interactive mode** — treat it as a global "flat output" override. |

### Discovery

| Flag                   | Description                                                                                                                         |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `-L, --list-languages` | Print every supported language (one per line) and exit.                                                                             |
| `--list-themes`        | Print every bundled theme and exit.                                                                                                 |
| `--examples`           | Print colorized, copy-pasteable usage examples for every common flag and exit. Complements `--help` (terse) with curated scenarios. |
| `-h, --help`           | Short help.                                                                                                                         |
| `--help`               | Long help with full descriptions.                                                                                                   |
| `-V, --version`        | Print version.                                                                                                                      |

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
markdown       = false       # true → render every file as markdown
markdown-on-extension = true # true → render only .md / .markdown files
no-gutter      = false       # true → hide line numbers / changes / grid by default
follow         = false       # set true to default to tail mode
tail-lines     = 10
highlight-line = [10, 20]
encoding       = "auto"      # "utf-8" | "iso-8859-1" | "auto" (default)
```

### Mapping rules

| TOML             | argv              |
| ---------------- | ----------------- |
| `key = "string"` | `--key=string`    |
| `key = 42`       | `--key=42`        |
| `key = true`     | `--key`           |
| `key = false`    | (omitted)         |
| `key = [a, b]`   | `--key=a --key=b` |

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

| Key                    | Action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `j` / `Down`           | Cursor down one line                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `k` / `Up`             | Cursor up one line                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `g` / `Home`           | Jump to first line                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `G` / `End`            | Jump to last line                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `Ctrl-d`               | Half-page down                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `Ctrl-u`               | Half-page up                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `PageDown`             | Full page down                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `PageUp`               | Full page up                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `m`                    | Toggle rendered Markdown view ↔ raw source. Active when the file has a `.md` / `.markdown` / `.mdown` / `.mkd` extension, or when `--markdown` was passed on launch. Status bar shows `[md]` while in rendered mode. The toggle preserves your scroll position: pressing `m` from raw view lands you on the corresponding block in the rendered output, and pressing `m` again returns to the source line of the block you were viewing. The status bar in rendered mode shows `rendered N/M ↔ src K`. |
| `n`                    | Toggle the gutter (line numbers + cursor glyph) on/off. Initial state follows `--gutter` / `--no-gutter`. Status bar shows `no-gutter` when off.                                                                                                                                                                                                                                                                                                                                                       |
| `+` / `=`              | Increase `--top-pad` by 1 row (live). Status bar shows `pad=N` when nonzero.                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `-`                    | Decrease `--top-pad` by 1 row (saturates at 0).                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `q` / `Esc` / `Ctrl-c` | Quit                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

If your terminal hides the top of the alt-screen behind its own UI (Warp does this in some panes / after resizing), the live `+` / `-` bindings let you tune `top-pad` until content is visible — no need to exit, edit config, and retry. The value resets to whatever `--top-pad` (or `top-pad` in config) said next time you launch.

Restrictions:

- One file at a time — multiple files are rejected with an error.
- No stdin input — interactive mode requires a real file path.
- Pager is bypassed.
- Color is forced on (interactive mode in a TTY always wants color).

If the top of your screen is hidden behind a terminal-host overlay (e.g. Warp), pass `--top-pad=2` (or whatever number works) — or set `top-pad = 2` in your config.

---

## Environment variables

| Variable            | Effect                                                                     |
| ------------------- | -------------------------------------------------------------------------- |
| `BATTY_CONFIG_PATH` | Override the config file path. `/dev/null` disables config entirely.       |
| `PAGER`             | Pager binary (default `less`).                                             |
| `LESS`              | Pager arguments. batty sets `LESS=-RF` if unset before spawning the pager. |
| `NO_COLOR`          | When set (any value), disables color output in `--color=auto` mode.        |

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

## Changelog

Release history lives in [`CHANGELOG.md`](CHANGELOG.md), formatted per
[Keep a Changelog](https://keepachangelog.com/).

---

## License

MIT.
