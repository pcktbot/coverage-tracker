<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { page } from '$app/stores';
  import { repos, refreshRepos } from '$lib/stores/repos';
  import { runningRepos, markRunning, markDone } from '$lib/stores/coverage';
  import CoverageBadge from '$lib/components/CoverageBadge.svelte';
  import TrendSparkline from '$lib/components/TrendSparkline.svelte';
  import RunLog from '$lib/components/RunLog.svelte';
  import {
    listRuns,
    getTrend,
    getFileCoverage,
    runCoverage,
    cloneOrPullRepo,
    readEnvFile,
    writeEnvFile,
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
  let running = $derived($runningRepos.has(repoId));
  let error = $state('');
  let loading = $state(false);
  let exporting = $state(false);
  let pulling = $state(false);
  let pullSuccess = $state(false);
  let runLogKey = $state(0);
  let lastRunFailed = $state(false);
  let selectedRun = $derived(runs.find(r => r.id === selectedRunId));

  // Env editor
  let envOpen = $state(false);
  let envContent = $state('');
  let envOriginal = $state('');
  let envLoading = $state(false);
  let envSaving = $state(false);
  let envSaved = $state(false);
  let envError = $state('');
  let envDirty = $derived(envContent !== envOriginal);

  onMount(() => {
    const id = repoId;
    console.log('[repo-page] onMount, repoId=', id);
    if (Number.isFinite(id)) {
      load(id);
    } else {
      console.error('[repo-page] invalid repoId:', $page.params.id);
    }
  });

  async function load(id: number) {
    loading = true;
    error = '';
    console.log('[repo-page] load() starting for id=', id);
    try {
      const [r, t] = await Promise.all([listRuns(id), getTrend(id, 20)]);
      console.log('[repo-page] load() got', r.length, 'runs,', t.length, 'trend points');
      runs = r;
      trend = t;
      if (r.length > 0) {
        selectedRunId = r[0].id;
        files = await getFileCoverage(r[0].id);
        console.log('[repo-page] load() got', files.length, 'files for run', r[0].id);
      } else {
        selectedRunId = null;
        files = [];
      }
    } catch (e: any) {
      console.error('[repo-page] load() error:', e);
      error = e?.message ?? String(e);
    } finally {
      loading = false;
      console.log('[repo-page] load() done. loading=false, runs=', runs.length, 'error=', error);
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
    lastRunFailed = false;
    runLogKey++;
    error = '';
    markRunning(repoId);
    // Flush the DOM so RunLog is mounted & listening before events arrive
    await tick();
    try {
      await runCoverage(repoId);
      runs = await listRuns(repoId);
      trend = await getTrend(repoId, 20);
      if (runs.length > 0) {
        selectedRunId = runs[0].id;
        files = await getFileCoverage(selectedRunId);
        markDone(repoId, runs[0]);
      } else {
        // No runs found — clear running state
        runningRepos.update((s) => { s.delete(repoId); return new Set(s); });
      }
    } catch (e: any) {
      error = e.message;
      lastRunFailed = true;
      // Always clear running state on error
      runningRepos.update((s) => { s.delete(repoId); return new Set(s); });
      // Refresh runs so we capture the failed entry
      try {
        runs = await listRuns(repoId);
        if (runs.length > 0) selectedRunId = runs[0].id;
      } catch { /* non-fatal */ }
    }
  }

  async function doPull() {
    if (!repo) return;
    pulling = true;
    pullSuccess = false;
    error = '';
    try {
      await cloneOrPullRepo(repo.id);
      // Refresh the repos store so local_path / ruby_version update in the UI
      const org = repo.org;
      await refreshRepos(org);
      pullSuccess = true;
      setTimeout(() => (pullSuccess = false), 3000);
    } catch (e: any) {
      error = e.message;
    } finally {
      pulling = false;
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

  function toggleEnv() {
    envOpen = !envOpen;
    if (envOpen && !envContent) {
      loadEnv();
    }
  }

  async function loadEnv() {
    envLoading = true;
    envError = '';
    try {
      const content = await readEnvFile(repoId);
      envContent = content;
      envOriginal = content;
    } catch (e: any) {
      envError = e.message;
    } finally {
      envLoading = false;
    }
  }

  async function saveEnv() {
    envSaving = true;
    envError = '';
    envSaved = false;
    try {
      await writeEnvFile(repoId, envContent);
      envOriginal = envContent;
      envSaved = true;
      setTimeout(() => (envSaved = false), 2000);
    } catch (e: any) {
      envError = e.message;
    } finally {
      envSaving = false;
    }
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

  // Track which files are expanded to show uncovered lines
  let expandedFiles = $state<Set<string>>(new Set());

  function toggleFile(path: string) {
    expandedFiles = new Set(expandedFiles);
    if (expandedFiles.has(path)) {
      expandedFiles.delete(path);
    } else {
      expandedFiles.add(path);
    }
  }

  // Compute summary totals from file data
  let totalCovered = $derived(files.reduce((s, f) => s + (f.lines_covered ?? 0), 0));
  let totalLines = $derived(files.reduce((s, f) => s + (f.lines_total ?? 0), 0));
  let totalPct = $derived(totalLines > 0 ? (totalCovered / totalLines * 100) : 0);

  /** Group consecutive line numbers into ranges: [1,2,3,7,8,12] → "L1-3, L7-8, L12" */
  function formatLineRanges(lines: number[]): string {
    if (!lines || lines.length === 0) return '';
    const sorted = [...lines].sort((a, b) => a - b);
    const ranges: string[] = [];
    let start = sorted[0];
    let end = sorted[0];
    for (let i = 1; i < sorted.length; i++) {
      if (sorted[i] === end + 1) {
        end = sorted[i];
      } else {
        ranges.push(start === end ? `L${start}` : `L${start}-${end}`);
        start = sorted[i];
        end = sorted[i];
      }
    }
    ranges.push(start === end ? `L${start}` : `L${start}-${end}`);
    return ranges.join(', ');
  }

  /** Strip the repo local_path prefix from absolute file paths to show relative paths */
  function shortPath(fullPath: string): string {
    const root = repo?.local_path;
    if (root && fullPath.startsWith(root)) {
      return fullPath.slice(root.length).replace(/^\//, '');
    }
    // Fallback: show just the last 3 path segments
    const parts = fullPath.split('/');
    return parts.length > 3 ? '…/' + parts.slice(-3).join('/') : fullPath;
  }
</script>

<div class="detail-header">
  <div>
    <a href="/" class="back-btn btn-ghost">← Back</a>
    <h1>{repo?.name ?? '…'}</h1>
    {#if repo?.ruby_version}
      <span class="text-muted mono" style="font-size:0.8125rem">ruby {repo.ruby_version}</span>
    {/if}
  </div>
  <div class="header-actions">
    <button class="btn-secondary" onclick={doPull} disabled={running || pulling}>
      {pulling ? 'Pulling…' : 'Pull'}
    </button>
    {#if pullSuccess}<span class="badge badge-green" style="align-self:center">Pulled!</span>{/if}
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

{#if loading}
  <div class="section">
    <p class="text-muted">Loading coverage data…</p>
  </div>
{/if}

{#if !loading}

{#if running || lastRunFailed}
  <div class="section">
    <h2>{running ? 'Live output' : 'Run output'}</h2>
    {#key runLogKey}
      <RunLog {repoId} runId={null} />
    {/key}
  </div>
{/if}

<!-- Error detail for failed runs -->
{#if (selectedRun?.status === 'failed' || selectedRun?.status === 'interrupted') && selectedRun.error_message && !running && !lastRunFailed}
  <div class="section">
    <h2>Error detail</h2>
    <pre class="error-detail">{selectedRun.error_message}</pre>
  </div>
{/if}

<!-- Coverage data: run history + file coverage (primary content) -->
<div class="two-col">
  <!-- Run history -->
  <div class="section card">
    <h2 style="padding:0.75rem 1rem;border-bottom:1px solid var(--border)">Run history</h2>
    {#if runs.length === 0}
      <p class="text-muted" style="padding:1rem">No runs yet. Click <strong>Run</strong> to start.</p>
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
                <span class="badge {run.status === 'success' ? 'badge-green' : run.status === 'failed' || run.status === 'interrupted' ? 'badge-red' : 'badge-yellow'}"
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
    <div style="padding:0.75rem 1rem;border-bottom:1px solid var(--border);display:flex;justify-content:space-between;align-items:center">
      <h2 style="margin:0">File coverage</h2>
      {#if files.length > 0}
        <div style="display:flex;align-items:center;gap:0.75rem">
          <span class="mono" style="font-size:0.75rem;color:var(--text-muted)">{totalCovered}/{totalLines} lines</span>
          <CoverageBadge pct={totalPct} size="sm" />
        </div>
      {/if}
    </div>
    {#if files.length === 0}
      <p class="text-muted" style="padding:1rem">No file data.</p>
    {:else}
      <div class="file-list">
        {#each files as f}
          <div class="file-entry">
            <button class="file-row" onclick={() => (f.uncovered_lines?.length ?? 0) > 0 && toggleFile(f.file_path)}>
              {#if (f.uncovered_lines?.length ?? 0) > 0}
                <span class="file-chevron" class:open={expandedFiles.has(f.file_path)}>&#9654;</span>
              {:else}
                <span class="file-chevron-spacer"></span>
              {/if}
              <span class="file-path mono" title={f.file_path}>{shortPath(f.file_path)}</span>
              <div class="file-right">
                <span class="file-lines mono">{f.lines_covered ?? 0}/{f.lines_total ?? 0}</span>
                <div class="coverage-bar-wrap">
                  <div class="coverage-bar"
                    style="width:{f.coverage_percent ?? 0}%;background:{
                      (f.coverage_percent ?? 0) >= 80 ? 'var(--success)' :
                      (f.coverage_percent ?? 0) >= 60 ? 'var(--warning)' : 'var(--danger)'
                    }"></div>
                </div>
                <span class="file-pct">{(f.coverage_percent ?? 0).toFixed(1)}%</span>
              </div>
            </button>
            {#if expandedFiles.has(f.file_path) && (f.uncovered_lines?.length ?? 0) > 0}
              <div class="uncovered-lines">
                <span class="uncovered-label">Uncovered ({f.uncovered_lines?.length ?? 0} lines):</span>
                <span class="uncovered-ranges mono">{formatLineRanges(f.uncovered_lines ?? [])}</span>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

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

<!-- Env editor (collapsible, secondary) -->
<div class="section">
  <button class="env-toggle" onclick={toggleEnv}>
    <span class="env-chevron" class:open={envOpen}>&#9654;</span>
    <h2 style="display:inline;margin:0">.env.test</h2>
  </button>
  {#if envOpen}
    {#if envLoading}
      <p class="text-muted" style="padding:0.5rem 0">Loading…</p>
    {:else}
      {#if envError}
        <div class="error-msg" style="margin-bottom:0.5rem">{envError}</div>
      {/if}
      <textarea
        class="env-editor"
        bind:value={envContent}
        spellcheck="false"
        placeholder="# KEY=value"
        rows="10"
      ></textarea>
      <div class="env-actions">
        <button class="btn-primary" onclick={saveEnv} disabled={envSaving || !envDirty}>
          {envSaving ? 'Saving…' : 'Save .env.test'}
        </button>
        <button class="btn-ghost" onclick={loadEnv} disabled={envLoading}>Reload</button>
        {#if envSaved}<span class="badge badge-green">Saved!</span>{/if}
        {#if envDirty}<span class="text-muted" style="font-size:0.75rem">unsaved changes</span>{/if}
      </div>
    {/if}
  {/if}
</div>

{/if} <!-- end !loading -->

<style>
  .detail-header {
    display: flex; justify-content: space-between; align-items: flex-start;
    margin-bottom: 1.25rem; gap: 1rem;
  }
  .back-btn {
    margin-bottom: 0.25rem; padding: 0.125rem 0;
    display: inline-block; text-decoration: none;
    color: var(--text-secondary); font-size: 0.8125rem;
  }
  .back-btn:hover { color: var(--accent); text-decoration: none; }
  .header-actions { display: flex; gap: 0.5rem; flex-shrink: 0; }
  .section { margin-bottom: 1rem; }
  .two-col { display: grid; grid-template-columns: 280px 1fr; gap: 1rem; min-width: 0; }
  .two-col > * { min-width: 0; overflow: hidden; }
  tr.selected td { background: var(--accent-subtle); }
  .file-list { max-height: calc(100vh - 280px); overflow-y: auto; overflow-x: hidden; }
  .file-entry { border-bottom: 1px solid var(--border-subtle); }
  .file-entry:last-child { border-bottom: none; }
  .file-row {
    display: flex; align-items: center;
    padding: 0.3rem 0.75rem; gap: 0.5rem; width: 100%;
    background: none; border: none; cursor: pointer; color: inherit;
    text-align: left; min-width: 0;
  }
  .file-row:hover { background: var(--bg-muted); }
  .file-chevron {
    font-size: 0.5rem; transition: transform 0.15s; display: inline-block;
    flex-shrink: 0; width: 0.75rem; color: var(--text-muted);
  }
  .file-chevron.open { transform: rotate(90deg); }
  .file-chevron-spacer { width: 0.75rem; flex-shrink: 0; }
  .file-path {
    font-size: 0.6875rem; min-width: 0; flex: 1;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    direction: rtl; text-align: left;
  }
  .file-right { display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0; }
  .file-lines { font-size: 0.625rem; color: var(--text-muted); white-space: nowrap; }
  .coverage-bar-wrap { width: 48px; height: 6px; background: var(--bg-muted); border-radius: 3px; overflow: hidden; flex-shrink: 0; }
  .coverage-bar { height: 100%; border-radius: 3px; transition: width 0.3s; }
  .file-pct { font-size: 0.6875rem; width: 40px; text-align: right; font-weight: 500; flex-shrink: 0; }
  .uncovered-lines {
    padding: 0.25rem 0.75rem 0.5rem 1.75rem;
    font-size: 0.6875rem; line-height: 1.5;
    background: var(--bg-subtle);
  }
  .uncovered-label { color: var(--danger); font-weight: 500; margin-right: 0.5rem; }
  .uncovered-ranges { color: var(--text-secondary); word-break: break-all; }
  @media (max-width: 900px) { .two-col { grid-template-columns: 1fr; } }
  @media (max-width: 600px) {
    .detail-header { flex-direction: column; }
    .header-actions { width: 100%; justify-content: flex-start; }
  }

  /* Env editor */
  .env-toggle {
    background: none; border: none; padding: 0; cursor: pointer;
    display: flex; align-items: center; gap: 0.5rem;
    color: var(--text-primary);
  }
  .env-toggle:hover h2 { color: var(--accent); }
  .env-chevron {
    font-size: 0.625rem; transition: transform 0.15s; display: inline-block;
  }
  .env-chevron.open { transform: rotate(90deg); }
  .env-editor {
    width: 100%;
    font-family: var(--font-mono);
    font-size: 0.8125rem;
    line-height: 1.5;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-subtle);
    color: var(--text-primary);
    resize: vertical;
    tab-size: 4;
    margin-top: 0.5rem;
  }
  .env-editor:focus { outline: 2px solid var(--accent); outline-offset: -1px; }
  .env-actions { display: flex; align-items: center; gap: 0.5rem; margin-top: 0.5rem; }

  /* Error detail */
  .error-detail {
    background: var(--bg-subtle);
    border: 1px solid var(--danger);
    border-radius: var(--radius-sm);
    padding: 0.75rem;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.6;
    overflow-x: auto;
    max-height: 300px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
    color: var(--danger);
  }
</style>
