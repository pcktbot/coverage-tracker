import { writable } from 'svelte/store';
import type { CoverageRun, CoverageTrendPoint } from '$lib/api';

// Map of repoId → latest run
export const latestRuns = writable<Map<number, CoverageRun>>(new Map());

// Map of repoId → trend points
export const trends = writable<Map<number, CoverageTrendPoint[]>>(new Map());

// Set of repoIds currently running
export const runningRepos = writable<Set<number>>(new Set());

export function markRunning(repoId: number): void {
  runningRepos.update((s) => {
    s.add(repoId);
    return new Set(s);
  });
}

export function markDone(repoId: number, run: CoverageRun): void {
  runningRepos.update((s) => {
    s.delete(repoId);
    return new Set(s);
  });
  latestRuns.update((m) => {
    m.set(repoId, run);
    return new Map(m);
  });
}
