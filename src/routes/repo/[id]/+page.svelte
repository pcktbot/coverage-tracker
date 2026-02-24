<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { repos } from '$lib/stores/repos';
  import { markRunning, markDone } from '$lib/stores/coverage';
  import CoverageBadge from '$lib/components/CoverageBadge.svelte';
  import TrendSparkline from '$lib/components/TrendSparkline.svelte';
  import RunLog from '$lib/components/RunLog.svelte';
  import {
    listRuns,
    getTrend,
    getFileCoverage,
    runCoverage,
    cloneOrPullRepo,
    exportCsv,
    downloadCsv,
    type CoverageRun,
    type FileCoverage,
    type CoverageTrendPoint,
  } from '$lib/api';

  let repoId = $derived(Number($page.params.id));
  let repo = $derived($repos.find((r) => r.id === repoId));

  let runs = $state<CoverageRun[]>([]);
  let trend = $state<CoverageTrendPoint[]>([]);
  let files = $state<FileCoverage[]>([]);
  let selectedRunId = $state<number | null>(null);
  let running = $state(false);
  let error = $state('');
  let exporting = $state(false);
  let runLogKey = $state(0);

  onMount(async () => {
    await load();
  });

  async function load() {
    try {
      runs = await listRuns(repoId);
      trend = await getTrend(repoId, 20);
      if (runs.length > 0) {
        selectedRunId = runs[0].id;
        files = await getFileCoverage(selectedRunId);
      }
    } catch (e: any) {
      error = e.message;
    }
  }

  async function selectRun(runId: number) {
    selectedRunId = runId;
    try {
      files = await getFileCoverage(runId);
    } catch (e: any) {
      error = e.message;
    }
  }

  async function doRun() {
    if (!repo) return;
    running = true;
    runLogKey++;
    error = '';
    markRunning(repoId);
    try {
      await runCoverage(repoId);
      runs = await listRuns(repoId);
      trend = await getTrend(repoId, 20);
      if (runs.length > 0) {
        selectedRunId = runs[0].id;
        files = await getFileCoverage(selectedRunId);
        markDone(repoId, runs[0]);
      }
    } catch (e: any) {
      error = e.message;
    } finally {
      running = false;
    }
  }

  async function doPull() {
    if (!repo) return;
    error = '';
    try {
      await cloneOrPullRepo(repo.id);
    } catch (e: any) {
      error = e.message;
    }
  }

  async function doExport() {
    exporting = true;
    error = '';
    try {
      const csv = await exportCsv(repoId, true);
      downloadCsv(csv, `coverage-${repo?.name ?? repoId}.csv`);
    } catch (e: any) {
      error = e.message;
    } finally {
      exporting = false;
    }
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  // Simple SVG trend chart
  const CHART_W = 480;
  const CHART_H = 120;
  const PAD = 24;

  let trendPoints = $derived(
    trend.map((p, i) => {
      const x = trend.length < 2
        ? PAD + (CHART_W - PAD * 2) / 2
        : PAD + (i / (trend.length - 1)) * (CHART_W - PAD * 2);
      const pct = p.overall_coverage ?? 0;
      const y = CHART_H - PAD - (pct / 100) * (CHART_H - PAD * 2);
      return { x, y, pct, date: p.started_at };
    })
  );
  let polyline = $derived(trendPoints.map((p) => `${p.x},${p.y}`).join(' '));
</script>

<div class="detail-header">
  <div>
    <button class="back-btn btn-ghost" onclick={() => goto('/')}>← Back</button>
    <h1>{repo?.name ?? '…'}</h1>
    {#if repo?.ruby_version}
      <span class="text-muted mono" style="font-size:0.8125rem">ruby {repo.ruby_version}</span>
    {/if}
  </div>
  <div class="header-actions">
    <button class="btn-secondary" onclick={doPull} disabled={running}>Pull</button>
    <button class="btn-primary" onclick={doRun} disabled={running || !repo?.local_path}>
      {running ? 'Running…' : 'Run'}
    </button>
    <button class="btn-secondary" onclick={doExport} disabled={exporting}>
      {exporting ? 'Exporting…' : 'Export CSV'}
    </button>
  </div>
</div>

{#if error}
  <div class="error-msg" style="margin-bottom:1rem">{error}</div>
{/if}

{#if running}
  <div class="section">
    <h2>Live output</h2>
    {#key runLogKey}
      <RunLog {repoId} runId={selectedRunId} />
    {/key}
  </div>
{/if}

<!-- Trend chart -->
{#if trendPoints.length > 1}
  <div class="section card" style="padding:1rem">
    <h2 style="margin-bottom:0.75rem">Coverage trend</h2>
    <svg width={CHART_W} height={CHART_H} style="width:100%;max-width:{CHART_W}px">
      <!-- Y axis labels -->
      {#each [0, 25, 50, 75, 100] as tick}
        {@const y = CHART_H - PAD - (tick / 100) * (CHART_H - PAD * 2)}
        <line x1={PAD} x2={CHART_W - PAD} y1={y} y2={y} stroke="var(--border-subtle)" stroke-width="1"/>
        <text x={PAD - 4} y={y + 4} text-anchor="end" font-size="10" fill="var(--text-muted)">{tick}%</text>
      {/each}
      <polyline points={polyline} fill="none" stroke="var(--accent)" stroke-width="2"
        stroke-linejoin="round" stroke-linecap="round"/>
      {#each trendPoints as p}
        <circle cx={p.x} cy={p.y} r="3" fill="var(--accent)">
          <title>{p.pct.toFixed(1)}% — {formatDate(p.date)}</title>
        </circle>
      {/each}
    </svg>
  </div>
{/if}

<div class="two-col">
  <!-- Run history -->
  <div class="section card">
    <h2 style="padding:0.75rem 1rem;border-bottom:1px solid var(--border)">Run history</h2>
    {#if runs.length === 0}
      <p class="text-muted" style="padding:1rem">No runs yet.</p>
    {:else}
      <table>
        <thead>
          <tr><th>Date</th><th>Coverage</th><th>Status</th></tr>
        </thead>
        <tbody>
          {#each runs as run}
            <tr
              class:selected={run.id === selectedRunId}
              onclick={() => selectRun(run.id)}
              style="cursor:pointer"
            >
              <td class="mono" style="font-size:0.75rem">{formatDate(run.started_at)}</td>
              <td><CoverageBadge pct={run.overall_coverage} size="sm" /></td>
              <td>
                <span class="badge {run.status === 'success' ? 'badge-green' : run.status === 'failed' ? 'badge-red' : 'badge-yellow'}"
                  style="font-size:0.6875rem">{run.status}</span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- File coverage -->
  <div class="section card">
    <h2 style="padding:0.75rem 1rem;border-bottom:1px solid var(--border)">File coverage</h2>
    {#if files.length === 0}
      <p class="text-muted" style="padding:1rem">No file data.</p>
    {:else}
      <div class="file-list">
        {#each files as f}
          <div class="file-row">
            <span class="file-path mono">{f.file_path}</span>
            <div class="file-right">
              <div class="coverage-bar-wrap">
                <div class="coverage-bar"
                  style="width:{f.coverage_percent ?? 0}%;background:{
                    (f.coverage_percent ?? 0) >= 80 ? 'var(--success)' :
                    (f.coverage_percent ?? 0) >= 60 ? 'var(--warning)' : 'var(--danger)'
                  }"></div>
              </div>
              <span class="file-pct">{(f.coverage_percent ?? 0).toFixed(1)}%</span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .detail-header {
    display: flex; justify-content: space-between; align-items: flex-start;
    margin-bottom: 1.25rem; gap: 1rem;
  }
  .back-btn { margin-bottom: 0.25rem; padding: 0.125rem 0; }
  .header-actions { display: flex; gap: 0.5rem; flex-shrink: 0; }
  .section { margin-bottom: 1rem; }
  .two-col { display: grid; grid-template-columns: 1fr 1.5fr; gap: 1rem; }
  tr.selected td { background: var(--accent-subtle); }
  .file-list { max-height: 480px; overflow-y: auto; }
  .file-row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.3rem 0.75rem; gap: 0.75rem; border-bottom: 1px solid var(--border-subtle);
  }
  .file-row:last-child { border-bottom: none; }
  .file-path { font-size: 0.6875rem; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  .file-right { display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0; }
  .coverage-bar-wrap { width: 60px; height: 6px; background: var(--bg-muted); border-radius: 3px; overflow: hidden; }
  .coverage-bar { height: 100%; border-radius: 3px; transition: width 0.3s; }
  .file-pct { font-size: 0.6875rem; width: 36px; text-align: right; font-weight: 500; }
  @media (max-width: 700px) { .two-col { grid-template-columns: 1fr; } }
</style>
