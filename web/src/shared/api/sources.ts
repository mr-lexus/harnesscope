export interface SourceFile {
  path: string;
  byte_offset: number;
  line_number: number;
  file_size: number;
  events_count: number;
  ignored_count: number;
  status: 'NEW' | 'READY' | 'BACKLOG' | 'WAITING' | 'ERROR';
  last_error: string | null;
  updated_at: string;
}

export interface SourceEntry {
  source: {
    id: string;
    adapter: string;
    path: string;
    enabled: boolean;
    include_content: boolean;
    last_scan_at: string | null;
    last_success_at: string | null;
    last_error: string | null;
  };
  files: SourceFile[];
}

async function request<T>(path = '', init?: RequestInit): Promise<T> {
  const response = await fetch(`/api/v1/sources${path}`, init);
  const body = await response.json();
  if (!response.ok) throw new Error(body.error ?? `Request failed (${response.status})`);
  return body;
}

export const fetchSources = () => request<{ items: SourceEntry[]; poll_interval_seconds: number }>();
export const addSource = (input: { path: string; include_content: boolean }) => request('', {
  method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(input),
});
export const setSourceEnabled = (id: string, enabled: boolean) => request(`/${encodeURIComponent(id)}`, {
  method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ enabled }),
});
export const removeSource = (id: string) => request(`/${encodeURIComponent(id)}`, { method: 'DELETE' });
export const setSourceContent = (id: string, include_content:boolean) => request(`/${encodeURIComponent(id)}`, { method: 'PATCH',headers:{'Content-Type':'application/json'},body:JSON.stringify({include_content}) });


export interface CollectionStatus {
  status: 'NOT_CONNECTED' | 'PAUSED' | 'NEEDS_ATTENTION' | 'OVERDUE' | 'STARTING' | 'CATCHING_UP' | 'COLLECTING';
  connected_sources: number; enabled_sources: number; files_inspected: number;
  backlog_files: number; error_files: number; converted_events: number;
  last_scan_at: string | null; detected_codex_path: string | null; database_path: string | null;
}
export async function fetchCollection(): Promise<CollectionStatus> {
  const response=await fetch('/api/v1/collection');
  if (!response.ok) throw new Error('Collection status unavailable');
  return response.json();
}
export const collectionLabel = {
  NOT_CONNECTED:'Not connected', PAUSED:'Collection paused', NEEDS_ATTENTION:'Needs attention',
  OVERDUE:'Scan overdue', STARTING:'Starting collection', CATCHING_UP:'Importing history', COLLECTING:'Collection active',
};
