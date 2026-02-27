# c9s

A k9s-inspired TUI for managing Claude Code sessions. Monitor, attach, and switch between multiple Claude Code sessions from a single terminal.

## Screenshot

<!-- TODO: add screenshot -->

## Quickstart

```sh
cargo install --git https://github.com/waxb/c9s
c9s
```

## Features

- Embedded PTY terminals with tab management
- Session discovery from `~/.claude` JSONL files
- Quick switcher popup (Ctrl+Space / Space in list)
- Config tree viewer with token estimates
- Usage dashboard (OAuth rate limits + local stats)
- Bell notifications when Claude finishes
- Session status (Active / Thinking / Idle / Dead)
- Native text selection in terminal mode

## Keybindings

### List View

| Key | Action |
|---|---|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `Enter` / `a` | Attach (open terminal) |
| `1`-`9` | Attach to session by number |
| `Space` | Quick switcher |
| `d` | Session detail panel |
| `n` | New session (enter path) |
| `/` | Filter sessions |
| `s` | Cycle sort column |
| `r` | Refresh session list |
| `?` | Toggle help |
| `Esc` | Back / clear filter |
| `q` | Quit |

### Terminal View

| Key | Action |
|---|---|
| `Ctrl+d` | Detach (back to list) |
| `Ctrl+Space` | Quick switcher |
| `Ctrl+n` / `Ctrl+p` | Cycle next / prev session |
| `Ctrl+k` / `Ctrl+j` | Scroll history up / down |

## Requirements

- Rust 1.75+ (edition 2021)
- macOS or Linux
- Claude Code CLI installed

## Building from Source

```sh
git clone https://github.com/waxb/c9s.git
cd c9s
cargo build --release
```

The binary will be at `target/release/c9s`.

## How It Works

c9s discovers Claude Code sessions by scanning `~/.claude/projects/` for JSONL conversation logs. It parses token usage, model info, git branch, and message counts from these files. Live session status is determined by cross-referencing running `claude` processes with their working directories.

When you attach to a session, c9s spawns a PTY terminal running `claude --resume <session-id>` in the session's working directory. The terminal is rendered via ratatui with full VT100 emulation (vt100 crate), supporting colors, cursor positioning, and scrollback.

## License

MIT
