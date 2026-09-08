# Changelog

## 0.4.0

- Project x session matrix: all transcripts of a project root are listed; the list view shows collapsed project rows with live counts and summed cost and tokens, expandable into session rows.
- Project tabs in the terminal view hold several sessions each (`name [current/total]`); `!` marks a background session waiting for input, `*` a finished turn.
- Session pane on `Ctrl+]` (configurable via `~/.c9s/config.toml`): switch, new, fork, discard, rename, message and coordinate.
- Quick switcher (`Ctrl+Space`, also in the list view) lists projects with `[current/total]` and expands into sessions.
- New sessions start with `claude --session-id <uuid> -n <generated-name>`; forks use `--fork-session`.
- Take-over dialog before killing a session that runs in another terminal; unverified processes only offer a fork.
- Discard hides the session persistently and never deletes transcripts; `U` unhides.
- Registry-only sessions (no transcript yet) appear immediately; terminals opened through the resume picker are matched to their real session id by pid.
- Notifiers watch only their own transcript.
- SQLite schema v2 with title, session name, context, compaction, API error, cache write and hidden columns.

## 0.3.0

- Removed the tervezo integration.
- Corrected pricing: current Opus models at $5/$25, Fable at $10/$50, cache writes at 1.25x (5 minute) and 2x (1 hour) of input, Fable 5.1 cache reads at $0.25 per million tokens.
- Compaction, tool call, API error and context size statistics read from the transcript; session titles and last prompts shown.
- Live status, pid and session name from `~/.claude/sessions/*.json`; new Waiting status; kills only registry-verified pids.
- PTY child handles are kept and reaped; PTY size matches the rendered area; quit confirmation lists every live attached session.
- Bell notifications from registry status transitions; the `/tmp/c9s-bell-debug.log` file is gone.
- Session discovery runs on a background thread; clippy clean on Rust 1.96.
