<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { repos, activeOrg, enabledRepos, refreshRepos } from '$lib/stores/repos';
  import { latestRuns, trends, runningRepos, markRunning, markDone } from '$lib/stores/coverage';
  import CoverageBadge from '$lib/components/CoverageBadge.svelte';
  import TrendSparkline from '$lib/components/TrendSparkline.svelte';
  import {
    cloneOrPullRepo,
    runCoverage,
    listRuns,
    getTrend,
    exportCsv,
    downloadCsv,
  } from '$lib/api';

  let error = $state('');
  let cloningAll = $state(false);
  let runningAll = $state(false);
  let exporting = $state(false);

  onMount(async () => {
    await loadRunData();
  });

  async function loadRunData() {
    // Parallelize all IPC calls to avoid sequential lock contention
    await Promise.all($repos.map(async (repo) => {
      try {
        const [runs, trend] = await Promise.all([
          listRuns(repo.id),
          getTrend(repo.id, 10),
        ]);
        const latest = runs.find((r) => r.status === 'success' || r.status === 'failed') ?? runs[0];
        if (latest) {
          latestRuns.update((m) => { m.set(repo.id, latest); return new Map(m); });
        }
        trends.update((m) => { m.set(repo.id, trend); return new Map(m); });
      } catch { /* non-fatal */ }
    }));
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
    error = '';
    const promises = $enabledRepos.map((repo) => runRepo(repo.id));
    await Promise.allSettled(promises);
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
    <button class="btn-secondary" onclick={cloneAll} disabled={cloningAll || $enabledRepos.length === 0}>
      {cloningAll ? 'Cloning…' : 'Clone / pull all'}
    </button>
    <button class="btn-primary" onclick={runAll} disabled={runningAll || $enabledRepos.length === 0}>
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
    <p class="text-secondary">No repos synced for <strong>{$activeOrg}</strong>.</p>
    <p class="text-muted">Go to <a href="/settings">Settings</a> → <em>Sync from GitHub</em> to fetch the repo list, then enable the ones you want to track.</p>
  </div>
{:else if $enabledRepos.length === 0}
  <div class="empty">
    <p class="text-secondary">All repos are disabled.</p>
    <p class="text-muted">Go to <a href="/settings">Settings</a> to enable repos for cloning and running.</p>
  </div>
{:else}
  <table class="repo-table">
    <thead>
      <tr>
        <th>Repo</th>
        <th>Ruby</th>
        <th class="col-cov">Coverage</th>
        <th class="col-trend">Trend</th>
        <th class="col-status">Status</th>
        <th class="col-actions"></th>
      </tr>
    </thead>
    <tbody>
      {#each $enabledRepos as repo}
        {@const run = $latestRuns.get(repo.id)}
        {@const trend = $trends.get(repo.id) ?? []}
        {@const running = $runningRepos.has(repo.id)}
        <tr>
          <td>
            <button class="repo-name" onclick={() => goto(`/repo/${repo.id}`)}>{repo.name}</button>
          </td>
          <td class="text-muted mono" style="font-size:0.75rem">{repo.ruby_version ?? '—'}</td>
          <td class="col-cov"><CoverageBadge pct={run?.overall_coverage} /></td>
          <td class="col-trend"><TrendSparkline points={trend} width={80} height={24} /></td>
          <td class="col-status">
            {#if run?.status === 'failed'}
              <span class="badge badge-red">failed</span>
            {:else if running}
              <span class="badge badge-yellow">running…</span>
            {:else if run?.status === 'success'}
              <span class="badge badge-green">ok</span>
            {/if}
          </td>
          <td class="col-actions">
            <button class="btn-ghost" onclick={async () => { await cloneOrPullRepo(repo.id); await refreshRepos($activeOrg ?? undefined); }} disabled={running}>Pull</button>
            <button class="btn-primary" onclick={() => runRepo(repo.id)} disabled={running || !repo.local_path}>
              {running ? 'Running…' : 'Run'}
            </button>
            <button class="btn-ghost" onclick={() => doExport(repo.id)} disabled={exporting}>CSV</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.25rem; }
  .header-actions { display: flex; gap: 0.5rem; }
  .empty { padding: 3rem 1rem; text-align: center; }

  .repo-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }
  .repo-table th {
    text-align: left;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text-muted);
    padding: 0 0.75rem 0.5rem;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  .repo-table td {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border-subtle);
    vertical-align: middle;
  }
  .repo-table tbody tr:last-child td { border-bottom: none; }
  .repo-table tbody tr:hover td { background: #f7f8fa; }

  .repo-name {
    background: none; border: none; padding: 0;
    font-size: 0.875rem; font-weight: 500; color: var(--accent);
    cursor: pointer; text-align: left;
  }
  .repo-name:hover { text-decoration: underline; }

  .col-cov   { width: 90px; }
  .col-trend { width: 100px; }
  .col-status { width: 72px; }
  .col-actions { width: 1px; white-space: nowrap; text-align: right; }
  .col-actions button + button { margin-left: 0.25rem; }
</style>
