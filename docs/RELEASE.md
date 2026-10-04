# Harnesscope v0.3.0-rc.2

[Documentation](../README.md#documentation)

**Prerelease for field testing.** Real Codex GUI acceptance on macOS, connected task-tracker response validation and week-long retrospective acceptance remain pending. This candidate does not replace stable v0.2.3 in Homebrew or the default install scripts. See [capabilities](CAPABILITIES.md) and the [acceptance checklist](ACCEPTANCE.md).

## Changes since v0.2.3

- Local retrospective reviews and comparisons, explicit process/turn scopes, compact responsive panel, source diagnostics and process monitoring.
- Restartable Codex history collection, observed token counters and fork boundaries; sanitized evidence archive with provenance and explicit exclusions. Supported JSONL/gzip and selected read-only SQLite history.
- Offline hook queue and local OTLP/HTTP JSON; workflow versions and diffs; task timelines, human outcomes and provider-neutral mappings for observed task-tracker MCP calls.
- Paginated evidence APIs, read-only stdio MCP, portable export/import and archive reindexing.
- Durable wrapper delivery, replay-safe events and checkpoints, verified backup/restore including referenced evidence objects, and automatic pre-migration backup.
- macOS binary cask installation without local compilation or an Xcode upgrade. The stable tap still installs v0.2.3; this candidate is downloaded separately.
- Complete English/Russian guide pairs with language-preserving navigation, included in the binary archives.

## Install the candidate

Download the archive for your platform from [this release](https://github.com/mr-lexus/harnesscope/releases/tag/v0.3.0-rc.2) and verify its SHA-256 against `checksums.txt` before extracting:

| Platform | Target in the archive name |
| --- | --- |
| macOS Apple Silicon | `aarch64-apple-darwin` |
| macOS Intel | `x86_64-apple-darwin` |
| Windows x64 | `x86_64-pc-windows-msvc` |
| Linux x64 | `x86_64-unknown-linux-gnu` |

The executable is self-contained; Rust, Node and Xcode are not needed to run it. Run `harnesscope --version` to confirm `0.3.0-rc.2`. For an isolated trial, set `HARNESSCOPE_DB_PATH` to a new database path and `HARNESSCOPE_SERVER_PORT` to an unused port in the environment used by the server and capture setup. Follow the [setup guide](EVIDENCE.md). Hooks require normal Codex trust; the optional macOS LaunchAgent installer requires Python 3 and has not yet passed real-device acceptance.

Before upgrading an existing collector, create and verify a [backup](BACKUPS.md), drain pending queues and stop the old process. This candidate uses schema 7 and creates a pre-migration snapshot for v5/v6. Do not point an older binary at the upgraded database; use a verified pre-upgrade copy at a separate path for rollback. Queues and original agent logs are separate from database backups.

## Known limits

No guarantee of exact model-request reconstruction, hidden prompts or per-role token counts. Secret removal is conservative best effort, not a guarantee for arbitrary text. No automatic workflow rewriting, analysis skill, remote synchronization or built-in task-tracker credentials. Passing platform CI and binary smoke checks does not close real GUI acceptance. Full details remain in the [support matrix](CAPABILITIES.md).
