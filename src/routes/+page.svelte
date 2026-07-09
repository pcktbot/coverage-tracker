<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { activeOrg } from '$lib/stores/repos';
  import {
    createProject,
    listProjects,
    updateProjectStatus,
    type ProjectSummary,
  } from '$lib/api';

  const COMMAND_CENTER_STATE_KEY = 'coverage-manager-command-center-state';

  let error = $state('');
  let loading = $state(true);
  let creating = $state(false);
  let savingProjectId = $state<number | null>(null);
  let dragProjectId = $state<number | null>(null);
  let dragOverStatus = $state<string | null>(null);
  let projects = $state<ProjectSummary[]>([]);
  let selectedProjectId = $state<number | null>(null);

  const FLOW_COLUMNS = ['incoming', 'needs_attention', 'waiting', 'in_motion', 'done_this_week'];

  onMount(async () => {
    try {
      const raw = localStorage.getItem(COMMAND_CENTER_STATE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as { selectedProjectId?: number };
        if (Number.isFinite(parsed.selectedProjectId)) {
          selectedProjectId = parsed.selectedProjectId ?? null;
        }
      }
    } catch {
      // Local state restore is best-effort only.
    }
    await load();
  });

  $effect(() => {
    try {
      localStorage.setItem(
        COMMAND_CENTER_STATE_KEY,
        JSON.stringify({ selectedProjectId }),
      );
    } catch {
      // Local state persistence is best-effort only.
    }
  });

  async function load() {
    loading = true;
    error = '';
    try {
      const projectList = await listProjects();
      projects = projectList;
      selectedProjectId = selectedProjectId && projectList.some((project) => project.id === selectedProjectId)
        ? selectedProjectId
        : projectList[0]?.id ?? null;
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

  const selectedProject = $derived(
    projects.find((project) => project.id === selectedProjectId)
      ?? [...projects].sort((a, b) => b.focus_score - a.focus_score)[0]
      ?? null,
  );

  function handleDragStart(projectId: number) {
    dragProjectId = projectId;
    error = '';
  }

  function handleDragEnd() {
    dragProjectId = null;
    dragOverStatus = null;
  }

  function handleDragOver(event: DragEvent, status: string) {
    event.preventDefault();
    dragOverStatus = status;
  }

  function handleDragLeave(status: string) {
    if (dragOverStatus === status) {
      dragOverStatus = null;
    }
  }

  async function handleDrop(event: DragEvent, status: string) {
    event.preventDefault();
    dragOverStatus = null;
    if (dragProjectId == null) return;
    const project = projects.find((entry) => entry.id === dragProjectId);
    if (!project || project.status === status) {
      dragProjectId = null;
      return;
    }

    const previousStatus = project.status;
    savingProjectId = project.id;
    projects = projects.map((entry) =>
      entry.id === project.id ? { ...entry, status } : entry,
    );

    try {
      await updateProjectStatus(project.id, status);
    } catch (e: any) {
      projects = projects.map((entry) =>
        entry.id === project.id ? { ...entry, status: previousStatus } : entry,
      );
      error = e.message ?? String(e);
    } finally {
      savingProjectId = null;
      dragProjectId = null;
    }
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
          <button class="priority-item" class:selected={selectedProject?.id === project.id} onclick={() => (selectedProjectId = project.id)}>
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
      <h2>Selected Project</h2>
      {#if selectedProject}
        <div class="selected-project">
          <div>
            <strong>{selectedProject.name}</strong>
            <div class="text-muted">{selectedProject.platform_name ?? 'No platform'} · {labelFor(selectedProject.status)}</div>
          </div>
          <div class="selected-project-grid">
            <div class="selected-project-stat">
              <span class="selected-project-label">Focus</span>
              <strong>{selectedProject.focus_score}</strong>
            </div>
            <div class="selected-project-stat">
              <span class="selected-project-label">Repos</span>
              <strong>{selectedProject.repo_count}</strong>
            </div>
            <div class="selected-project-stat">
              <span class="selected-project-label">Sources</span>
              <strong>{selectedProject.source_link_count}</strong>
            </div>
            <div class="selected-project-stat">
              <span class="selected-project-label">Priority</span>
              <strong>{selectedProject.manual_priority}</strong>
            </div>
          </div>
          <button class="btn-secondary" onclick={() => goto(`/projects/${selectedProject.id}`)}>Open project</button>
        </div>
      {/if}
    </section>
  </div>

  <section class="kanban-section">
    <div class="kanban-header">
      <h2>Leadership Flow</h2>
      <span class="text-muted">{projects.length} tracked projects</span>
    </div>
    <div class="kanban-board">
      {#each FLOW_COLUMNS as status}
        <div
          class="kanban-column card"
          class:drag-over={dragOverStatus === status}
          role="list"
          aria-label={`${labelFor(status)} projects`}
          ondragover={(event) => handleDragOver(event, status)}
          ondragleave={() => handleDragLeave(status)}
          ondrop={(event) => handleDrop(event, status)}
        >
          <div class="column-header">
            <h3>{labelFor(status)}</h3>
            <span class="badge badge-gray">{projectsFor(status).length}</span>
          </div>
          <div class="column-body">
            {#each projectsFor(status) as project}
              <button
                class="project-card"
                class:dragging={dragProjectId === project.id}
                draggable="true"
                onclick={() => goto(`/projects/${project.id}`)}
                ondragstart={() => handleDragStart(project.id)}
                ondragend={handleDragEnd}
              >
                <strong>{project.name}</strong>
                <div class="text-muted">{project.platform_name ?? 'No platform'}</div>
                <div class="project-meta">
                  <span>{project.repo_count} repos</span>
                  <span>{project.source_link_count} source links</span>
                </div>
                {#if savingProjectId === project.id}
                  <div class="project-saving">Updating…</div>
                {/if}
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
  .priority-list { display:grid; gap:0.5rem; margin-top:0.75rem; }
  .priority-item, .project-card {
    width:100%; text-align:left; padding:0.75rem; border:1px solid var(--border);
    border-radius:var(--radius-sm); background:var(--bg); color:inherit;
  }
  .priority-item:hover, .project-card:hover { background:var(--bg-subtle); }
  .priority-item.selected { border-color: var(--accent); background: var(--accent-subtle); }
  .selected-project { display:grid; gap:0.9rem; margin-top:0.75rem; }
  .selected-project-grid { display:grid; grid-template-columns:1fr 1fr; gap:0.75rem; }
  .selected-project-stat { padding:0.75rem; border:1px solid var(--border); border-radius:var(--radius-sm); background:var(--bg); display:grid; gap:0.2rem; }
  .selected-project-label { font-size:0.72rem; color:var(--text-secondary); text-transform:uppercase; letter-spacing:0.05em; }
  .kanban-section { min-width:0; }
  .kanban-header { display:flex; justify-content:space-between; align-items:center; margin-bottom:0.75rem; }
  .kanban-board { display:grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap:0.75rem; }
  .kanban-column { min-width:0; display:flex; flex-direction:column; }
  .kanban-column.drag-over { border-color: var(--accent); background: var(--accent-subtle); }
  .column-header { display:flex; justify-content:space-between; align-items:center; padding:0.75rem; border-bottom:1px solid var(--border); }
  .column-body { display:grid; gap:0.5rem; padding:0.75rem; min-height:7rem; }
  .project-meta { display:flex; justify-content:space-between; gap:0.5rem; margin-top:0.5rem; font-size:0.75rem; color:var(--text-muted); }
  .project-card { cursor: grab; }
  .project-card.dragging { opacity: 0.55; }
  .project-saving { margin-top:0.5rem; font-size:0.75rem; color:var(--accent); }
  @media (max-width: 1100px) {
    .summary-grid { grid-template-columns:1fr; }
    .kanban-board { grid-template-columns: 1fr 1fr; }
  }
  @media (max-width: 700px) {
    .kanban-board { grid-template-columns:1fr; }
  }
</style>
