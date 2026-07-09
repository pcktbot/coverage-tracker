<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { enabledRepos } from '$lib/stores/repos';
  import { getProject, listCachedConfluencePages, refreshConfluencePage, saveProject, type CachedConfluencePage, type Project, type ProjectDocRef } from '$lib/api';

  const STATUSES = ['incoming', 'needs_attention', 'waiting', 'in_motion', 'done_this_week'];

  let projectId = $derived(Number($page.params.id));
  let project = $state<Project | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let saved = $state(false);
  let error = $state('');
  let confluenceCacheError = $state('');
  let refreshingConfluence = $state(false);
  let cachedConfluencePages = $state<CachedConfluencePage[]>([]);
  let newDocRef = $state<ProjectDocRef>({
    kind: 'github',
    label: '',
    github_org: '',
    github_repo: '',
    github_branch: '',
    github_path: '',
    confluence_space_key: '',
    confluence_page_id: '',
  });
  let teamsMembersInput = $state('');
  let adoStatesInput = $state('');

  onMount(async () => {
    try {
      project = await getProject(projectId);
      teamsMembersInput = project.teams_members.join(', ');
      adoStatesInput = project.ado_states.join(', ');
      await loadCachedConfluencePages(project);
    } catch (e: any) {
      error = e.message ?? String(e);
    } finally {
      loading = false;
    }
  });

  async function loadCachedConfluencePages(currentProject: Project) {
    const pageIds = currentProject.doc_refs
      .filter((ref) => ref.kind === 'confluence' && ref.confluence_page_id)
      .map((ref) => ref.confluence_page_id!)
      .filter(Boolean);
    cachedConfluencePages = pageIds.length > 0 ? await listCachedConfluencePages(pageIds) : [];
  }

  function toggleRepo(repoId: number, checked: boolean) {
    if (!project) return;
    const next = new Set(project.linked_repo_ids);
    if (checked) next.add(repoId);
    else next.delete(repoId);
    project = { ...project, linked_repo_ids: [...next] };
  }

  function addDocRef() {
    if (!project || !newDocRef.label.trim()) return;
    const nextRef = normalizeDocRef(newDocRef);
    project = { ...project, doc_refs: [...project.doc_refs, nextRef] };
    newDocRef = {
      kind: newDocRef.kind,
      label: '',
      github_org: '',
      github_repo: '',
      github_branch: '',
      github_path: '',
      confluence_space_key: '',
      confluence_page_id: '',
    };
  }

  function removeDocRef(index: number) {
    if (!project) return;
    project = { ...project, doc_refs: project.doc_refs.filter((_, i) => i !== index) };
  }

  function normalizeDocRef(ref: ProjectDocRef): ProjectDocRef {
    return {
      kind: ref.kind,
      label: ref.label.trim(),
      github_org: ref.github_org?.trim() || undefined,
      github_repo: ref.github_repo?.trim() || undefined,
      github_branch: ref.github_branch?.trim() || undefined,
      github_path: ref.github_path?.trim() || undefined,
      confluence_space_key: ref.confluence_space_key?.trim() || undefined,
      confluence_page_id: ref.confluence_page_id?.trim() || undefined,
    };
  }

  function describeDocRef(ref: ProjectDocRef): string {
    if (ref.kind === 'github') {
      return [ref.github_org, ref.github_repo, ref.github_branch, ref.github_path].filter(Boolean).join(' / ');
    }
    return [ref.confluence_space_key, ref.confluence_page_id].filter(Boolean).join(' / ');
  }

  async function persist() {
    if (!project) return;
    saving = true;
    error = '';
    saved = false;
    try {
      await saveProject(project);
      teamsMembersInput = project.teams_members.join(', ');
      adoStatesInput = project.ado_states.join(', ');
      saved = true;
      setTimeout(() => (saved = false), 2000);
    } catch (e: any) {
      error = e.message ?? String(e);
    } finally {
      saving = false;
    }
  }

  async function refreshConfluenceRefs() {
    if (!project) return;
    const refs = project.doc_refs.filter((ref) => ref.kind === 'confluence' && ref.confluence_page_id);
    if (refs.length === 0) return;
    refreshingConfluence = true;
    confluenceCacheError = '';
    try {
      await Promise.all(refs.map((ref) => refreshConfluencePage(ref.confluence_page_id!)));
      await loadCachedConfluencePages(project);
    } catch (e: any) {
      confluenceCacheError = e.message ?? String(e);
    } finally {
      refreshingConfluence = false;
    }
  }

  function updateTeamsMembers(value: string) {
    teamsMembersInput = value;
    if (!project) return;
    project = {
      ...project,
      teams_members: value
        .split(',')
        .map((member) => member.trim())
        .filter(Boolean),
    };
  }

  function updateAdoStates(value: string) {
    adoStatesInput = value;
    if (!project) return;
    project = {
      ...project,
      ado_states: value
        .split(',')
        .map((state) => state.trim())
        .filter(Boolean),
    };
  }
</script>

<div class="page-header">
  <div>
    <a href="/" class="back-link">← Command center</a>
    <h1>{project?.name ?? 'Project'}</h1>
  </div>
  <div class="header-actions">
    {#if saved}<span class="badge badge-green">Saved</span>{/if}
    <button class="btn-primary" onclick={persist} disabled={saving || !project}>
      {saving ? 'Saving…' : 'Save project'}
    </button>
  </div>
</div>

{#if error}
  <div class="error-msg" style="margin-bottom:1rem">{error}</div>
{/if}

{#if loading}
  <p class="text-muted">Loading project…</p>
{:else if project}
  <div class="project-grid">
    <section class="card section">
      <h2>Details</h2>
      <div class="form-group">
        <label for="project-name">Name</label>
        <input id="project-name" bind:value={project.name} />
      </div>
      <div class="form-group">
        <label for="project-platform">Platform</label>
        <input id="project-platform" bind:value={project.platform_name} placeholder="payments-platform" />
      </div>
      <div class="form-group">
        <label for="project-status">Leadership flow</label>
        <select id="project-status" bind:value={project.status}>
          {#each STATUSES as status}
            <option value={status}>{status.replaceAll('_', ' ')}</option>
          {/each}
        </select>
      </div>
      <div class="form-group">
        <label for="project-priority">Manual priority</label>
        <input id="project-priority" type="number" min="0" max="10" bind:value={project.manual_priority} />
      </div>
      <div class="toggle-field">
        <label for="project-active" class="toggle-label">Active project</label>
        <label class="toggle-row" for="project-active">
          <input id="project-active" type="checkbox" bind:checked={project.is_active} />
          <span>{project.is_active ? 'On' : 'Off'}</span>
        </label>
      </div>
    </section>

    <section class="card section">
      <h2>ADO scope</h2>
      <div class="form-group">
        <label for="ado-iteration-path">Iteration path</label>
        <input id="ado-iteration-path" bind:value={project.ado_iteration_path} placeholder="Consumer Soutions\\FY26\\Q2\\Sprint 18" />
      </div>
      <div class="form-group">
        <label for="ado-team">Team</label>
        <input id="ado-team" bind:value={project.ado_team} placeholder="Content Gen" />
      </div>
      <div class="form-group">
        <label for="ado-states">Included states</label>
        <input
          id="ado-states"
          value={adoStatesInput}
          oninput={(e) => updateAdoStates((e.currentTarget as HTMLInputElement).value)}
          placeholder="New, Active, Committed, In Progress, Blocked"
        />
      </div>
      <div class="form-group" style="margin-bottom:0">
        <label for="ado-tag">Fallback tag</label>
        <input id="ado-tag" bind:value={project.ado_tag} placeholder="ai_content_gen" />
        <p class="text-muted section-hint">Project and area defaults come from Settings. Per project, the main binding is iteration path.</p>
      </div>
    </section>

    <section class="card section">
      <h2>Document references</h2>
      <div class="doc-ref-builder">
        <div class="form-group">
          <label for="doc-ref-kind">Source</label>
          <select id="doc-ref-kind" bind:value={newDocRef.kind}>
            <option value="github">GitHub / repo doc</option>
            <option value="confluence">Confluence page</option>
          </select>
        </div>
        <div class="form-group">
          <label for="doc-ref-label">Label</label>
          <input id="doc-ref-label" bind:value={newDocRef.label} placeholder="Runbook, architecture overview, incident notes" />
        </div>
        {#if newDocRef.kind === 'github'}
          <div class="doc-ref-grid">
            <div class="form-group">
              <label for="doc-github-org">GitHub org</label>
              <input id="doc-github-org" bind:value={newDocRef.github_org} placeholder="g5search" />
            </div>
            <div class="form-group">
              <label for="doc-github-repo">Repo name</label>
              <input id="doc-github-repo" bind:value={newDocRef.github_repo} placeholder="my-service" />
            </div>
            <div class="form-group">
              <label for="doc-github-branch">Branch</label>
              <input id="doc-github-branch" bind:value={newDocRef.github_branch} placeholder="main" />
            </div>
            <div class="form-group">
              <label for="doc-github-path">Doc path</label>
              <input id="doc-github-path" bind:value={newDocRef.github_path} placeholder="docs/runbooks/deploy.md" />
            </div>
          </div>
        {:else}
          <div class="doc-ref-grid">
            <div class="form-group">
              <label for="doc-conf-space">Confluence space</label>
              <input id="doc-conf-space" bind:value={newDocRef.confluence_space_key} placeholder="PLAT" />
            </div>
            <div class="form-group">
              <label for="doc-conf-page">Confluence page ID</label>
              <input id="doc-conf-page" bind:value={newDocRef.confluence_page_id} placeholder="123456789" />
            </div>
          </div>
        {/if}
        <div class="doc-ref-actions">
          <button class="btn-secondary" onclick={addDocRef} disabled={!newDocRef.label.trim()}>
            Add reference
          </button>
        </div>
      </div>
      {#if project.doc_refs.length > 0}
        <div class="doc-ref-list">
          {#each project.doc_refs as ref, index}
            <div class="doc-ref-row">
              <div>
                <strong>{ref.label}</strong>
                <div class="text-muted" style="margin-top:0.2rem;font-size:0.75rem">{ref.kind} · {describeDocRef(ref)}</div>
              </div>
              <button class="btn-ghost" onclick={() => removeDocRef(index)}>Remove</button>
            </div>
          {/each}
        </div>
      {/if}
      <p class="text-muted section-hint">Use structured document identifiers instead of raw URLs. GitHub refs capture org, repo, branch, and path; Confluence refs capture space and page ID.</p>
      {#if project.doc_refs.some((ref) => ref.kind === 'confluence' && ref.confluence_page_id)}
        <div class="confluence-cache">
          <div class="confluence-cache-header">
            <strong>Confluence cache</strong>
            <button class="btn-secondary" onclick={refreshConfluenceRefs} disabled={refreshingConfluence}>
              {refreshingConfluence ? 'Refreshing…' : 'Refresh Confluence pages'}
            </button>
          </div>
          {#if confluenceCacheError}
            <div class="error-msg" style="margin-top:0.75rem">{confluenceCacheError}</div>
          {/if}
          {#if cachedConfluencePages.length > 0}
            <div class="doc-ref-list" style="margin-top:0.75rem">
              {#each cachedConfluencePages as cachedPage}
                <div class="doc-ref-row">
                  <div>
                    <strong>{cachedPage.title}</strong>
                    <div class="text-muted" style="margin-top:0.2rem;font-size:0.75rem">
                      page {cachedPage.page_id} · fetched {cachedPage.last_fetched_at}
                    </div>
                    <div style="margin-top:0.35rem;font-size:0.8125rem">{cachedPage.excerpt}</div>
                  </div>
                  <a class="btn-ghost" href={cachedPage.web_url} target="_blank" rel="noreferrer">Open</a>
                </div>
              {/each}
            </div>
          {:else}
            <p class="text-muted" style="margin-top:0.75rem">No cached Confluence pages yet.</p>
          {/if}
        </div>
      {/if}
    </section>

    <section class="card section">
      <h2>Linked repos</h2>
      <div class="repo-list">
        {#each $enabledRepos as repo}
          <label class="repo-option">
            <input
              type="checkbox"
              checked={project.linked_repo_ids.includes(repo.id)}
              onchange={(e) => toggleRepo(repo.id, (e.currentTarget as HTMLInputElement).checked)}
            />
            <span>{repo.org}/{repo.name}</span>
          </label>
        {/each}
      </div>
    </section>
  </div>

  <section class="card section" style="margin-top:1rem">
    <h2>Collaboration sources</h2>
    <div class="doc-ref-grid">
      <div class="form-group">
        <label for="teams-team-id">Teams team ID</label>
        <input id="teams-team-id" bind:value={project.teams_team_id} placeholder="19:...@thread.tacv2 or team GUID" />
      </div>
      <div class="form-group">
        <label for="teams-channel-id">Teams channel ID</label>
        <input id="teams-channel-id" bind:value={project.teams_channel_id} placeholder="19:...@thread.tacv2" />
      </div>
      <div class="form-group">
        <label for="loop-workspace-id">Loop workspace ID</label>
        <input id="loop-workspace-id" bind:value={project.loop_workspace_id} placeholder="Loop workspace identifier" />
      </div>
      <div class="form-group">
        <label for="loop-page-id">Loop page ID</label>
        <input id="loop-page-id" bind:value={project.loop_page_id} placeholder="Loop page or component identifier" />
      </div>
    </div>
    <div class="form-group" style="margin-bottom:0">
      <label for="teams-members">Teams members</label>
      <input
        id="teams-members"
        value={teamsMembersInput}
        oninput={(e) => updateTeamsMembers((e.currentTarget as HTMLInputElement).value)}
        placeholder="name@example.com, teammate@example.com"
      />
      <p class="text-muted section-hint">Staged manual membership list for project ownership and later Teams data pulls.</p>
    </div>
  </section>

  <section class="card section" style="margin-top:1rem">
    <h2>Notes</h2>
    <textarea class="notes" bind:value={project.notes} rows="6" placeholder="Project intent, risks, cross-team constraints, meeting follow-ups, and doc gaps."></textarea>
  </section>
{/if}

<style>
  .page-header { display:flex; justify-content:space-between; align-items:flex-start; gap:1rem; margin-bottom:1rem; }
  .header-actions { display:flex; align-items:center; gap:0.5rem; }
  .back-link { display:inline-block; margin-bottom:0.3rem; color:var(--text-secondary); text-decoration:none; }
  .back-link:hover { color:var(--accent); }
  .project-grid { display:grid; grid-template-columns: 1fr 1fr; gap:1rem; }
  .section { padding:1rem; }
  .section h2 { margin-bottom:0.75rem; }
  .section-hint { margin:0.5rem 0 0.75rem; font-size:0.75rem; }
  .doc-ref-builder {
    display:grid;
    gap:0.75rem;
    margin-bottom:0.75rem;
  }
  .doc-ref-grid {
    display:grid;
    grid-template-columns:repeat(2, minmax(0, 1fr));
    gap:0.75rem;
  }
  .doc-ref-actions {
    display:flex;
    justify-content:flex-end;
  }
  .repo-list { display:grid; gap:0.5rem; max-height:420px; overflow:auto; }
  .repo-option, .toggle-row { display:flex; align-items:center; gap:0.5rem; font-size:0.875rem; }
  .toggle-field {
    display:flex;
    align-items:center;
    justify-content:space-between;
    gap:1rem;
    padding:0.625rem 0.75rem;
    border:1px solid var(--border);
    border-radius:var(--radius-sm);
    background:var(--bg-subtle);
  }
  .toggle-label {
    margin:0;
    color:var(--text);
    font-size:0.875rem;
  }
  .doc-ref-list {
    display:grid;
    gap:0.5rem;
    margin-bottom:0.5rem;
  }
  .doc-ref-row {
    display:flex;
    align-items:center;
    justify-content:space-between;
    gap:0.75rem;
    padding:0.625rem 0.75rem;
    border:1px solid var(--border);
    border-radius:var(--radius-sm);
    background:var(--bg);
  }
  .confluence-cache {
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border-subtle);
  }
  .confluence-cache-header {
    display:flex;
    justify-content:space-between;
    align-items:center;
    gap:0.75rem;
  }
  .notes { width:100%; resize:vertical; min-height:140px; padding:0.75rem; border:1px solid var(--border); border-radius:var(--radius-sm); font:inherit; }
  @media (max-width: 900px) { .project-grid { grid-template-columns:1fr; } }
  @media (max-width: 640px) {
    .toggle-field,
    .doc-ref-row { flex-direction:column; align-items:flex-start; }
    .doc-ref-grid { grid-template-columns:1fr; }
  }
</style>
