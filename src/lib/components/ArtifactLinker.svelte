<script lang="ts">
  import { listProjects, listCachedConfluencePages } from '$lib/api';
  import type { ProjectSummary, CachedConfluencePage } from '$lib/api';
  import { invoke } from '@tauri-apps/api/core';
  import { linkArtifact, unlinkArtifact, type ArtifactKind, type Session } from '$lib/orchestrator';

  let { session }: { session: Session } = $props();

  // Local mutable copy so the drawer reacts after link/unlink without prop refresh.
  let local = $state<Session>({ ...session });

  let kind = $state<ArtifactKind>(session.artifact_kind ?? 'project');
  let idText = $state<string>(session.artifact_id ?? '');
  let titleText = $state<string>(session.artifact_title ?? '');
  let urlText = $state<string>(session.artifact_url ?? '');
  let saving = $state(false);
  let error = $state<string | null>(null);

  let projectOptions = $state<ProjectSummary[]>([]);
  let confluenceOptions = $state<CachedConfluencePage[]>([]);

  async function refreshProjects() {
    try { projectOptions = await listProjects(); } catch { projectOptions = []; }
  }

  async function refreshConfluenceCache() {
    if (!idText.trim()) { confluenceOptions = []; return; }
    try {
      const pages = await listCachedConfluencePages([idText.trim()]);
      confluenceOptions = pages;
      if (pages[0]) titleText = pages[0].title;
    } catch { confluenceOptions = []; }
  }

  async function probeAdoTitle() {
    if (!idText.trim()) return;
    try {
      const result: any = await Promise.race([
        invoke('ado_query_project_work_items', { projectId: idText.trim() }),
        new Promise((_, rej) => setTimeout(() => rej(new Error('timeout')), 1500))
      ]);
      if (Array.isArray(result) && result.length && result[0]?.title) {
        titleText = result[0].title;
      }
    } catch {
      // silent — autocomplete is best-effort
    }
  }

  $effect(() => {
    if (kind === 'project' && projectOptions.length === 0) {
      void refreshProjects();
    }
  });

  async function save() {
    if (kind === 'github_pr' && urlText.trim()) {
      idText = urlText.trim();
    }
    if (!idText.trim()) { error = 'ID required'; return; }
    saving = true;
    error = null;
    try {
      await linkArtifact(local.id, {
        kind,
        id: idText.trim(),
        title: titleText.trim() || null,
        url: urlText.trim() || null,
      });
      local = {
        ...local,
        artifact_kind: kind,
        artifact_id: idText.trim(),
        artifact_title: titleText.trim() || null,
        artifact_url: urlText.trim() || null,
      };
    } catch (e: any) {
      error = String(e?.message ?? e);
    } finally {
      saving = false;
    }
  }

  async function unlink() {
    saving = true;
    error = null;
    try {
      await unlinkArtifact(local.id);
      idText = ''; titleText = ''; urlText = '';
      local = {
        ...local,
        artifact_kind: null,
        artifact_id: null,
        artifact_title: null,
        artifact_url: null,
      };
    } catch (e: any) {
      error = String(e?.message ?? e);
    } finally {
      saving = false;
    }
  }

  const linked = $derived(Boolean(local.artifact_kind));
</script>

<section class="linker">
  {#if linked}
    <div class="current">
      Linked to <strong>{local.artifact_title ?? local.artifact_id}</strong>
      <span class="kind">[{local.artifact_kind}]</span>
      <button class="unlink" onclick={unlink} disabled={saving} type="button">Unlink</button>
    </div>
  {:else}
    <div class="form">
      <label>
        Kind
        <select bind:value={kind}>
          <option value="project">Project</option>
          <option value="ado">ADO Work Item</option>
          <option value="confluence">Confluence Page</option>
          <option value="github_pr">GitHub PR</option>
        </select>
      </label>

      {#if kind === 'project'}
        <label>
          Project
          <select bind:value={idText} onchange={() => {
            const p = projectOptions.find((x) => String(x.id) === idText);
            if (p) titleText = p.name;
          }}>
            <option value="">— pick a project —</option>
            {#each projectOptions as p}
              <option value={String(p.id)}>{p.name}</option>
            {/each}
          </select>
        </label>
      {:else if kind === 'ado'}
        <label>
          Work Item ID
          <input bind:value={idText} onblur={() => void probeAdoTitle()} placeholder="e.g. 123456" />
        </label>
        <label>
          Title (optional)
          <input bind:value={titleText} placeholder="auto-filled from ADO on blur" />
        </label>
      {:else if kind === 'confluence'}
        <label>
          Page ID
          <input bind:value={idText} onblur={() => void refreshConfluenceCache()} placeholder="e.g. 458760" />
        </label>
        <label>
          Title (optional)
          <input bind:value={titleText} placeholder="auto-filled from cache on blur" />
        </label>
      {:else if kind === 'github_pr'}
        <label>
          PR URL
          <input bind:value={urlText} placeholder="https://github.com/org/repo/pull/123" />
        </label>
        <label>
          Title (optional)
          <input bind:value={titleText} placeholder="Manual title" />
        </label>
        <p class="hint">Stub: paste the full URL. The ID field is derived from the URL on save.</p>
      {/if}

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <button onclick={() => void save()} disabled={saving} type="button">Link</button>
    </div>
  {/if}
</section>

<style>
  .linker { margin-bottom: 1rem; padding: 0.75rem; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--bg-muted); }
  .current { display: flex; align-items: center; gap: 0.5rem; }
  .kind { color: var(--text-muted); font-size: 0.75rem; text-transform: uppercase; }
  .unlink { margin-left: auto; padding: 0.25rem 0.5rem; }
  .form { display: flex; flex-direction: column; gap: 0.5rem; }
  label { display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.875rem; }
  select, input { padding: 0.4rem; border: 1px solid var(--border); border-radius: var(--radius-sm); font: inherit; }
  .hint { margin: 0; font-size: 0.75rem; color: var(--text-muted); }
  .error { margin: 0; color: var(--accent-danger, #c0392b); font-size: 0.875rem; }
  button { padding: 0.4rem 0.8rem; cursor: pointer; }
</style>
