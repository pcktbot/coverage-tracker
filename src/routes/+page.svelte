<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { repos, activeOrg, enabledRepos, refreshRepos } from '$lib/stores/repos';
  import { latestRuns, trends, runningRepos, markRunning, markDone } from '$lib/stores/coverage';
  import CoverageBadge from '$lib/components/CoverageBadge.svelte';
  import TrendSparkline from '$lib/components/TrendSparkline.svelte';
  import {
    syncOrgRepos,
    cloneOrPullRepo,
    runCoverage,
    listRuns,
    getTrend,
    exportCsv,
    downloadCsv,
  } from '$lib/api';

  let error = $state('');
  let syncing = $state(false);
  let cloningAll = $state(false);
  let runningAll = $state(false);
  let exporting = $state(false);

  onMount(async () => {
    await loadRunData();
  });

  async function loadRunData() {
    for (const repo of $repos) {
      try {
        const runs = await listRuns(repo.id);
        const latest = runs.find((r) => r.status === 'success' || r.status === 'failed') ?? runs[0];
        if (latest) {
          latestRuns.update((m) => { m.set(repo.id, latest); return new Map(m); });
        }
        const trend = await getTrend(repo.id, 10);
        trends.update((m) => { m.set(repo.id, trend); return new Map(m); });
      } catch { /* non-fatal */ }
    }
  }

  async function syncRepos() {
    if (!$activeOrg) return;
    syncing = true;
    error = '';
    try {
      await syncOrgRepos($activeOrg);
      await refreshRepos($activeOrg);
      await loadRunData();
    } catch (e: any) {
      error = e.message;
    } finally {
      syncing = false;
    }
  }

  async function cloneAll() {
    cloningAll = true;
    error = '';
    for (const repo of $enabledRepos) {
      try {
        await cloneOrPullRepo(repo.id);
      } catch (e: any) {
        error = (error ? error + '\n' : '') + `${repo.name}: ${e.message}`;
      }
    }
    await refreshRepos($activeOrg ?? undefined);
    cloningAll = false;
  }

  async function runRepo(repoId: number) {
    markRunning(repoId);
    error = '';
    try {
      await runCoverage(repoId);
      const runs = await listRuns(repoId);
      const latest = runs[0];
      if (latest) markDone(repoId, latest);
      const trend = await getTrend(repoId, 10);
      trends.update((m) => { m.set(repoId, trend); return new Map(m); });
    } catch (e: any) {
      error = (error ? error + '\n' : '') + `Run failed: ${e.message}`;
      runningRepos.update((s) => { s.delete(repoId); return new Set(s); });
    }
  }

  async function runAll() {
    runningAll = true;
    for (const repo of $enabledRepos) {
      await runRepo(repo.id);
    }
    runningAll = false;
  }

  async function doExport(repoId?: number) {
    exporting = true;
    error = '';
    try {
      const csv = await exportCsv(repoId, false);
      const name = repoId
        ? `coverage-${$repos.find((r) => r.id === repoId)?.name ?? repoId}.csv`
        : `coverage-${$activeOrg ?? 'all'}.csv`;
      downloadCsv(csv, name);
    } catch (e: any) {
      error = e.message;
    } finally {
      exporting = false;
    }
  }
</script>

<div class="page-header">
  <h1>{$activeOrg ?? 'Coverage'}</h1>
  <div class="header-actions">
    <button class="btn-secondary" onclick={syncRepos} disabled={syncing || !$activeOrg}>
      {syncing ? 'Syncing…' : 'Sync repos'}
    </button>
    <button class="btn-secondary" onclick={cloneAll} disabled={cloningAll}>
      {cloningAll ? 'Cloning…' : 'Clone / pull all'}
    </button>
    <button class="btn-primary" onclick={runAll} disabled={runningAll}>
      {runningAll ? 'Running…' : 'Run all'}
    </button>
    <button class="btn-secondary" onclick={() => doExport()} disabled={exporting}>
      {exporting ? 'Exporting…' : 'Export CSV'}
    </button>
  </div>
</div>

{#if error}
  <div class="error-msg" style="margin-bottom:1rem">{error}</div>
{/if}

{#if $repos.length === 0}
  <div class="empty">
    <p class="text-secondary">No repos found for <strong>{$activeOrg}</strong>.</p>
    <p class="text-muted">Click <em>Sync repos</em> to fetch from GitHub, or configure a token in Settings.</p>
  </div>
{:else}
  <div class="repo-grid">
    {#each $repos as repo}
      {@const run = $latestRuns.get(repo.id)}
      {@const trend = $trends.get(repo.id) ?? []}
      {@const running = $runningRepos.has(repo.id)}
      <div class="repo-card card" class:disabled={!repo.enabled}>
        <div class="card-top">
          <div class="repo-meta">
            <button class="repo-name" onclick={() => goto(`/repo/${repo.id}`)}>{repo.name}</button>
            {#if repo.ruby_version}
              <span class="ruby-tag text-muted mono">ruby {repo.ruby_version}</span>
            {/if}
          </div>
          <CoverageBadge pct={run?.overall_coverage} />
        </div>

        <div class="card-mid">
          <TrendSparkline points={trend} width={90} height={30} />
          {#if run?.status === 'failed'}
            <span class="badge badge-red" style="font-size:0.6875rem">failed</span>
          {:else if running}
            <span class="badge badge-yellow" style="font-size:0.6875rem">running…</span>
          {/if}
        </div>

        <div class="card-actions">
          <button class="btn-ghost" onclick={() => cloneOrPullRepo(repo.id)} disabled={running}>Pull</button>
          <button class="btn-primary" onclick={() => runRepo(repo.id)} disabled={running || !repo.local_path}>
            {running ? 'Running…' : 'Run'}
          </button>
          <button class="btn-ghost" onclick={() => doExport(repo.id)} disabled={exporting}>CSV</button>
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1.25rem;
  }
  .header-actions { display: flex; gap: 0.5rem; }
  .empty { padding: 3rem 1rem; text-align: center; }
  .repo-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 0.875rem;
  }
  .repo-card { padding: 0.875rem; display: flex; flex-direction: column; gap: 0.5rem; }
  .repo-card.disabled { opacity: 0.5; }
  .card-top { display: flex; justify-content: space-between; align-items: flex-start; }
  .repo-meta { display: flex; flex-direction: column; gap: 0.125rem; min-width: 0; }
  .repo-name {
    background: none; border: none; padding: 0;
    font-size: 0.875rem; font-weight: 600; color: var(--accent);
    cursor: pointer; text-align: left;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .repo-name:hover { text-decoration: underline; }
  .ruby-tag { font-size: 0.6875rem; }
  .card-mid { display: flex; align-items: center; gap: 0.5rem; min-height: 32px; }
  .card-actions { display: flex; gap: 0.375rem; justify-content: flex-end; margin-top: 0.25rem; }
</style>
