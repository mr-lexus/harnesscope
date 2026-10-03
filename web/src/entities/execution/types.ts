import type { Session } from '../session/types';

export interface Execution {
  id: string;
  session_id: string;
  runtime_id: string;
  native_execution_id: string;
  turn_index: number;
  prompt_summary: string | null;
  model: string;
  reasoning_effort: string;
  selected_agent_role: string;
  started_at: string;
  ended_at: string | null;
  duration_ms: number | null;
  status: 'RUNNING' | 'COMPLETED' | 'FAILED' | 'CANCELLED' | string;
  exit_code: number | null;
  error_message: string | null;
  repo_root: string | null;
  worktree_path: string | null;
  branch: string | null;
  head_sha: string | null;
  capture_scope: 'PROCESS' | 'TURN' | 'UNKNOWN' | 'DEMO';
  git_attribution: 'OBSERVED' | 'CORRELATED' | 'AMBIGUOUS' | 'UNKNOWN' | string;
}

export interface RuntimeInstance {
  id: string;
  runner_name: string;
  runner_version: string;
  surface: string;
  pid: number | null;
  hostname: string;
  os: string;
  cwd: string;
  command_line: string;
  started_at: string;
  ended_at: string | null;
  exit_code: number | null;
  status: string;
}

export interface AgentInstance {
  id: string;
  execution_id: string;
  parent_agent_id: string | null;
  agent_role: string;
  agent_name: string;
  model: string;
  started_at: string;
  ended_at: string | null;
  status: string;
}

export interface ExecutionComponent {
  execution_id: string;
  component_id: string;
  state: 'CONFIGURED' | 'DISCOVERED' | 'LOADED' | 'INVOKED' | 'OBSERVED' | 'UNKNOWN' | string;
  invocations_count: number;
  details_json: string | null;
  component_type?: string;
  component_name?: string;
  component_version?: string;
}

export interface ChangedFile {
  status: string;
  path: string;
}

export interface GitSnapshot {
  id: string;
  execution_id: string;
  snapshot_type: 'BEFORE' | 'AFTER' | string;
  captured_at: string;
  repo_root: string;
  worktree_path: string;
  branch: string;
  head_commit: string;
  is_dirty: boolean;
  changed_files_count: number;
  diff_stat: string | null;
  changed_files_json: string | null;
  attribution: string;
}

export interface EventItem {
  id?: number;
  event_id: string;
  timestamp: string;
  runtime_id: string | null;
  session_id: string | null;
  execution_id: string | null;
  agent_instance_id: string | null;
  event_type: string;
  source: string;
  payload_json: string;
}

export interface ExecutionDetail {
  usage: { input_tokens: number; cached_input_tokens: number; output_tokens: number; reasoning_output_tokens: number; total_tokens: number } | null;
  execution: Execution;
  runtime: RuntimeInstance | null;
  session: Session | null;
  agents: AgentInstance[];
  components: ExecutionComponent[];
  git_snapshots: GitSnapshot[];
  events_total: number;
}
