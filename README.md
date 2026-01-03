# grim

`grim` is an oldschool terminal process + task manager built with Rust and Ratatui.

- Interactive TUI for browsing processes, inspecting details, and killing processes
- Built-in task list (persisted to a local file)
- CLI mode for killing processes by PID or name

## Install

### From source

Requirements:
- Rust toolchain (stable)

Build:

```bash
cargo build --release
```

Run:

```bash
cargo run
```

The binary will be at:

```text
target/release/grim
```

## Usage

### Interactive mode (TUI)

Run the TUI (default if you run with no targets):

```bash
cargo run
```

Or explicitly:

```bash
cargo run -- --interactive
```

### CLI mode (kill by PID/name)

Kill by PID:

```bash
cargo run -- 12345
```

Kill by name substring (matches process name or full cmdline):

```bash
cargo run -- chrome
```

Use exact matching (name must equal):

```bash
cargo run -- --exact chrome
```

Skip confirmation prompts:

```bash
cargo run -- --force chrome
```

Kill child processes too:

```bash
cargo run -- --kill-children chrome
```

Watch mode (re-check periodically):

```bash
cargo run -- --watch --interval 2 chrome
```

Limit total kills and/or stop after timeout:

```bash
cargo run -- --watch --max 10 --timeout 60 chrome
```

### `--help`

```bash
cargo run -- --help
```

## Interactive Keybindings

Press `?` in the TUI to open the in-app help overlay for the current screen.

### Global

- `q` quit
- `p` processes screen
- `t` tasks screen
- `?` help overlay

### Navigation

- `↑/↓` move selection
- `PgUp/PgDn` page up/down

### Processes screen

- `/` focus filter input
- `r` refresh now
- `c` toggle kill-children
- `f` toggle force (skip confirmation)
- `k` kill selected process (force off = confirmation dialog)

### Tasks screen

- `a` add task
- `e` or `Enter` edit task
- `x` or `Space` toggle done
- `d` delete task

## Task Storage

Tasks are stored in a plain text file in the current working directory:

```text
grim_tasks.txt
```

Format:

```text
<done 0|1>\t<task text>
```

## Releases

This repository includes GitHub Actions workflows:

- CI: format + clippy + tests on pushes and PRs
- Releases: creates a GitHub Release with built binaries on every push to `main`

Each push to `main` produces a unique tag like:

```text
auto-<run_number>-<short_sha>
```

and uploads OS-specific binaries for Linux, macOS, and Windows.

## Development

Common commands:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo run
```

## License

No license file is currently included. Add one if you plan to distribute publicly.
