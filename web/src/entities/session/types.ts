import { Execution } from '../execution/types';

export interface SessionConflict {
  id: string;
  session_id: string;
  conflicting_session_id: string | null;
  execution_id: string | null;
  conflict_type: string;
  severity: 'CRITICAL' | 'WARNING' | 'INFO' | string;
  detected_at: string;
  resolved_at: string | null;
  details_json: string | null;
}

export interface Session {
  id: string;
  runner_name: string;
  native_session_id: string;
  title: string | null;
  started_at: string;
  ended_at: string | null;
  status: string;
  created_at: string;
  parent_session_id?: string | null;
  fork_reason?: string | null;
  forked_at?: string | null;
}

export interface RuntimeSessionBinding {
  id: string;
  runtime_id: string;
  session_id: string;
  bound_at: string;
  unbound_at: string | null;
  reason: 'START' | 'RESUME' | 'ATTACH' | string;
}

export interface SessionWithStats extends Session {
  execution_count: number;
  runtime_count: number;
  resume_count: number;
  last_active_at: string;
  parent_session_id?: string | null;
  fork_reason?: string | null;
  forked_at?: string | null;
  conflicts_count?: number;
}

export interface SessionDetail {
  session: Session;
  bindings: RuntimeSessionBinding[];
  executions: Execution[];
  events_total: number;
  child_forks: Session[];
  conflicts: SessionConflict[];
}
