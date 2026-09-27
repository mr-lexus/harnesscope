import { Execution, EventItem } from '../execution/types';

export interface Session {
  id: string;
  runner_name: string;
  native_session_id: string;
  title: string | null;
  started_at: string;
  ended_at: string | null;
  status: string;
  created_at: string;
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
}

export interface SessionDetail {
  session: Session;
  bindings: RuntimeSessionBinding[];
  executions: Execution[];
  events: EventItem[];
}
