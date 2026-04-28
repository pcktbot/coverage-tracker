<script lang="ts">
  import MarkdownDoc from '$lib/components/MarkdownDoc.svelte';
  import type { Repo, RepoBranch, RepoDocContent, RepoDocSummary } from '$lib/api';

  interface Props {
    repos: Repo[];
    repoId: number | null;
    query: string;
    runbooksOnly: boolean;
    docs: RepoDocSummary[];
    selectedDocPath: string | null;
    doc: RepoDocContent | null;
    currentBranch: string;
    branches: RepoBranch[];
    branchesLoading: boolean;
    branchSwitching: boolean;
    branchError: string;
    loadingList: boolean;
    loadingDoc: boolean;
    error: string;
    paneLabel: string;
    onRepoChange: (repoId: number | null) => void;
    onQueryChange: (query: string) => void;
    onToggleRunbooks: () => void;
    onSelectDoc: (path: string) => void;
    onBranchChange: (branchName: string) => void;
  }

  let {
    repos,
    repoId,
    query,
    runbooksOnly,
    docs,
    selectedDocPath,
    doc,
    currentBranch,
    branches,
    branchesLoading,
    branchSwitching,
    branchError,
    loadingList,
    loadingDoc,
    error,
    paneLabel,
    onRepoChange,
    onQueryChange,
    onToggleRunbooks,
    onSelectDoc,
    onBranchChange,
  }: Props = $props();

  let asideOpen = $state(false);
  const currentRepo = $derived(repos.find((repo) => repo.id === repoId));

  function formatRepoLabel(repo: Repo): string {
    return `${repo.org}/${repo.name}`;
  }

  function formatDate(iso?: string): string {
    if (!iso) return 'Unknown';
    return new Date(iso).toLocaleString();
  }

  function handleRepoChange(event: Event) {
    const value = Number((event.currentTarget as HTMLSelectElement).value);
    onRepoChange(Number.isFinite(value) ? value : null);
  }

  function handleQueryInput(event: Event) {
    onQueryChange((event.currentTarget as HTMLInputElement).value);
  }
</script>

<section class="pane card">
  <div class="pane-toolbar">
    <div class="toolbar-main">
      <select value={repoId ?? ''} onchange={handleRepoChange} aria-label={`${paneLabel} repo`}>
        {#each repos as repo}
          <option value={repo.id}>{formatRepoLabel(repo)}</option>
        {/each}
      </select>
      <input
        value={query}
        oninput={handleQueryInput}
        placeholder="Search runbooks, incidents, deploy steps…"
        aria-label={`Search ${paneLabel} docs`}
      />
      <button class:runbooks-only={runbooksOnly} class="filter-chip" onclick={onToggleRunbooks}>
        {runbooksOnly ? 'Runbooks only' : 'All markdown'}
      </button>
      <select
        value={currentBranch}
        onchange={(event) => onBranchChange((event.currentTarget as HTMLSelectElement).value)}
        aria-label="Repo branch"
        disabled={branchesLoading || branchSwitching || branches.length === 0}
      >
        {#if branchesLoading}
          <option value="">Loading branches…</option>
        {:else if branches.length === 0}
          <option value="">No branches</option>
        {:else}
          {#each branches as branch}
            <option value={branch.name}>{branch.name}{branch.is_remote ? ' (remote)' : ''}</option>
          {/each}
        {/if}
      </select>
    </div>
    <button
      class="aside-toggle"
      class:open={asideOpen}
      onclick={() => (asideOpen = !asideOpen)}
      aria-expanded={asideOpen}
      aria-controls="docs-list"
    >
      <span class="toggle-icon">&#9654;</span>
      {asideOpen ? 'Hide docs' : `Browse docs (${docs.length})`}
    </button>
  </div>

  <div class="pane-body" class:has-aside={asideOpen}>
    <aside id="docs-list" class="doc-list" class:open={asideOpen}>
      <div class="doc-list-inner">
        {#if error}
          <div class="error-msg">{error}</div>
        {/if}
        {#if branchError}
          <div class="error-msg" style="margin-bottom:0.5rem">{branchError}</div>
        {/if}
        {#if loadingList}
          <p class="text-muted">Scanning docs…</p>
        {:else if docs.length === 0}
          <p class="text-muted">No matching docs.</p>
        {:else}
          {#each docs as item}
            <button class:selected={item.path === selectedDocPath} class="doc-list-item" onclick={() => onSelectDoc(item.path)}>
              <div class="doc-item-top">
                <strong>{item.title}</strong>
                {#if item.is_runbook}<span class="badge badge-yellow">runbook</span>{/if}
              </div>
              <div class="doc-item-path mono">{item.path}</div>
              <div class="doc-item-preview">{item.preview}</div>
            </button>
          {/each}
        {/if}
      </div>
    </aside>

    <article class="doc-viewer">
      {#if loadingDoc}
        <p class="text-muted">Loading document…</p>
      {:else if doc}
        <div class="doc-meta">
          <div>
            <h2>{doc.title}</h2>
            <div class="meta-line mono">{currentRepo ? formatRepoLabel(currentRepo) : ''} · {doc.path}</div>
            <div class="meta-branch">{branchSwitching ? 'Switching branch…' : currentBranch ? `branch ${currentBranch}` : ''}</div>
          </div>
          <div class="meta-actions">
            <span class="text-muted">{formatDate(doc.modified_at)}</span>
            <button class="aside-toggle inline-toggle" class:open={asideOpen} onclick={() => (asideOpen = !asideOpen)}>
              <span class="toggle-icon">&#9654;</span>
              {asideOpen ? 'Hide list' : 'Show list'}
            </button>
          </div>
        </div>
        <MarkdownDoc markdown={doc.markdown} />
      {:else}
        <div class="empty-state">
          <p class="text-muted">Pick a document to read.</p>
          <button class="aside-toggle inline-toggle" class:open={asideOpen} onclick={() => (asideOpen = !asideOpen)}>
            <span class="toggle-icon">&#9654;</span>
            {asideOpen ? 'Hide list' : 'Browse docs'}
          </button>
        </div>
      {/if}
    </article>
  </div>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .pane-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.85rem;
    border-bottom: 1px solid var(--border);
    background: linear-gradient(180deg, #fcfcfd 0%, var(--bg-subtle) 100%);
  }

  .toolbar-main {
    display: grid;
    grid-template-columns: minmax(180px, 0.8fr) minmax(220px, 1.2fr) auto minmax(180px, 0.8fr);
    gap: 0.75rem;
    flex: 1;
  }

  .filter-chip {
    white-space: nowrap;
    background: var(--bg);
    border-color: var(--border);
    color: var(--text-secondary);
  }

  .filter-chip.runbooks-only {
    background: var(--warning-subtle);
    border-color: #f7d9a4;
    color: #9a5b00;
  }

  .aside-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    background: transparent;
    color: var(--text-secondary);
    border-color: transparent;
    padding: 0.35rem 0.5rem;
    flex-shrink: 0;
  }

  .aside-toggle:hover {
    background: var(--bg);
    border-color: var(--border);
  }

  .toggle-icon {
    display: inline-block;
    font-size: 0.65rem;
    transition: transform 0.15s ease;
  }

  .aside-toggle.open .toggle-icon {
    transform: rotate(90deg);
  }

  .pane-body {
    display: grid;
    grid-template-columns: 0 minmax(0, 1fr);
    min-height: 0;
    flex: 1;
    overflow: hidden;
    transition: grid-template-columns 0.18s ease;
  }

  .pane-body.has-aside {
    grid-template-columns: 320px minmax(0, 1fr);
  }

  .doc-list {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: #fff;
    border-right: 1px solid transparent;
    transition: border-color 0.18s ease;
  }

  .pane-body.has-aside .doc-list {
    border-right-color: var(--border);
  }

  .doc-list-inner {
    width: 100%;
    height: 100%;
    overflow-y: auto;
    padding: 0.5rem;
  }

  .doc-list-item {
    width: 100%;
    text-align: left;
    border: 1px solid transparent;
    background: transparent;
    border-radius: var(--radius-sm);
    padding: 0.7rem 0.75rem;
    margin-bottom: 0.35rem;
  }

  .doc-list-item:hover {
    background: var(--bg-subtle);
    border-color: var(--border-subtle);
  }

  .doc-list-item.selected {
    background: var(--accent-subtle);
    border-color: #bfdbfe;
  }

  .doc-item-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-bottom: 0.2rem;
  }

  .doc-item-path {
    font-size: 0.68rem;
    color: var(--text-muted);
    margin-bottom: 0.35rem;
    word-break: break-all;
  }

  .doc-item-preview {
    font-size: 0.78rem;
    color: var(--text-secondary);
    line-height: 1.45;
  }

  .doc-viewer {
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding: 1.1rem 1.25rem 1.5rem;
    background:
      radial-gradient(circle at top right, rgba(37, 99, 235, 0.08), transparent 28%),
      linear-gradient(180deg, #fff 0%, #fdfefe 100%);
  }

  .doc-meta {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding-bottom: 1rem;
    margin-bottom: 1rem;
    border-bottom: 1px solid var(--border);
  }

  .meta-line {
    margin-top: 0.25rem;
    color: var(--text-muted);
    font-size: 0.72rem;
    word-break: break-all;
  }

  .meta-branch {
    margin-top: 0.25rem;
    color: var(--text-secondary);
    font-size: 0.78rem;
  }

  .meta-actions {
    display: flex;
    align-items: flex-end;
    flex-direction: column;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .inline-toggle {
    padding-right: 0;
  }

  .empty-state {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  @media (max-width: 900px) {
    .pane-toolbar {
      flex-direction: column;
      align-items: stretch;
    }

    .toolbar-main {
      grid-template-columns: 1fr;
    }

    .pane-body {
      grid-template-columns: 1fr;
    }

    .pane-body.has-aside {
      grid-template-columns: 1fr;
      grid-template-rows: 240px minmax(0, 1fr);
    }

    .doc-list {
      max-height: 0;
      border-right: none;
      border-bottom: 1px solid transparent;
      transition: max-height 0.18s ease, border-color 0.18s ease;
    }

    .pane-body.has-aside .doc-list {
      max-height: 240px;
      border-bottom-color: var(--border);
    }

    .doc-list-inner {
      width: 100%;
    }

    .doc-meta,
    .empty-state {
      flex-direction: column;
    }

    .meta-actions {
      align-items: flex-start;
    }
  }
</style>
