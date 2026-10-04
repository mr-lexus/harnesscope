# Harnesscope audit and rework

[Documentation](../README.md#documentation)

> This document records the October 3, 2026 stage; statuses and plans below belong to that checkpoint. See the [capability matrix](CAPABILITIES.md) for current support and outstanding acceptance checks.

Date: October 3, 2026. Reviewed Rust/React, SQLite, API, wrappers, Git observation, installers, CI and UI. At the initial audit checkpoint, changes were in the working tree; no release or deployment had been performed.

The successive audit stages are retained below. A local Windows collector was installed after the initial review; see [daily use](RUNNING.md). The pre-commit review appears at the end.

A native Codex rollout collector was subsequently added: Sources, checkpoints/restarts, turns, tool calls and observed tokens. See [native sources](NATIVE_SOURCES.md) for its scope.

## Main fixes

| Area | Problem | Fix |
| --- | --- | --- |
| HTTP | Wildcard CORS exposed local telemetry to other pages; shutdown accepted cross-origin requests | Origin, Host and Sec-Fetch-Site checks; loopback-only binding; no wildcard CORS |
| Static files | URL paths joined to cwd `web/dist` allowed traversal | Removed arbitrary filesystem fallback, rejected traversal, unknown APIs return 404 |
| Events | Row deduplication did not prevent repeated side effects | Check IDs before projections in one transaction; retries cannot reopen completed executions or double calls |
| Atomicity | Mid-processing failure left partial state | Whole-event transaction and rollback, including errors and unwinding |
| Migrations | DDL and version recording were separate | Transactional migrations and refusal of unknown newer schemas |
| Resume | A new wrapper-local ID placed execution outside the identified native session | Persistent aliases and canonicalization of subsequent events |
| Unknown IDs | Shared `sess_unknown`/`run_unknown` merged unrelated records | Separate unknown-entity IDs; validate IDs for key events |
| Lineage | An unimported parent broke the foreign key and lost the child | Keep the session; retain unconfirmed parent in payload |
| Ingestion | Errors were hidden behind 202 | Validate the batch; 422 with exact failed IDs and successful IDs for retries |
| Secrets | Regex over serialized JSON missed nested fields or broke escapes; titles were not scrubbed | Recursive JSON redaction before projections, additional patterns, config contents opt-in |
| Statistics | JOIN multiplied resume_count; textual time comparison mixed RFC3339 and SQLite datetime | DISTINCT resume bindings, julianday comparisons, safe large-page offsets |
| Git | trim shifted porcelain's first character; spaces, Unicode and renames broke parsing | NUL-delimited porcelain v1, correct renames, staged diff when HEAD exists |
| Attribution | A late snapshot could show OBSERVED for an AMBIGUOUS execution | Server controls attribution and propagates it to snapshots |
| Wrapper | Blocking HTTP ran inside async runtime; autostart always used 4242 | Blocking worker and configured port |
| Windows autostart | Background server inherited output pipes and blocked CLI capture | CreateProcessW without handle inheritance or a window; dedicated end-to-end test |
| Processes | PID identified wrapper; finished agents stayed RUNNING | Actual child PID, terminate agent records/bindings, incomplete executions become UNKNOWN |
| CLI metadata | `-s` misread as Codex/Copilot session; Codex used nonexistent config.json | Verified flags, resume/fork, config.toml; generic CLI does not interpret other tools' flags |
| UI | Process called a turn; Runner showed constant CLI; zero duration appeared unfinished | Explicit capture scope, corrected labels and zero durations, current version |
| Installation | Archives unpacked without checksums; latest errors fell back to an old release | Verify SHA-256 first, fail closed, unique temp directory and safe cleanup |
| Release workflow | workflow_dispatch version interpolated directly into shell | Pass through env and validate format |

## Product core

Overview and the review form connect observations to human outcomes, group experiments and export filtered retrospectives. JSON/JSONL import supports any agent through a documented contract. See [product direction](PRODUCT.md), [event API](EVENTS.md) and [runnable example](../examples/events.jsonl).

## Checks

- TypeScript/Vite and Rust builds.
- **64 Rust tests passed** at this stage: 12 unit, 13 regression, 5 CLI, 6 delivery, 16 integration, 12 native-source tests; includes real CLI → local HTTP → SQLite on a separate port/temp DB.
- SQL fault injection for rollback, v2 → v5 migration, retries, session aliases after engine recreation, counters and time filters.
- Hostile HTTP Origin/Host, traversal, unknown APIs, partial errors and validation before writes.
- Clippy with warnings denied and rustfmt; checks added to Windows/Linux/macOS CI.
- `cargo audit` and `npm audit` found no known vulnerabilities; removed a yanked transitive `yoke-derive` version from the lockfile.
- Browser checks on a separate DB: empty state, demo exclusion, populated Overview, saving reviews, updated cohorts, URL filters, export without browser errors and 390 px width without horizontal overflow. The PowerShell installer was syntax-checked; downloading/installing an actual release was not performed at this stage.

The local validation environment was Windows. Linux/macOS were configured in CI; remote CI had not yet been run and was not claimed as passing at this checkpoint.

## Explicit limits at this stage

- Wrappers observe process lifetime, not internal turns. They do not intercept stdout or recognize every IDE event. Codex has a native rollout adapter for supported boundaries; generic `ingest` still accepts our envelope, not arbitrary logs.
- A finished GUI launcher does not prove editor completion. Initial launch cwd does not prove internal cwd changes. Git attribution means observation, not authorship.
- Arguments/config do not prove the actual model serving requests. Wrappers do not observe model changes inside an interactive process.
- Durable outbox and 15-second heartbeat distinguish freshness from result. RUNNING means no recorded completion. Crashed agents/wrappers do not restart automatically; see [delivery limits](DELIVERY.md).
- Events require causal order. Late reconstruction of unknown parents, cost calculation, OTLP and other agents' logs were follow-up work at this stage. Codex token counters were already supported.
- Redaction is best effort, not absolute removal of all secrets. Databases, paths, prompts and notes can remain sensitive. Old records are not rewritten automatically.
- No server authentication for untrusted local users. Network hosting is deliberately forbidden.
- SQLite uses one connection; some older read handlers are synchronous. Large archives need load tests, read connections, timeline pagination and retention. Large real databases had not been measured at this stage.
- Migrations preserve older records as `capture_scope=UNKNOWN`; they do not repair incorrect historical relationships. Back up important existing data before updating. Old demo records are not automatically reclassified.

The audit fixes reproduced problems and adds regression protection; it does not prove the absence of all future bugs.

## Native collector checks

Retry/copy without duplicates; real server stop/restart; incomplete UTF-8; changed/truncated files; partial failure across files; SQL rollback with checkpoint; stale second-collector checkpoint; cancellation, bounded backlog, pause/resume/remove; fork/reset tokens; metadata-only and opt-in redaction; no false wrapper conflict. Browser checks covered source registration, pause/resume, token cards and narrow Sources without overflow. Tests used synthetic rollouts and separate databases, not personal history.

## Delivery and compact panel

Disk queue before HTTP, lost-ack replay, lease/CAS for competing consumers, runtime ordering, independent streams, persistent backoff and redaction before writes. Real CLI test: offline run exiting 7 → new server → completed delivery without another wrapper. A real long-running child confirmed periodic heartbeat and stopping observations after exit. Tests covered batch rollback, stale signals and refusal to reopen completed processes.

UI changes: mobile execution/session/experiment cards, compact navigation, density, polling pause, URL filters, session pagination, expandable diagnostics, source drawer, event search and lazy pages. Sessions beyond the first page became accessible. UI pause does not pause collection.

Visual checks at 320, 390, 768, 1024 and 1440 px found no document-wide horizontal overflow; wide tables/tabs scroll within their regions. Browser checks covered saving reviews, draft persistence across tabs, event search, return to URL filters, source drawer, density persistence, polling pause/resume and Monitor with a completed real test process. No console errors were found. Test records remain in a separate preview DB.

## Scale and recovery — 2026-10-03

Added reproducible HTTP/outbox benchmarks at 100k events, a cursor-paginated journal, transactional queue-size counter and SQLite online backup/verify/restore. Restore checks integrity, never overwrites paths and pauses collectors. See [measurements and limits](PERFORMANCE.md) and [backup procedures](BACKUPS.md). Detail APIs now return `events_total`; fetch history separately through the [event API](EVENTS.md).

## Pre-commit review — 2026-10-03

Rechecked projection/checkpoint transactions, native-event idempotency, outbox/backup, background shutdown, local installer, API and panel forms. Focused fixes:

- Repeated `session_meta` for the same thread no longer resets the legacy token baseline and double-charges the next cumulative sample. Regression coverage includes checkpoint persistence and database reopening. Thread changes within a file remain rejected.
- Unchanged `WAITING` files skip work until size/mtime changes, as READY already did. This avoids hashing the whole consumed prefix every poll. Appending resumes prefix validation and processing; partial UTF-8 is tested.
- Error-file counts use a SQL aggregate instead of rereading every parser checkpoint after each pass.
- `server start` exits nonzero when the started server is unreachable. An end-to-end test holds the port with another TCP listener.
- Windows updates wait for EXE release: HTTP can close before the collector finishes its batch. Copy retries are bounded to 30 seconds and Windows locking/access errors; other errors return immediately.
- Outbox SQLite files and journals were added to `.gitignore`.

This review added no migrations or content-policy changes. Mixed legacy fork rollouts remained explicitly unsupported at that checkpoint; source logs were unchanged. WAITING uses the same size/mtime contract as READY and does not claim to detect intentional replacement preserving both attributes.

Validation at this stage: `cargo test --release --all-targets --locked -j 1` passed 77 tests; `cargo clippy --release --all-targets --locked -j 1 -- -D warnings`, `cargo fmt --all -- --check`, production frontend build, `cargo audit`, `npm audit --audit-level=high` succeeded. PowerShell parsed the installer; a real local update used a verified backup and installed the exact tested EXE hash. After updating, the native backlog was empty except for three previously identified incompatible files. Remote CI was checked separately after push.

### Remote CI follow-up

The first push passed Linux/macOS CI, but Windows lost two of eight concurrent offline-wrapper observations. Added per-wrapper stderr checks instead of relying only on final execution counts. The queue had a short 500 ms SQLite busy timeout and lacked retries during competing initial WAL setup. Increased waiting to five seconds and retried BUSY/LOCKED initialization with fresh connections and bounded time. A new test holds a writer lock for 900 ms, then checks persistence, claim/ack and queue release. Exhausted waits remain explicit errors; no infinite retries or child exit-code changes were introduced.
