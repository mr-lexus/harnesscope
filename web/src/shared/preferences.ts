import { useSyncExternalStore } from 'react';
const listeners = new Set<() => void>();
const readDensity = () => { try { return localStorage.getItem('harnesscope-density') !== 'comfortable'; } catch { return true; } };
let compact = readDensity();
let live = true;
const subscribe = (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; };
const notify = () => listeners.forEach(fn => fn());
document.documentElement.dataset.density = compact ? 'compact' : 'comfortable';
export function usePreferences() {
  return { compact: useSyncExternalStore(subscribe, () => compact), live: useSyncExternalStore(subscribe, () => live) };
}
export function toggleDensity() { compact = !compact; document.documentElement.dataset.density = compact ? 'compact' : 'comfortable'; try { localStorage.setItem('harnesscope-density', compact ? 'compact' : 'comfortable'); } catch { /* Storage is optional. */ } notify(); }
export function toggleLive() { live = !live; notify(); }
export function useRefreshInterval(): number | false { return useSyncExternalStore(subscribe, () => live) ? 5000 : false; }
