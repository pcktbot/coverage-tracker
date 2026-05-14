<script lang="ts">
  import { onMount } from 'svelte';
  import { adminListTables, adminListRows, type AdminRowsResponse } from '$lib/orchestrator';

  type Which = 'orchestrator' | 'coverage';

  let which = $state<Which>('orchestrator');
  let tables = $state<string[]>([]);
  let table = $state<string>('');
  let data = $state<AdminRowsResponse | null>(null);
  let page = $state(0);
  const pageSize = 100;
  let loading = $state(false);

  async function refreshTables() {
    tables = await adminListTables(which);
    if (tables.length > 0 && !tables.includes(table)) {
      table = tables[0];
      page = 0;
      await refreshRows();
    } else if (tables.length === 0) {
      table = '';
      data = null;
    }
  }

  async function refreshRows() {
    if (!table) { data = null; return; }
    loading = true;
    try {
      data = await adminListRows(which, table, pageSize, page * pageSize);
    } finally {
      loading = false;
    }
  }

  onMount(refreshTables);

  function fmt(v: unknown): string {
    if (v === null || v === undefined) return '∅';
    if (typeof v === 'string') return v;
    return JSON.stringify(v);
  }

  const totalPages = $derived(data ? Math.max(1, Math.ceil(data.total / pageSize)) : 0);
</script>

<header class="bar">
  <h1>DB Viewer</h1>
  <div class="picker">
    <label>
      DB
      <select bind:value={which} onchange={refreshTables}>
        <option value="orchestrator">orchestrator</option>
        <option value="coverage">coverage</option>
      </select>
    </label>

    <label>
      Table
      <select bind:value={table} onchange={() => { page = 0; void refreshRows(); }} disabled={tables.length === 0}>
        {#each tables as t}
          <option value={t}>{t}</option>
        {/each}
      </select>
    </label>

    {#if data}
      <span class="meta">
        {data.total.toLocaleString()} rows · page {page + 1}/{totalPages}
      </span>
      <button type="button" onclick={() => { if (page > 0) { page--; void refreshRows(); } }} disabled={page === 0}>‹ prev</button>
      <button type="button" onclick={() => { if (page + 1 < totalPages) { page++; void refreshRows(); } }} disabled={page + 1 >= totalPages}>next ›</button>
    {/if}
  </div>
</header>

{#if loading}
  <p class="muted">Loading…</p>
{:else if !data}
  <p class="muted">No data. Pick a DB and table above.</p>
{:else if data.rows.length === 0}
  <p class="muted">Table is empty.</p>
{:else}
  <div class="grid-wrap">
    <table class="grid">
      <thead>
        <tr>
          {#each data.columns as c}
            <th>{c}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each data.rows as row, i (i)}
          <tr>
            {#each data.columns as c}
              <td>{fmt(row[c])}</td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .bar { display: flex; align-items: baseline; gap: 1rem; flex-wrap: wrap; }
  h1 { margin: 0; }
  .picker { display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; }
  .picker label { display: inline-flex; gap: 0.25rem; align-items: center; font-size: 0.875rem; }
  select { padding: 0.25rem; border-radius: var(--radius-sm); border: 1px solid var(--border); background: var(--bg); }
  .meta { color: var(--text-muted); font-size: 0.8125rem; font-family: var(--font-mono, monospace); }
  button { padding: 0.25rem 0.6rem; border-radius: var(--radius-sm); border: 1px solid var(--border); background: var(--bg); cursor: pointer; font: inherit; color: inherit; }
  button:disabled { opacity: 0.4; cursor: not-allowed; }
  .muted { color: var(--text-muted); font-style: italic; padding: 1rem 0; }
  .grid-wrap { overflow: auto; margin-top: 1rem; max-height: calc(100vh - 200px); border: 1px solid var(--border); border-radius: var(--radius-sm); }
  .grid { border-collapse: collapse; width: 100%; font-size: 0.8125rem; font-family: var(--font-mono, monospace); }
  .grid th { position: sticky; top: 0; background: var(--bg-muted); padding: 0.4rem 0.6rem; text-align: left; border-bottom: 1px solid var(--border); white-space: nowrap; }
  .grid td { padding: 0.25rem 0.6rem; border-bottom: 1px solid var(--border); vertical-align: top; max-width: 480px; white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere; }
</style>
