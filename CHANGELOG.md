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
- Transcript statistics are exact for every file size: lines are parsed with a streaming deserializer that skips message bodies, only appended bytes are parsed on refresh, and per-file offsets are checkpointed in SQLite so a restart with 1 GB of transcripts renders in well under a second. Previously files over 2 MB were sampled head and tail only, undercounting tokens and cost by up to 60x.
- Usage log: every fresh usage API reading and every keep-warm ping is stored in `~/.c9s/data.db` (`usage_samples`); `c9s usage-log [N]` prints the timeline so the cost of a ping versus a cold prompt can be read off the 5-hour percentage.
- Keep-warm: `Ctrl+q` in a session (or `w` in the pane and list) marks it with a red `X` and types a minimal prompt whenever it has been idle for 50 minutes, keeping the 1-hour prompt cache alive for up to 12 hours; configurable key under `[keys] keep_warm`.
- Usage panel on the selector page shows every per-model weekly window the usage API reports (the `limits[]` weekly_scoped entries such as Fable, plus `seven_day_opus` and `seven_day_sonnet`) under the all-models bar; panel height follows the number of bars.
- PTY is resized whenever the pane layout or the active session changes; list and quick switcher selection stay on the same row across background refreshes.
- `n` in the list view prefills the directory prompt with the selected project root; Enter on an empty prompt uses that root, an invalid path keeps the prompt open with an error instead of silently returning to the list, and long paths scroll so the end stays visible.

## 0.3.0

- Removed the tervezo integration.
- Corrected pricing: current Opus models at $5/$25, Fable at $10/$50, cache writes at 1.25x (5 minute) and 2x (1 hour) of input, Fable 5.1 cache reads at $0.25 per million tokens.
- Compaction, tool call, API error and context size statistics read from the transcript; session titles and last prompts shown.
- Live status, pid and session name from `~/.claude/sessions/*.json`; new Waiting status; kills only registry-verified pids.
- PTY child handles are kept and reaped; PTY size matches the rendered area; quit confirmation lists every live attached session.
- Bell notifications from registry status transitions; the `/tmp/c9s-bell-debug.log` file is gone.
- Session discovery runs on a background thread; clippy clean on Rust 1.96.
