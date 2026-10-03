export interface Observation { runtime_id: string; runner: string; pid: number | null; status: string; observed_at: string | null; received_at: string | null; freshness: string; execution_id: string | null; cwd: string }
export interface PendingBatch { id: number; stream: string; events: number; created_at: number; attempts: number; next_attempt: number; blocked: boolean; last_error: string | null }
export interface Monitoring { runtimes: Observation[]; queue_available: boolean; pending_events: number; pending_batches: number; blocked_batches: number; batches: PendingBatch[]; heartbeat_seconds: number; stale_after_seconds: number }
export async function fetchMonitoring(): Promise<Monitoring> { const r = await fetch('/api/v1/monitoring'); if (!r.ok) throw new Error('Unable to refresh monitoring'); return r.json(); }
export async function retryDelivery() { const r = await fetch('/api/v1/outbox/retry', {method:'POST'}); if (!r.ok) throw new Error('Unable to retry delivery'); }
