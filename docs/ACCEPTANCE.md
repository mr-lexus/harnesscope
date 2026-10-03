# Real-device acceptance: Codex GUI on macOS

Status: **not executed on a real macOS GUI**. CI tests and a Windows browser
preview do not close this gate. Use a disposable project and fictitious task data
first. Do not paste real credentials as test data.

## Install the candidate

Build `codex/retrospective-evidence` with Node 24 and stable Rust:

```sh
cd web && npm ci && npm run build && cd ..
cargo build --release
bash scripts/install-macos.sh target/release/harnesscope
~/.local/bin/harnesscope capture setup --codex-home "$HOME/.codex"
```

Open `http://127.0.0.1:4242/sources`, enable sanitized evidence for active/archived
sessions, and review/trust hooks inside Codex. Register the test project's workflow
root in Evidence. Set `HARNESSCOPE_DB_PATH` before setup when using a custom database;
generated hooks pin its absolute path. Record OS, GUI application/engine versions,
separately installed `codex --version`, channels and paths.

## Scenarios and expected evidence

For each scenario record native session/turn IDs, observation IDs and all gaps.

1. Task requirement → clarification → tool → harmless test failure → fix → check →
   finish. Mark acceptance manually; turn completion must not imply acceptance.
2. Change AGENTS.md/a test skill between turns. Inspect versions and differences.
   Delete/recreate the file and inspect the explicit missing-file version.
3. Compact, interrupt/resume, fork, create/finish a subagent. Verify boundaries,
   parent relationships and usage. Inherited parent work must not increase child
   counters. Unavailable active context must remain unavailable.
4. Register a local [task adapter](TASK_ADAPTERS.md). Through the agent's configured
   task-tracker MCP, read a task, modify a fictitious description/comment, read
   again. Verify versions and call/session links. Test an error response and the
   same ID in two tracker namespaces. If the shape differs, update the local
   mapping; retain unsupported evidence without guessing links.
5. Stop the collector, generate hook activity, restart and verify queued delivery.
   Retry and check stable IDs. Pause/resume a source with appended records.
6. Use a synthetic canary such as `password=harnesscope-test-canary` in messages,
   arguments and results. Check its absence from DB, objects, queue, logs, backup
   and export. Read a fictitious `.env` in the disposable project and verify
   exclusion of its output, including delayed output.
7. Export a selected task/week, verify backup, restore to another path, inspect
   MCP history/context/workflow and confirm restored native sources are paused.
8. Reconstruct several real tasks across a week: requirements, workflow, actions,
   failures/rework, checks and accepted result. Every missing portion needs a reason.

Repeat the critical flow in Codex CLI. Verify LaunchAgent startup after login and
restart. Test disk pressure on a separate limited-capacity volume, not the system
volume. Record performance on large active histories.

## Close the gate

Keep a report with versions, channel configuration, scenario results, observation
references and remaining gaps. Update [CAPABILITIES.md](CAPABILITIES.md) only for
specific verified versions/platforms. Do not generalize from one build. Start the
production retrospective skill after this gate passes or the user explicitly
accepts the remaining documented limits.
