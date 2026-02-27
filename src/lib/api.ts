import { invoke } from '@tauri-apps/api/core';

export async function openInTerminal(repoId: number): Promise<void> {
  const res = await invoke<ApiResult<void>>('open_in_terminal', { repoId });
  if (!res.ok) throw new Error(res.error ?? 'Failed to open terminal');
}

export interface ApiResult<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

export interface Org {
  id: number;
  name: string;
  is_active: boolean;
}

export interface Repo {
  id: number;
  org: string;
  name: string;
  github_url: string;
  local_path?: string;
  ruby_version?: string;
  node_version?: string;
  enabled: boolean;
  last_synced_at?: string;
}

export interface CoverageRun {
  id: number;
  repo_id: number;
  started_at: string;
  completed_at?: string;
  status: 'running' | 'success' | 'failed' | 'interrupted';
  error_message?: string;
  overall_coverage?: number;
  lines_covered?: number;
  lines_total?: number;
}

export interface FileCoverage {
  id: number;
  run_id: number;
  file_path: string;
  coverage_percent?: number;
  lines_covered?: number;
  lines_total?: number;
  uncovered_lines: number[];
}

export interface CoverageTrendPoint {
  run_id: number;
  started_at: string;
  overall_coverage?: number;
  status: string;
}

export interface Settings {
  github_token: string;
  clone_root: string;
}

// ── Orgs ──────────────────────────────────────────────────────────────────────

export async function listOrgs(): Promise<Org[]> {
  const r: ApiResult<Org[]> = await invoke('list_orgs');
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function addOrg(name: string): Promise<void> {
  const r: ApiResult<void> = await invoke('add_org', { name });
  if (!r.ok) throw new Error(r.error);
}

export async function removeOrg(name: string): Promise<void> {
  const r: ApiResult<void> = await invoke('remove_org', { name });
  if (!r.ok) throw new Error(r.error);
}

export async function setActiveOrg(name: string): Promise<void> {
  const r: ApiResult<void> = await invoke('set_active_org', { name });
  if (!r.ok) throw new Error(r.error);
}

export async function getActiveOrg(): Promise<string | null> {
  const r: ApiResult<string | null> = await invoke('get_active_org');
  if (!r.ok) throw new Error(r.error);
  return r.data ?? null;
}

// ── Settings ──────────────────────────────────────────────────────────────────

export async function getSettings(): Promise<Settings> {
  const r: ApiResult<Settings> = await invoke('get_settings');
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function saveSettings(s: Settings): Promise<void> {
  const r: ApiResult<void> = await invoke('save_settings', {
    githubToken: s.github_token,
    cloneRoot: s.clone_root,
  });
  if (!r.ok) throw new Error(r.error);
}

// ── Repos ─────────────────────────────────────────────────────────────────────

export async function listRepos(org?: string): Promise<Repo[]> {
  const r: ApiResult<Repo[]> = await invoke('list_repos', { org: org ?? null });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function setRepoEnabled(id: number, enabled: boolean): Promise<void> {
  const r: ApiResult<void> = await invoke('set_repo_enabled', { id, enabled });
  if (!r.ok) throw new Error(r.error);
}

export async function syncOrgRepos(org: string): Promise<number> {
  const r: ApiResult<number> = await invoke('sync_org_repos', { org });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function cloneOrPullRepo(repoId: number): Promise<string> {
  const r: ApiResult<string> = await invoke('clone_or_pull_repo', { repoId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function readEnvFile(repoId: number): Promise<string> {
  const r: ApiResult<string> = await invoke('read_env_file', { repoId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function writeEnvFile(repoId: number, content: string): Promise<void> {
  const r: ApiResult<void> = await invoke('write_env_file', { repoId, content });
  if (!r.ok) throw new Error(r.error);
}

// ── Coverage ──────────────────────────────────────────────────────────────────

export async function runCoverage(repoId: number): Promise<number> {
  const r: ApiResult<number> = await invoke('run_coverage', { repoId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function listRuns(repoId: number): Promise<CoverageRun[]> {
  console.log('[api] listRuns invoke start, repoId=', repoId);
  const r: ApiResult<CoverageRun[]> = await invoke('list_runs', { repoId });
  console.log('[api] listRuns invoke done, ok=', r.ok, 'data length=', r.data?.length);
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function getTrend(repoId: number, limit = 20): Promise<CoverageTrendPoint[]> {
  console.log('[api] getTrend invoke start, repoId=', repoId);
  const r: ApiResult<CoverageTrendPoint[]> = await invoke('get_trend', { repoId, limit });
  console.log('[api] getTrend invoke done, ok=', r.ok, 'data length=', r.data?.length);
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function getFileCoverage(runId: number): Promise<FileCoverage[]> {
  console.log('[api] getFileCoverage invoke start, runId=', runId);
  const r: ApiResult<FileCoverage[]> = await invoke('get_file_coverage', { runId });
  console.log('[api] getFileCoverage invoke done, ok=', r.ok, 'data length=', r.data?.length);
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

// ── Export ────────────────────────────────────────────────────────────────────

export async function exportCsv(repoId?: number, includeFiles = false): Promise<string> {
  const r: ApiResult<string> = await invoke('export_csv', {
    repoId: repoId ?? null,
    includeFiles,
  });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

/** Trigger a CSV download in the browser window. */
export function downloadCsv(csv: string, filename: string): void {
  const blob = new Blob([csv], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
