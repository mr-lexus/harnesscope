# Large-history and recovery checks

[Documentation](../README.md#documentation)

> This document records the October 3, 2026 stage; statuses and plans below belong to that checkpoint. See the [capability matrix](CAPABILITIES.md) for current support and outstanding acceptance checks.

Measurements from 2026-10-03 on Windows with a local debug build and synthetic data; personal archives were not read. This is a reproducible check of specific scenarios, not a performance guarantee for every project. Release builds, Linux/macOS, millions of records and extended soak tests were not measured here.

## Long event journal

One session and execution with 10,000 / 100,000 events containing a 128-byte text field. Import uses atomic HTTP batches of 500 events. Latency is the median of five requests including client response reading and JSON decoding, reusing one HTTP connection. The scenario checks record counts and retries the final batch.

| Metric | Before | After |
| --- | ---: | ---: |
| Execution detail, 10k events | 563 ms / 4.31 MB | 2.2 ms / 1.42 KB |
| Execution detail, 100k | 5,537 ms / 43.37 MB | 14.0 ms / 1.42 KB |
| Session detail, 100k | 5,553 ms / 43.37 MB | 13.3 ms / 1.01 KB |
| Peak server working set, 100k | 269.3 MB | 26.3 MB |
| Import 100k | 37.5 s | 38.2 s |
| Main database after shutdown | 40.7 MB | 40.7 MB |

Sizes are decimal. Health, execution list and retrospective stayed around 0.6 / 0.8 / 1.5 ms in this scenario. It has just one execution; those numbers do not describe reports across 100,000 separate executions.

The journal was removed from periodically refreshed detail responses and loads only on the Events tab. The API uses arrival-ID cursor pages and type/source search. New events with old timestamps do not shift earlier pages. A 50-event page in the 100k history is 21.75 KB. Counts still require counting rows, and search scans the relevant history metadata.

The 100,003-record main database was copied while the server was running, restored to a new path, and checked for counts and integrity. Combined creation and verified restoration took 3.16 s.

## Offline queue

100,000 events, batches of 100, one stream, 128-byte text fields. After enqueueing, the queue is closed, reopened and delivered into a new main database. Final counts are verified.

| Metric | Before | After |
| --- | ---: | ---: |
| Enqueue all records | 53.0 s | 37.9 s |
| First 10k | 3.88 s | 3.74 s |
| Last 10k | 6.69 s | 3.97 s |
| Delivery after reopening | 40.2 s | 37.9 s |
| Enqueued / delivered | 100,000 / 100,000 | 100,000 / 100,000 |

Previously, budget checks reread every accumulated JSON batch on each write. UTF-8 payload size now uses a transactional counter maintained by SQLite triggers on insert, delete and update. Existing queues are counted once on upgrade. The 256 MiB limit and no-eviction policy remain. This limits payload, not SQLite files with indexes, free pages and WAL.

## Correctness checks

The suite at this stage contained 72 Rust tests, including eight simultaneous real offline wrappers exiting with code 7 and subsequent delivery; eight producers/four consumers across 3,200 events; stream ordering; lost acknowledgement after commit; redelivery; partial UTF-8 and native-source resume; backup during active writes and an uncommitted transaction; no-overwrite checks and refusal of corrupt, unrelated or future databases. Actual OS crashes and physical disk failures were not simulated.

## Reproduce

```sh
cd web
npm run build
cd ..
cargo build -j 1
python scripts/benchmark.py --exe target/debug/harnesscope.exe --output .audit/benchmark.json --check-backup
cargo run --example queue_benchmark -j 1 -- 100000
cargo test -j 1
```

Remove `.exe` on Linux/macOS. Adjust `--sizes 10000,100000` as needed; the script creates temporary databases, selects a free loopback port and stops its own server. Peak memory is measured only on Windows. Raw [benchmark results](benchmarks/benchmark-after.json) are stored alongside `benchmark-before.json`, `queue-before.json` and `queue-after.json`.

Further profiles: many short executions/cohorts, directories with thousands of files, large payloads, long offline periods and sustained concurrency. Sources diagnostics still returns all file metadata; session details still returns all its executions. These limits are separate from the bounded event journal.
