# Connect a task tracker through observed MCP calls

[Documentation](../README.md#documentation)

Harnesscope has no built-in task-tracker provider. Configure the tracker MCP in
your agent, using that provider's own authentication. Harnesscope observes the
agent's captured calls/results; it does not connect to the tracker, poll tasks,
install an MCP server, or store provider credentials.

## Register a local mapping

Copy [the neutral example](../examples/task-adapter.json) into
`task-adapters.local/read-task.json` (gitignored). Replace the tool name and field
paths with the ones actually present in a sanitized observation:

```json
{
  "schema_version": 1,
  "id": "tracker-read-task",
  "namespace": "team-tracker",
  "tool": "mcp__tracker__get_task",
  "task_id": { "source": "arguments", "pointer": "/taskId" },
  "title_pointer": "/title"
}
```

```sh
harnesscope task-adapters add task-adapters.local/read-task.json
harnesscope task-adapters list
# Optional: derive task links from already archived observations.
harnesscope evidence reindex
# Disable new associations without deleting historical evidence or human marks.
harnesscope task-adapters remove tracker-read-task
```

Use the same `HARNESSCOPE_DB_PATH` as the collector. Registration is stored in the
database and takes effect without restarting it. No adapter is enabled by default.
Configuration files are limited to 16 KiB. Unknown fields, unsafe content, invalid
pointers and unsupported versions are rejected; do not put secrets in this file.

## Mapping contract (version 1)

| Field | Meaning |
| --- | --- |
| `id` | Immutable local mapping name: 1–80 ASCII letters, digits, `.`, `_`, `-`. |
| `namespace` | Tracker/account/workspace scope with the same syntax. Distinct scopes must use distinct namespaces. Multiple tools for the same tracker may share it. |
| `tool` | Exact observed tool name, case-sensitive. No substring matching or wildcards. Only one enabled mapping per tool. |
| `task_id.source` | `arguments` or `result`. |
| `task_id.pointer` | RFC 6901 JSON Pointer to one stable task ID. Supports nested fields, array indexes, `~1` for `/`, and `~0` for `~`. Empty string selects the root. |
| `title_pointer` | Optional pointer into the decoded result for a display title. Missing title falls back to the namespaced ID. |

Arguments may be an object or JSON-encoded string. Results may be plain JSON,
JSON-encoded strings, MCP `structuredContent`, or a single MCP text block containing
JSON/text. Pointers into results are evaluated **after** this envelope decoding.
For example, with `structuredContent: {task: {id: 42, title: "Fix tests"}}`, use
`task_id: {source: "result", pointer: "/task/id"}` and `title_pointer: "/task/title"`.
String and integer IDs are supported. Redacted, missing and compound IDs are rejected.

Task identity is `mcp:<namespace>:<native-id>`. A matching invocation and response
must share a native call ID and session; conflicting known turn IDs do not match.
Only the most recent matching invocation is considered, including when it belongs
to an unregistered tool. Known error results do not become task versions.
Post-tool hooks containing the tool name, arguments, call ID and response provide
both sides in one observation; a separate pre-tool record is not required.
Use task-reading tools: Harnesscope does not infer whether an arbitrary custom
response semantically means success, nor verify the provider's task freshness.

Each captured response retains its observation, object hash and observed time.
Changed requirements/comments therefore remain separate versions. Redelivery of
the same observation and reindexing do not duplicate versions. The full sanitized
response stays available; mapping extracts identity and initial display title,
without reducing the task's detailed content. A title in the task list is the
first observed title; subsequent titles remain in version evidence.

## Boundaries and lifecycle

- This mapping handles one task per tool result. For list/batch results, use a
  single-task read tool or a private bridge that exposes one task per call. A fixed
  array pointer can select one item; it does not iterate a list automatically.
- A result without a captured invocation, stable ID, safe content, or known
  envelope remains ordinary evidence. Text mentions never confirm task links.
- Mappings cannot recover data absent from agent telemetry. Enable sanitized
  evidence collection first. Supported envelopes include native `response_item`,
  `item_completed` payload items, and hook `tool_input`/`tool_response` records.
- Registering a different mapping under an existing ID is rejected. Disable the
  old mapping and create a new ID to change interpretation. Historical task links
  and versions are preserved; reindexing adds newly supported associations rather
  than silently rewriting prior task identities or human assessments.
- SQLite backup/restore includes mappings. Portable retrospective packages include
  task evidence and links, but do not install mapping configuration on another host.
- Private tool names can occur in locally collected evidence and its exports.
  Review packages before sharing. Keeping private configuration out of Git does
  not make captured task content public-safe automatically.

Test with fictitious tasks: read, change requirements/comments, read again, verify
two versions, restart/reindex and confirm counts remain stable. Also test an error
response and identical task IDs in two namespaces. See [acceptance](ACCEPTANCE.md).
