<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { repos } from '$lib/stores/repos';
  import { getProject, saveProject, type Project } from '$lib/api';

  const STATUSES = ['incoming', 'needs_attention', 'waiting', 'in_motion', 'done_this_week'];

  let projectId = $derived(Number($page.params.id));
  let project = $state<Project | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let saved = $state(false);
  let error = $state('');

  onMount(async () => {
    try {
      project = await getProject(projectId);
    } catch (e: any) {
      error = e.message ?? String(e);
    } finally {
      loading = false;
    }
  });

  function toggleRepo(repoId: number, checked: boolean) {
    if (!project) return;
    const next = new Set(project.linked_repo_ids);
    if (checked) next.add(repoId);
    else next.delete(repoId);
    project = { ...project, linked_repo_ids: [...next] };
  }

  async function persist() {
    if (!project) return;
    saving = true;
    error = '';
    saved = false;
    try {
      await saveProject(project);
      saved = true;
      setTimeout(() => (saved = false), 2000);
    } catch (e: any) {
      error = e.message ?? String(e);
    } finally {
      saving = false;
    }
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
      <label class="toggle-row">
        <input type="checkbox" bind:checked={project.is_active} />
        <span>Active project</span>
      </label>
    </section>

    <section class="card section">
      <h2>Linked repos</h2>
      <div class="repo-list">
        {#each $repos as repo}
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
  .repo-list { display:grid; gap:0.5rem; max-height:420px; overflow:auto; }
  .repo-option, .toggle-row { display:flex; align-items:center; gap:0.5rem; font-size:0.875rem; }
  .notes { width:100%; resize:vertical; min-height:140px; padding:0.75rem; border:1px solid var(--border); border-radius:var(--radius-sm); font:inherit; }
  @media (max-width: 900px) { .project-grid { grid-template-columns:1fr; } }
</style>
