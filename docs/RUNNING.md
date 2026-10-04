# Daily use of Harnesscope

[Documentation](../README.md#documentation)

> This document records the October 3, 2026 stage; statuses and plans below belong to that checkpoint. See the [capability matrix](CAPABILITIES.md) for current support and outstanding acceptance checks.

## On this computer

The working panel is **http://127.0.0.1:4242/**. Port 4343 was used only for the test panel.

Open **Harnesscope** from the Windows Start menu. It starts the local server if necessary and opens the panel. The installed build requires no terminal, Node.js or Rust.

The collector connects to the selected Codex installation's `sessions` directory. Existing files import incrementally; new files and appended lines are picked up automatically. You can close the browser and continue in Codex: collection continues while the server runs.

User-login startup is enabled on this Windows installation. The server does not run before login; afterwards it resumes saved checkpoints. It is a user background application, not a system service or watchdog. After a crash, open Harnesscope from Start; retained source logs allow catch-up.

## Collected data

- Session/turn IDs, start/end, observed model and reasoning effort.
- Tool calls represented by supported rollout records.
- Available token counters. Modern per-turn counters take precedence over legacy `token_count`; they must not be added together.
- Initial working directory and relationships supported by explicit evidence.

By default, messages, instructions, reasoning, tool arguments and results **are not retained**. Source files remain unchanged. Storage and HTTP are local; collection makes no model API calls. Metadata such as paths and tool names can still be sensitive.

## Controls

**Sources** shows status, last scan, file counts, backlog and errors. **Pause** retains the checkpoint; **Resume** continues. Closing the tab and Pause automatic refresh affect display only, not collection.

The **Harnesscope - stop collection** Start-menu shortcut stops the whole server. Opening Harnesscope or logging into Windows again restarts it. To pause a particular source persistently, use Pause in Sources.

Login startup uses `Harnesscope Collector.lnk` in the user's Startup folder. Open it with `shell:startup` in Run. Deleting only that shortcut disables login startup without deleting the database. The installer recreates it with `-AutoStart`.

Executable: `%LOCALAPPDATA%\Programs\harnesscope\bin\harnesscope.exe`.
Database: `%LOCALAPPDATA%\harnesscope\harnesscope\data\harnesscope.db`.
Before an update, the local installer saves a verified copy of the existing database under `backups`. See [additional backups](BACKUPS.md).

## Older-format errors

A failed file does not stop other imports. Expand **File checkpoints and diagnostics**: problematic files appear first. Do not edit source logs just to make an indicator green.

During inspection on 2026-10-03, three legacy fork rollouts contained child metadata followed by copied parent history. The collector at that stage rejected the mixed format to avoid attributing parent work to a child and doubling statistics. New ordinary rollouts continued collecting. Completion/tokens without an observed turn start also do not create an invented execution; the record remains unmapped.

A historical turn's RUNNING status means no completion was recorded. It does not prove the process is currently running. Process observation uses the separate universal wrapper and Monitor.

## Reinstall from source on Windows

```powershell
cd web
npm ci
npm run build
cd ..
cargo build --release -j 1
.\scripts\install-local.ps1 -AutoStart
```

The installer uses `CODEX_HOME\sessions` when `CODEX_HOME` is set, otherwise the user's `.codex\sessions`. Options include `-CodexPath 'D:\Codex\sessions'`, `-Port 4242`, `-InstallDir`, `-DataDir`. Collection uses metadata mode. If an existing registration has another content policy, installation stops instead of silently changing it.

Administrator access, network binary downloads and PowerShell execution-policy changes are unnecessary. Hidden startup uses local VBS launchers and WScript. If WScript is blocked by organization policy or unavailable in a future Windows version, use `harnesscope serve` in a terminal and an approved startup mechanism.

Linux/macOS support `sources add-codex` and `serve`; the startup installer described in this stage targets Windows. Unix SIGTERM waits for the current bounded collector batch before shutdown.
