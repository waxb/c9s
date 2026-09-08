# c9s

A k9s-inspired TUI for managing Claude Code sessions. Monitor, attach, and switch between many Claude Code sessions, several per project, from a single terminal.

## Screenshot

<!-- TODO: add screenshot -->

## Quickstart

```sh
cargo install --git https://github.com/waxb/c9s
c9s
```

## Features

- Project x session matrix: every transcript under a project root is listed, not just the newest one
- Embedded PTY terminals in project tabs; each tab holds any number of sessions of that root (`name [current/total]`)
- Session pane (`Ctrl+]`) to switch, create, fork, discard, rename and message the sessions of the current project
- Quick switcher (`Ctrl+Space`) over projects, expandable into their sessions
- Live status (Active / Waiting / Idle / Dead) and session names read from the Claude Code session registry
- Session titles, context window fill, cost with correct cache write pricing, compactions and API errors per session, per project and in total
- Safe take-over: attaching a session that runs in another terminal asks before killing the verified process, or forks instead
- Bell notifications when a background session finishes a turn or waits for input (`*` and `!` on the tab)
- New sessions get a docker-style name (`brave-otter`) passed to `claude -n`, so they are resumable by name
- Usage dashboard (OAuth rate limits + local stats), config tree viewer, side shell pane (`Ctrl+t`)
- Native text selection in terminal mode

## Keybindings

### List View

Projects are collapsed rows by default; expand them to see their sessions.

| Key | Action |
|---|---|
| `j` / `Down`, `k` / `Up` | Move |
| `g` / `G` | Jump to top / bottom |
| `Space`, `l` / `Right` | Expand project |
| `Space`, `h` / `Left` | Collapse project |
| `Enter` / `a` | Attach (project row: its current session) |
| `1`-`9` | Attach the Nth project's current session |
| `Ctrl+Space` | Quick switcher |
| `n` | New session (enter path, then name) |
| `F` | Fork the selected session (new id, same context) |
| `x` | Discard session (terminate if c9s-owned or verified, hide) |
| `u` / `U` | Hide session / unhide dialog for the project |
| `Ctrl+b` | Open the `claude --resume` picker for the selected project |
| `d` | Session detail panel (`f` there opens the transcript picker) |
| `/` | Filter (matches project, path, branch, title, session name) |
| `s` | Cycle sort column |
| `r` | Refresh |
| `L` | Toggle log panel |
| `?` | Toggle help |
| `Esc` | Back / clear filter |
| `q` | Quit |

### Terminal View

| Key | Action |
|---|---|
| `Ctrl+d` | Detach (back to list) |
| `Ctrl+Space` | Quick switcher |
| `Ctrl+n` / `Ctrl+p` | Next / previous project tab |
| `Ctrl+]` | Toggle the session pane |
| `Ctrl+q` | Toggle keep-warm pings for the current session (red `X` marker everywhere; pings a minimal prompt after 50 minutes idle, for 12 hours) |
| `Ctrl+k` / `Ctrl+j` | Scroll history up / down |
| `Ctrl+t` | Toggle side shell pane |

### Session Pane

| Key | Action |
|---|---|
| `j` / `k`, `Enter`, `1`-`9` | Select and switch to a session |
| `n` | New session in this project (name dialog) |
| `f` | Fork the selected session |
| `x` | Discard the selected session (confirmation) |
| `r` | Rename the selected session (`/rename` is typed into it) |
| `m` | Type a message into the selected session's input |
| `M` | Ask the current session to coordinate with the selected one (native cross-session messaging) |
| `Esc` / `Ctrl+]` | Close the pane |

The pane key can be changed in `~/.c9s/config.toml`:

```toml
[keys]
session_pane = "ctrl-]"
```

## Requirements

- Rust 1.75+ (edition 2021)
- macOS or Linux
- Claude Code CLI 2.1.224 or later (session registry, `--session-id`, `--fork-session`, cross-session messaging)

## Building from Source

```sh
git clone https://github.com/waxb/c9s.git
cd c9s
cargo build --release
```

The binary will be at `target/release/c9s`.

## How It Works

c9s discovers Claude Code sessions by scanning `~/.claude/projects/` for JSONL transcripts on a background thread every five seconds (or on `r`) and groups them by working directory. It parses token usage (including the 5 minute and 1 hour cache write split), model, git branch, message and tool call counts, compactions, titles, and the current context size from these files. Cost estimates use the published per-model API prices; on a subscription plan they show the API-equivalent value, not what you are billed.

Live sessions come from the registry files Claude Code writes under `~/.claude/sessions/` (pid, session id, cwd, name, status), polled every second, with a fallback to cross-referencing running `claude` processes with their working directories for older Claude Code versions. A session that is live but has no transcript yet still appears.

Attaching resumes the session with `claude --resume <id>` in a PTY inside a tab for its project root. New sessions start as `claude --session-id <uuid> -n <name>` so their transcript is tracked from the first line, and forks use `--fork-session`. Discarding terminates only processes c9s owns or the registry verifies, keeps the transcript, and remembers the hidden flag in `~/.c9s/data.db`.

Session-to-session communication builds on Claude Code's own cross-session messaging: the pane shows each live session's registry name, `M` asks the current session to reach a peer through `SendMessage`, and `m` types a message straight into an attached session's input.

## License

MIT
