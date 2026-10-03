import { Execution, ExecutionDetail } from '../../entities/execution/types';
import { SessionWithStats, SessionDetail } from '../../entities/session/types';
import { StatsSummary, HealthMetrics } from '../../entities/stats/types';

export interface PaginatedResult<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export interface ExecutionFilterParams {
  page?: number;
  page_size?: number;
  period?: string;
  scope?: string;
  runner?: string;
  model?: string;
  agent?: string;
  branch?: string;
  session?: string;
  component?: string;
  status?: string;
}

export interface SessionFilterParams {
  page?: number;
  page_size?: number;
  runner?: string;
  status?: string;
}

const BASE_URL = '';

export async function fetchHealth(): Promise<HealthMetrics> {
  const res = await fetch(`${BASE_URL}/api/v1/health`);
  if (!res.ok) throw new Error(`Health check failed: ${res.statusText}`);
  return res.json();
}

export async function fetchExecutions(params: ExecutionFilterParams = {}): Promise<PaginatedResult<Execution>> {
  const query = new URLSearchParams();
  Object.entries(params).forEach(([key, val]) => {
    if (val !== undefined && val !== null && val !== '' && val !== 'ALL') {
      query.set(key, String(val));
    }
  });

  const res = await fetch(`${BASE_URL}/api/v1/executions?${query.toString()}`);
  if (!res.ok) throw new Error(`Failed to load executions: ${res.statusText}`);
  return res.json();
}

export async function fetchExecutionDetail(id: string): Promise<ExecutionDetail> {
  const res = await fetch(`${BASE_URL}/api/v1/executions/${encodeURIComponent(id)}`);
  if (!res.ok) throw new Error(`Failed to load execution detail: ${res.statusText}`);
  return res.json();
}

export async function fetchSessions(params: SessionFilterParams = {}): Promise<PaginatedResult<SessionWithStats>> {
  const query = new URLSearchParams();
  Object.entries(params).forEach(([key, val]) => {
    if (val !== undefined && val !== null && val !== '' && val !== 'ALL') {
      query.set(key, String(val));
    }
  });

  const res = await fetch(`${BASE_URL}/api/v1/sessions?${query.toString()}`);
  if (!res.ok) throw new Error(`Failed to load sessions: ${res.statusText}`);
  return res.json();
}

export async function fetchSessionDetail(id: string): Promise<SessionDetail> {
  const res = await fetch(`${BASE_URL}/api/v1/sessions/${encodeURIComponent(id)}`);
  if (!res.ok) throw new Error(`Failed to load session detail: ${res.statusText}`);
  return res.json();
}

export async function fetchStats(): Promise<StatsSummary> {
  const res = await fetch(`${BASE_URL}/api/v1/stats`);
  if (!res.ok) throw new Error(`Failed to load stats: ${res.statusText}`);
  return res.json();
}

export async function seedDemoData(): Promise<{ status: string; message: string }> {
  const res = await fetch(`${BASE_URL}/api/v1/demo/seed`, {
    method: 'POST',
  });
  if (!res.ok) throw new Error(`Failed to seed demo data: ${res.statusText}`);
  return res.json();
}
