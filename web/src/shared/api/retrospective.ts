export interface Review {
  outcome: 'UNREVIEWED' | 'ACCEPTED' | 'REWORK' | 'REJECTED';
  notes: string;
  experiment: string;
  updated_at?: string;
}
export interface RetrospectiveFilter { days: number; runner?: string; project?: string; scope?: string; include_demo: boolean }
export interface Cohort {
  runner: string; model: string; scope: string; experiment: string;
  executions: number; completed: number; failed: number; reviewed: number;
  accepted: number; rework: number; rejected: number; avg_duration_ms: number | null;
}
export interface Retrospective {
  total: number; running: number; failed: number; reviewed: number; accepted: number;
  unknown_model: number; unknown_session: number; ambiguous_git: number;
  process_captures: number; turn_captures: number; unknown_scope: number;
  cohorts: Cohort[];
  attention: { id: string; runner: string; status: string; started_at: string; prompt_summary: string | null; reason: string }[];
  runners: string[]; projects: string[]; last_event_at: string | null;
}

async function json<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api/v1${path}`, init);
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.error ?? `Request failed (${response.status})`);
  }
  return response.json();
}
export function fetchRetrospective(filter: RetrospectiveFilter): Promise<Retrospective> {
  const query = new URLSearchParams();
  Object.entries(filter).forEach(([key,value]) => { if (value !== undefined && value !== '') query.set(key, String(value)); });
  return json(`/retrospective?${query}`);
}
export const fetchReview = (id: string) => json<Review | null>(`/executions/${encodeURIComponent(id)}/review`);
export const saveReview = (id: string, review: Review) => json(`/executions/${encodeURIComponent(id)}/review`, {
  method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(review),
});

export function downloadReport(data: Retrospective, filters: RetrospectiveFilter) {
  const blob = new Blob([JSON.stringify({ schema_version: 1, exported_at: new Date().toISOString(), filters, report: data }, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a'); anchor.href = url;
  anchor.download = `harnesscope-retrospective-${new Date().toISOString().slice(0, 10)}.json`;
  anchor.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
}
