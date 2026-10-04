# Harnesscope: product direction

[Documentation](../README.md#documentation)

> This document records the October 3, 2026 stage; statuses and plans below belong to that checkpoint. See the [capability matrix](CAPABILITIES.md) for current support and outstanding acceptance checks.

Research date: October 3, 2026.

## Audience and purpose

For developers using multiple AI agents who want to understand which workflow changes improve results: clearer instructions, a different model, worktree isolation, tests before implementation, tool selection, or a separate review stage.

The core loop: **record execution → review the result → record rework reasons → change one part of the workflow → compare similar tasks**. Run counts alone do not measure effectiveness. Exit code 0 does not prove the task was solved correctly. A discovered MCP server does not prove invocation.

## Related products

| Product | Strength in its official materials | Implication for Harnesscope |
| --- | --- | --- |
| [ccusage](https://ccusage.com/guide/) | Local token and estimated cost analysis across agents | Do not make cost the sole focus; add it only from verified usage events. |
| [Langfuse](https://langfuse.com/docs/evaluation/overview) | Tracing, quality evaluation, datasets and experiments | Connect technical evidence to outcomes; begin with human review without an LLM judge. |
| [AgentOps](https://www.agentops.ai/) | Agent execution observability and framework integrations | Support a common event contract beyond one CLI. |

These are reference points, not an exhaustive market review. Proposed positioning: **local coding-agent retrospectives with explicit evidence quality**. Validate the value through repeated real tasks instead of claiming uniqueness from a feature list.

## Implemented

- Overview: period, agent, project, observation scope, cohort comparisons and JSON export.
- Execution review: accepted, rework, rejected, unreviewed; notes and experiment labels.
- Review queue: unfinished records, errors, worktree overlap and missing reviews.
- Visible coverage of model identity, native session identity and observation boundaries.
- Separate `PROCESS`, `TURN`, `UNKNOWN`, `DEMO` scopes; demo excluded from Overview by default.
- Common HTTP/JSONL contract for external adapters, stable IDs and safe retries.

## Next product stages

1. **Two deep adapters instead of ten shallow integrations.** The first Codex rollout adapter is implemented: Sources, turns, tool calls, tokens and checkpoint/restart. See [formats and limits](NATIVE_SOURCES.md). Validate real logs from supported Codex versions and select a second agent people actually use. Support native logs or hooks with version-specific fixtures for turns, calls, usage, interruptions and lineage. Define identity guarantees and read permissions before adding an integration.
2. **Reliability under real workloads.** Disk queue, heartbeat, leases and Monitor are implemented; see [lifecycle](DELIVERY.md). Next: extended offline/restart scenarios and large databases. PID reuse does not merge runtimes; missing heartbeat does not mean task failure.
3. **Comparable experiments.** Task type, complexity, acceptance criteria, human rework time and test results. Separate process observations from real turns; show sample sizes and missing evidence.
4. **Practical history management.** Selected-event export, SQLite backup API, retention with deletion preview, search and recurring rework reasons.
5. **Team use after local usefulness.** Authentication, access control, consent for content transfer and a separate storage model. The current server deliberately does not support network exposure.

## A first useful retrospective

Choose one repository and a recurring task type. Record 10–20 runs, marking result usefulness and actual rework. Change one thing, such as requiring a failing test before a fix, and record a comparable group under a different experiment label. Compare accepted results, errors, duration and notes. Treat a small sample as evidence for the next experiment, not a model ranking.
