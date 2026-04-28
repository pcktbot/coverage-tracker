<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { repos } from '$lib/stores/repos';
  import DocsViewerPane from '$lib/components/DocsViewerPane.svelte';
  import {
    checkoutRepoBranch,
    listRepoBranches,
    listRepoDocs,
    readRepoDoc,
    type RepoBranch,
    type RepoDocContent,
    type RepoDocSummary,
  } from '$lib/api';

  const STORAGE_KEY = 'coverage-manager-docs-state';

  let repoId = $state<number | null>(null);
  let query = $state('');
  let runbooksOnly = $state(true);
  let docs = $state<RepoDocSummary[]>([]);
  let docPath = $state<string | null>(null);
  let doc = $state<RepoDocContent | null>(null);
  let loadingList = $state(false);
  let loadingDoc = $state(false);
  let error = $state('');
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let currentBranch = $state('');
  let branches = $state<RepoBranch[]>([]);
  let branchesLoading = $state(false);
  let branchSwitching = $state(false);
  let branchError = $state('');
  let initialized = $state(false);

  const availableRepos = $derived(
    $repos.filter((repo) => repo.local_path).sort((a, b) => a.org.localeCompare(b.org) || a.name.localeCompare(b.name))
  );
  onMount(() => {
    const params = $page.url.searchParams;
    const repoFromUrl = Number(params.get('repo') ?? params.get('leftRepo'));
    const restored = restoreState(repoFromUrl);

    if (!restored && availableRepos.length > 0) {
      repoId = availableRepos[0].id;
    }
    initialized = true;
  });

  $effect(() => {
    if (availableRepos.length === 0) return;
    if (repoId) return;
    repoId = availableRepos[0].id;
  });

  $effect(() => {
    if (repoId) {
      loadDocList();
      loadBranches();
    }
  });

  $effect(() => {
    if (!initialized || !repoId) return;
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      repoId,
      query,
      runbooksOnly,
      docPath,
    }));
  });

  function restoreState(repoFromUrl: number): boolean {
    const validIds = new Set(availableRepos.map((repo) => repo.id));
    if (validIds.has(repoFromUrl)) {
      repoId = repoFromUrl;
      return true;
    }

    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return false;

    try {
      const parsed = JSON.parse(raw) as {
        repoId?: number;
        query?: string;
        runbooksOnly?: boolean;
        docPath?: string | null;
      };
      if (parsed.repoId && validIds.has(parsed.repoId)) {
        repoId = parsed.repoId;
        query = parsed.query ?? '';
        runbooksOnly = parsed.runbooksOnly ?? true;
        docPath = parsed.docPath ?? null;
        return true;
      }
    } catch {
      localStorage.removeItem(STORAGE_KEY);
    }

    return false;
  }

  async function loadDocList() {
    if (!repoId) return;

    loadingList = true;
    error = '';

    try {
      const nextDocs = await listRepoDocs(repoId, {
        query,
        runbooksOnly,
      });
      docs = nextDocs;
      if (!nextDocs.some((item) => item.path === docPath)) {
        docPath = nextDocs[0]?.path ?? null;
        doc = null;
      }
      if (docPath) await loadDoc(docPath);
    } catch (e: any) {
      docs = [];
      doc = null;
      error = e.message ?? String(e);
    } finally {
      loadingList = false;
    }
  }

  async function loadDoc(path: string) {
    if (!repoId) return;
    loadingDoc = true;
    error = '';
    docPath = path;

    try {
      doc = await readRepoDoc(repoId, path);
    } catch (e: any) {
      doc = null;
      error = e.message ?? String(e);
    } finally {
      loadingDoc = false;
    }
  }

  async function loadBranches() {
    if (!repoId) return;
    branchesLoading = true;
    branchError = '';
    try {
      const state = await listRepoBranches(repoId);
      currentBranch = state.current_branch;
      branches = state.branches;
    } catch (e: any) {
      branches = [];
      currentBranch = '';
      branchError = e.message ?? String(e);
    } finally {
      branchesLoading = false;
    }
  }

  async function switchBranch(branchName: string) {
    if (!repoId || !branchName || branchName === currentBranch) return;
    branchSwitching = true;
    branchError = '';
    try {
      const state = await checkoutRepoBranch(repoId, branchName);
      currentBranch = state.current_branch;
      branches = state.branches;
      await loadDocList();
    } catch (e: any) {
      branchError = e.message ?? String(e);
    } finally {
      branchSwitching = false;
    }
  }

  function scheduleSearch() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      void loadDocList();
    }, 180);
  }

</script>

<div class="docs-header">
  <div>
    <h1>Docs workspace</h1>
    <p class="docs-subtitle">Search repo markdown, remember where you left off, and switch branches without leaving the reader.</p>
  </div>
</div>

{#if availableRepos.length === 0}
  <div class="empty card">
    <h2>No local repos</h2>
    <p class="text-secondary">Clone at least one repo first. Docs are read directly from each repo checkout.</p>
  </div>
{:else}
  <div class="single-layout">
    <DocsViewerPane
      repos={availableRepos}
      repoId={repoId}
      query={query}
      runbooksOnly={runbooksOnly}
      docs={docs}
      selectedDocPath={docPath}
      doc={doc}
      currentBranch={currentBranch}
      branches={branches}
      branchesLoading={branchesLoading}
      branchSwitching={branchSwitching}
      branchError={branchError}
      loadingList={loadingList}
      loadingDoc={loadingDoc}
      error={error}
      paneLabel="docs"
      onRepoChange={(nextRepoId) => { repoId = nextRepoId; docPath = null; doc = null; }}
      onQueryChange={(nextQuery) => { query = nextQuery; scheduleSearch(); }}
      onToggleRunbooks={() => { runbooksOnly = !runbooksOnly; void loadDocList(); }}
      onSelectDoc={(path) => loadDoc(path)}
      onBranchChange={(branchName) => void switchBranch(branchName)}
    />
  </div>
{/if}

<style>
  .docs-header {
    margin-bottom: 1rem;
  }

  .docs-subtitle {
    margin: 0.35rem 0 0;
    color: var(--text-secondary);
  }

  .empty {
    padding: 1.25rem;
  }

  .single-layout {
    height: calc(100vh - 170px);
    min-height: 0;
  }
</style>
