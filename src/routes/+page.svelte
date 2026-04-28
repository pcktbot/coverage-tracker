<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { activeOrg } from '$lib/stores/repos';
  import {
    createProject,
    listAgentProfiles,
    listProjects,
    type AgentProfile,
    type ProjectSummary,
  } from '$lib/api';

  let error = $state('');
  let loading = $state(true);
  let creating = $state(false);
  let projects = $state<ProjectSummary[]>([]);
  let agentProfiles = $state<AgentProfile[]>([]);

  const FLOW_COLUMNS = ['incoming', 'needs_attention', 'waiting', 'in_motion', 'done_this_week'];

  onMount(async () => {
    await load();
  });

  async function load() {
    loading = true;
    error = '';
    try {
      const [projectList, profiles] = await Promise.all([
        listProjects(),
        listAgentProfiles(),
      ]);
      projects = projectList;
      agentProfiles = profiles;
    } catch (e: any) {
      error = e.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  async function createNewProject() {
    creating = true;
    error = '';
    try {
      const projectId = await createProject(`New Project ${projects.length + 1}`);
      await goto(`/projects/${projectId}`);
    } catch (e: any) {
      error = e.message ?? String(e);
    } finally {
      creating = false;
    }
  }

  function projectsFor(status: string) {
    return projects.filter((project) => project.status === status);
  }

  function labelFor(status: string) {
    return status.replaceAll('_', ' ');
  }

</script>

<div class="page-header">
  <div>
    <h1>Command Center</h1>
    <p class="text-secondary" style="margin:0.35rem 0 0">Focus on active projects, connected sources, and advisor profiles across {$activeOrg ?? 'your orgs'}.</p>
  </div>
  <div class="header-actions">
    <button class="btn-primary" onclick={createNewProject} disabled={creating}>
      {creating ? 'Creating…' : 'New project'}
    </button>
  </div>
</div>

{#if error}
  <div class="error-msg" style="margin-bottom:1rem">{error}</div>
{/if}

{#if loading}
  <div class="empty">
    <p class="text-muted">Loading command center…</p>
  </div>
{:else if projects.length === 0}
  <div class="empty">
    <p class="text-secondary">No projects yet.</p>
    <p class="text-muted">Create a project and start linking repos, TFS/ADO ownership, meeting follow-ups, and docs into one focus view.</p>
  </div>
{:else}
  <div class="summary-grid">
    <section class="card summary-card">
      <h2>What Needs Attention</h2>
      <div class="priority-list">
        {#each [...projects].sort((a, b) => b.focus_score - a.focus_score).slice(0, 5) as project}
          <button class="priority-item" onclick={() => goto(`/projects/${project.id}`)}>
            <div>
              <strong>{project.name}</strong>
              <div class="text-muted">{project.platform_name ?? 'No platform'} · {labelFor(project.status)}</div>
            </div>
            <span class="badge badge-yellow">score {project.focus_score}</span>
          </button>
        {/each}
      </div>
    </section>

    <section class="card summary-card">
      <h2>Advisor Profiles</h2>
      <div class="profile-list">
        {#each agentProfiles as profile}
          <div class="profile-item">
            <strong>{profile.name}</strong>
            <div class="text-muted">{profile.project_scope} · {profile.source_types.length} source types</div>
          </div>
        {/each}
      </div>
    </section>
  </div>

  <section class="kanban-section">
    <div class="kanban-header">
      <h2>Leadership Flow</h2>
      <span class="text-muted">{projects.length} tracked projects</span>
    </div>
    <div class="kanban-board">
      {#each FLOW_COLUMNS as status}
        <div class="kanban-column card">
          <div class="column-header">
            <h3>{labelFor(status)}</h3>
            <span class="badge badge-gray">{projectsFor(status).length}</span>
          </div>
          <div class="column-body">
            {#each projectsFor(status) as project}
              <button class="project-card" onclick={() => goto(`/projects/${project.id}`)}>
                <strong>{project.name}</strong>
                <div class="text-muted">{project.platform_name ?? 'No platform'}</div>
                <div class="project-meta">
                  <span>{project.repo_count} repos</span>
                  <span>{project.source_link_count} source links</span>
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </section>
{/if}

<style>
  .page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.25rem; }
  .header-actions { display: flex; gap: 0.5rem; }
  .empty { padding: 3rem 1rem; text-align: center; }
  .summary-grid { display:grid; grid-template-columns: 1.1fr 0.9fr; gap:1rem; margin-bottom:1rem; }
  .summary-card { padding:1rem; }
  .priority-list, .profile-list { display:grid; gap:0.5rem; margin-top:0.75rem; }
  .priority-item, .profile-item, .project-card {
    width:100%; text-align:left; padding:0.75rem; border:1px solid var(--border);
    border-radius:var(--radius-sm); background:var(--bg); color:inherit;
  }
  .priority-item:hover, .project-card:hover { background:var(--bg-subtle); }
  .kanban-section { min-width:0; }
  .kanban-header { display:flex; justify-content:space-between; align-items:center; margin-bottom:0.75rem; }
  .kanban-board { display:grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap:0.75rem; }
  .kanban-column { min-width:0; display:flex; flex-direction:column; }
  .column-header { display:flex; justify-content:space-between; align-items:center; padding:0.75rem; border-bottom:1px solid var(--border); }
  .column-body { display:grid; gap:0.5rem; padding:0.75rem; }
  .project-meta { display:flex; justify-content:space-between; gap:0.5rem; margin-top:0.5rem; font-size:0.75rem; color:var(--text-muted); }
  @media (max-width: 1100px) {
    .summary-grid { grid-template-columns:1fr; }
    .kanban-board { grid-template-columns: 1fr 1fr; }
  }
  @media (max-width: 700px) {
    .kanban-board { grid-template-columns:1fr; }
  }
</style>
