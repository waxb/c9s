# Project Context

## Purpose
c9s is a k9s-inspired TUI (Terminal User Interface) for managing Claude Code sessions. It provides a unified dashboard to discover, monitor, attach to, and launch Claude Code CLI sessions — acting as a specialized terminal multiplexer for Claude Code workflows. Key capabilities include:

- **Session discovery**: Scans the filesystem for active and historical Claude Code sessions
- **Embedded terminal**: Attach to live Claude sessions via PTY embedding (resume or launch new)
- **Usage monitoring**: Displays token usage, estimated cost (per-model pricing), and API rate-limit status
- **Session details**: Browse session config, git branch, model, permission mode, plan slugs, compaction count, and more
- **Persistence**: Tracks session history and statistics in a local SQLite database (~/.c9s/data.db)

## Tech Stack
- **Language**: Rust (edition 2021)
- **TUI framework**: ratatui 0.30 + crossterm 0.28 (alternate screen, mouse capture, raw mode)
- **Terminal emulation**: portable-pty 0.9 (PTY spawning) + vt100 0.16 (ANSI parser)
- **Async runtime**: tokio 1 (full features) — used selectively; the main event loop is synchronous
- **Data storage**: rusqlite 0.32 (bundled SQLite)
- **CLI parsing**: clap 4 (derive feature)
- **Serialization**: serde + serde_json
- **HTTP client**: ureq 3 (for Claude API usage endpoint)
- **Error handling**: anyhow 1 + thiserror 2
- **Date/time**: chrono 0.4
- **File watching**: notify 7
- **Build profile**: Release uses LTO, strip, and codegen-units=1 for minimal binary size

## Project Conventions

### Code Style
- **Formatting**: `cargo fmt` (enforced in CI)
- **Linting**: `cargo clippy -- -D warnings` (zero warnings policy)
- **Naming**: Standard Rust conventions — snake_case for functions/variables, PascalCase for types/enums
- **Module organization**: Feature-based modules (`session/`, `terminal/`, `ui/`, `store/`, `input/`) with `mod.rs` re-exporting public items
- **File size**: Small, focused files — most modules are under 400 lines
- **Error handling**: `anyhow::Result` for application errors, `thiserror` for typed domain errors

### Architecture Patterns
- **State machine UI**: `App` struct holds all application state; `ViewMode` enum drives which view is rendered. The main loop dispatches `Action` enums produced by the input handler.
- **Action/event dispatch**: Keyboard/mouse events are mapped to `Action` variants in `input/handler.rs`, then processed by `process_action()` in `main.rs`. This decouples input from behavior.
- **Embedded PTY**: `EmbeddedTerminal` spawns a `claude --resume <id>` process via `portable-pty`, reads output through a `vt100::Parser`, and writes input bytes back — all wrapped in `Arc<Mutex<>>` for thread-safe access.
- **Terminal multiplexing**: `TerminalManager` maintains a vec of embedded terminals with tab-switching, cycle-next/prev, and bell notification forwarding.
- **Session discovery**: `SessionDiscovery` scans `~/.claude/projects/` directories for JSONL session files, parses them to extract metadata (tokens, model, status, timestamps).
- **Repository pattern**: `Store` wraps a SQLite connection with `upsert_session`, cost queries, and auto-migration.
- **Dirty-flag rendering**: The main loop only redraws when `needs_draw` is true (input event, terminal output, or periodic refresh), reducing unnecessary rendering.

### Testing Strategy
- **Unit tests**: In-module `#[cfg(test)]` blocks (e.g., `session/mod.rs` tests cost estimation)
- **Dev dependencies**: `tempfile` for filesystem-based tests
- **CI**: `cargo test` runs on every PR

### Git Workflow
- **Branch strategy**: Feature branches merged via pull requests to `main`
- **Commit format**: Conventional commits (`feat:`, `fix:`, `refactor:`, `ci:`, `chore:`)
- **CI pipeline**: GitHub Actions on pull_request — fmt check, clippy, build, test
- **Release**: Separate `release.yml` workflow for publishing

## Domain Context
- **Claude Code sessions** are managed by the `claude` CLI from Anthropic. Each session creates JSONL log files under `~/.claude/projects/<project-hash>/` containing conversation turns, tool calls, token counts, and metadata.
- **Session status** is inferred from process liveness: if the PID is still running, the session is Active/Idle/Thinking; otherwise it's Dead.
- **Cost estimation** uses per-model pricing tiers (Opus, Sonnet, Haiku) with separate rates for input, output, cache-read, and cache-write tokens.
- **Usage API** fetches rate-limit data (5-hour and 7-day windows) from Claude's API using OAuth credentials stored in `~/.claude/credentials.json`.

## Important Constraints
- Requires an interactive TTY — exits immediately if stdout is not a terminal
- Requires `claude` CLI to be installed and on PATH
- macOS-focused development (uses `libc` for process checking, platform-conditional imports)
- The embedded terminal captures raw mode, so c9s must carefully manage mouse capture toggling between list view and terminal view
- SQLite database is single-writer; no concurrent access handling needed for a single-user TUI

## External Dependencies
- **Claude CLI** (`claude`): Required runtime dependency — c9s discovers, resumes, and launches sessions through it
- **Claude API** (api.claude.ai): Optional — used to fetch usage/rate-limit data via OAuth token from `~/.claude/credentials.json`
- **Filesystem**: Reads session data from `~/.claude/projects/` and config from `~/.claude/settings.json`
- **SQLite**: Local database at `~/.c9s/data.db` for session history persistence
