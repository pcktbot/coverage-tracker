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
  tfs_base_url: string;
  tfs_pat: string;
  tfs_collection: string;
  tfs_default_project: string;
  tfs_default_area_path: string;
  confluence_base_url: string;
  confluence_username: string;
  confluence_token: string;
  microsoft_tenant_id: string;
  microsoft_client_id: string;
  microsoft_client_secret: string;
  mcp_enabled: boolean;
  anthropic_api_key: string;
  anthropic_model: string;
  ai_system_prompt: string;
}

export interface AIChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

export interface ConfluenceSpace {
  id: string;
  key: string;
  name: string;
  homepage_id?: string;
}

export interface ConfluencePageSummary {
  id: string;
  title: string;
  space_key?: string;
  space_name?: string;
  web_url: string;
  excerpt: string;
  last_updated_at?: string;
}

export interface ConfluencePage {
  id: string;
  title: string;
  space_key?: string;
  space_name?: string;
  web_url: string;
  body_html: string;
  plain_text: string;
  excerpt: string;
  last_updated_at?: string;
  version_number?: number;
}

export interface CachedConfluencePage {
  page_id: string;
  space_key?: string;
  title: string;
  web_url: string;
  version_number?: number;
  last_updated_at?: string;
  last_fetched_at: string;
  raw_html: string;
  plain_text: string;
  excerpt: string;
}

export interface AdoWorkItem {
  id: number;
  title: string;
  state: string;
  work_item_type: string;
  area_path?: string;
  iteration_path?: string;
  assigned_to?: string;
  tags: string[];
  url: string;
}

export interface AdoReleaseDefinition {
  id: number;
  name: string;
  path?: string;
  url?: string;
}

export interface AdoRelease {
  id: number;
  name: string;
  status?: string;
  created_on?: string;
  modified_on?: string;
  definition_name?: string;
  web_url?: string;
}

export interface AdoPreview {
  base_url: string;
  collection: string;
  project: string;
  area_path: string;
  api_version: string;
  work_items: AdoWorkItem[];
  release_definitions: AdoReleaseDefinition[];
  releases: AdoRelease[];
}

export interface RepoDocSummary {
  path: string;
  title: string;
  preview: string;
  is_runbook: boolean;
  modified_at?: string;
}

export interface RepoDocContent {
  path: string;
  title: string;
  markdown: string;
  is_runbook: boolean;
  modified_at?: string;
}

export interface RepoBranch {
  name: string;
  is_current: boolean;
  is_remote: boolean;
}

export interface RepoBranchState {
  current_branch: string;
  branches: RepoBranch[];
}

export interface AuthCheck {
  ok: boolean;
  status: string;
  message: string;
  hint?: string;
}

export interface GithubAuthDiagnostics {
  token_present: boolean;
  org?: string;
  repo?: string;
  api: AuthCheck;
  git: AuthCheck;
}

export interface RepoSources {
  repo_id: number;
  platform_name?: string;
  tfs_project?: string;
  tfs_area_path?: string;
  tfs_team?: string;
  tfs_release_definition?: string;
  confluence_space_key?: string;
  confluence_parent_page_id?: string;
  confluence_site_label?: string;
  notes?: string;
}

export interface ProjectSummary {
  id: number;
  name: string;
  slug: string;
  status: string;
  platform_name?: string;
  manual_priority: number;
  is_active: boolean;
  repo_count: number;
  source_link_count: number;
  focus_score: number;
}

export interface Project {
  id: number;
  name: string;
  slug: string;
  status: string;
  platform_name?: string;
  manual_priority: number;
  notes?: string;
  ado_iteration_path?: string;
  ado_team?: string;
  ado_states: string[];
  ado_tag?: string;
  doc_refs: ProjectDocRef[];
  teams_team_id?: string;
  teams_channel_id?: string;
  teams_members: string[];
  loop_workspace_id?: string;
  loop_page_id?: string;
  is_active: boolean;
  linked_repo_ids: number[];
}

export interface ProjectDocRef {
  kind: string;
  label: string;
  github_org?: string;
  github_repo?: string;
  github_branch?: string;
  github_path?: string;
  confluence_space_key?: string;
  confluence_page_id?: string;
}

export interface AgentProfile {
  id: number;
  name: string;
  goal: string;
  instructions: string;
  source_types: string[];
  project_scope: string;
  weight_manual_priority: number;
  weight_release_risk: number;
  weight_doc_gap: number;
  weight_meeting_followup: number;
  is_active: boolean;
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
    tfsBaseUrl: s.tfs_base_url,
    tfsPat: s.tfs_pat,
    tfsCollection: s.tfs_collection,
    tfsDefaultProject: s.tfs_default_project,
    tfsDefaultAreaPath: s.tfs_default_area_path,
    confluenceBaseUrl: s.confluence_base_url,
    confluenceUsername: s.confluence_username,
    confluenceToken: s.confluence_token,
    microsoftTenantId: s.microsoft_tenant_id,
    microsoftClientId: s.microsoft_client_id,
    microsoftClientSecret: s.microsoft_client_secret,
    mcpEnabled: s.mcp_enabled,
    anthropicApiKey: s.anthropic_api_key,
    anthropicModel: s.anthropic_model,
    aiSystemPrompt: s.ai_system_prompt,
  });
  if (!r.ok) throw new Error(r.error);
}

export async function diagnoseGithubAuth(org?: string): Promise<GithubAuthDiagnostics> {
  const r: ApiResult<GithubAuthDiagnostics> = await invoke('diagnose_github_auth', {
    org: org ?? null,
  });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function sendAIMessage(
  messages: AIChatMessage[],
  contextLines: string[],
): Promise<string> {
  const r: ApiResult<string> = await invoke('send_ai_message', {
    messages,
    contextLines,
  });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function getConfluenceSpace(spaceKey: string): Promise<ConfluenceSpace> {
  const r: ApiResult<ConfluenceSpace> = await invoke('confluence_get_space', { spaceKey });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function getConfluencePage(pageId: string): Promise<ConfluencePage> {
  const r: ApiResult<ConfluencePage> = await invoke('confluence_get_page', { pageId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function searchConfluencePages(
  query: string,
  spaceKey?: string,
  limit?: number,
): Promise<ConfluencePageSummary[]> {
  const r: ApiResult<ConfluencePageSummary[]> = await invoke('confluence_search_pages', {
    query,
    spaceKey: spaceKey ?? null,
    limit: limit ?? null,
  });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function refreshConfluencePage(pageId: string): Promise<CachedConfluencePage> {
  const r: ApiResult<CachedConfluencePage> = await invoke('confluence_refresh_page', { pageId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function adoPreview(): Promise<AdoPreview> {
  const r: ApiResult<AdoPreview> = await invoke('ado_preview');
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function adoQueryProjectWorkItems(projectId: number): Promise<AdoWorkItem[]> {
  const r: ApiResult<AdoWorkItem[]> = await invoke('ado_query_project_work_items', { projectId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function adoListReleaseDefinitions(): Promise<AdoReleaseDefinition[]> {
  const r: ApiResult<AdoReleaseDefinition[]> = await invoke('ado_list_release_definitions');
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function adoListReleases(): Promise<AdoRelease[]> {
  const r: ApiResult<AdoRelease[]> = await invoke('ado_list_releases');
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function getCachedConfluencePage(pageId: string): Promise<CachedConfluencePage | null> {
  const r: ApiResult<CachedConfluencePage | null> = await invoke('confluence_get_cached_page', { pageId });
  if (!r.ok) throw new Error(r.error);
  return r.data ?? null;
}

export async function listCachedConfluencePages(pageIds: string[]): Promise<CachedConfluencePage[]> {
  const r: ApiResult<CachedConfluencePage[]> = await invoke('confluence_list_cached_pages', { pageIds });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
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

export async function listProjects(): Promise<ProjectSummary[]> {
  const r: ApiResult<ProjectSummary[]> = await invoke('list_projects');
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function getProject(projectId: number): Promise<Project> {
  const r: ApiResult<Project> = await invoke('get_project', { projectId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function createProject(name: string): Promise<number> {
  const r: ApiResult<number> = await invoke('create_project', { name });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function saveProject(project: Project): Promise<void> {
  const r: ApiResult<void> = await invoke('save_project', {
    projectId: project.id,
    name: project.name,
    status: project.status,
    platformName: project.platform_name ?? null,
    manualPriority: project.manual_priority,
    notes: project.notes ?? null,
    adoIterationPath: project.ado_iteration_path ?? null,
    adoTeam: project.ado_team ?? null,
    adoStates: project.ado_states,
    adoTag: project.ado_tag ?? null,
    docRefs: project.doc_refs,
    teamsTeamId: project.teams_team_id ?? null,
    teamsChannelId: project.teams_channel_id ?? null,
    teamsMembers: project.teams_members,
    loopWorkspaceId: project.loop_workspace_id ?? null,
    loopPageId: project.loop_page_id ?? null,
    isActive: project.is_active,
    linkedRepoIds: project.linked_repo_ids,
  });
  if (!r.ok) throw new Error(r.error);
}

export async function updateProjectStatus(projectId: number, status: string): Promise<void> {
  const r: ApiResult<void> = await invoke('update_project_status', { projectId, status });
  if (!r.ok) throw new Error(r.error);
}

export async function listAgentProfiles(): Promise<AgentProfile[]> {
  const r: ApiResult<AgentProfile[]> = await invoke('list_agent_profiles');
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function createAgentProfile(name: string): Promise<number> {
  const r: ApiResult<number> = await invoke('create_agent_profile', { name });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function saveAgentProfile(profile: AgentProfile): Promise<void> {
  const r: ApiResult<void> = await invoke('save_agent_profile', {
    profileId: profile.id,
    name: profile.name,
    goal: profile.goal,
    instructions: profile.instructions,
    sourceTypes: profile.source_types,
    projectScope: profile.project_scope,
    weightManualPriority: profile.weight_manual_priority,
    weightReleaseRisk: profile.weight_release_risk,
    weightDocGap: profile.weight_doc_gap,
    weightMeetingFollowup: profile.weight_meeting_followup,
    isActive: profile.is_active,
  });
  if (!r.ok) throw new Error(r.error);
}

export async function deleteAgentProfile(profileId: number): Promise<void> {
  const r: ApiResult<void> = await invoke('delete_agent_profile', { profileId });
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

export async function getRepoSources(repoId: number): Promise<RepoSources> {
  const r: ApiResult<RepoSources> = await invoke('get_repo_sources', { repoId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function saveRepoSources(sources: RepoSources): Promise<void> {
  const r: ApiResult<void> = await invoke('save_repo_sources', {
    repoId: sources.repo_id,
    platformName: sources.platform_name ?? null,
    tfsProject: sources.tfs_project ?? null,
    tfsAreaPath: sources.tfs_area_path ?? null,
    tfsTeam: sources.tfs_team ?? null,
    tfsReleaseDefinition: sources.tfs_release_definition ?? null,
    confluenceSpaceKey: sources.confluence_space_key ?? null,
    confluenceParentPageId: sources.confluence_parent_page_id ?? null,
    confluenceSiteLabel: sources.confluence_site_label ?? null,
    notes: sources.notes ?? null,
  });
  if (!r.ok) throw new Error(r.error);
}

export async function listRepoDocs(
  repoId: number,
  options?: { query?: string; runbooksOnly?: boolean },
): Promise<RepoDocSummary[]> {
  const r: ApiResult<RepoDocSummary[]> = await invoke('list_repo_docs', {
    repoId,
    query: options?.query ?? null,
    runbooksOnly: options?.runbooksOnly ?? false,
  });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function readRepoDoc(repoId: number, path: string): Promise<RepoDocContent> {
  const r: ApiResult<RepoDocContent> = await invoke('read_repo_doc', { repoId, path });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function listRepoBranches(repoId: number): Promise<RepoBranchState> {
  const r: ApiResult<RepoBranchState> = await invoke('list_repo_branches', { repoId });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

export async function checkoutRepoBranch(repoId: number, branchName: string): Promise<RepoBranchState> {
  const r: ApiResult<RepoBranchState> = await invoke('checkout_repo_branch', { repoId, branchName });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
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

// ── End-of-life tracking ──────────────────────────────────────────────────────

export interface EolCycle {
  runtime: string;
  cycle: string;
  release_date?: string;
  eol_date?: string;
  lts_date?: string;
  latest?: string;
  is_eol: boolean;
}

export interface EolStatus {
  cycle?: string;
  is_eol: boolean;
  eol_date?: string;
  has_lts: boolean;
  lts_date?: string;
}

/** Refresh cached EOL data from endoflife.date (no-ops if <24 h old). */
export async function refreshEol(): Promise<void> {
  const r: ApiResult<void> = await invoke('refresh_eol');
  if (!r.ok) throw new Error(r.error);
}

/** Check whether a specific runtime version is end-of-life. */
export async function checkEol(runtime: 'nodejs' | 'ruby', version: string): Promise<EolStatus> {
  const r: ApiResult<EolStatus> = await invoke('check_eol', { runtime, version });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}

/** List all known release cycles for a runtime. */
export async function listEolCycles(runtime: 'nodejs' | 'ruby'): Promise<EolCycle[]> {
  const r: ApiResult<EolCycle[]> = await invoke('list_eol_cycles', { runtime });
  if (!r.ok) throw new Error(r.error);
  return r.data!;
}
