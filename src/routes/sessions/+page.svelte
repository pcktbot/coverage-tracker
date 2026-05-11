<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import SessionRow from '$lib/components/SessionRow.svelte';
  import SessionDrawer from '$lib/components/SessionDrawer.svelte';
  import { listSessions, type Session, type SessionStatus } from '$lib/orchestrator';

  type Filter = 'all' | SessionStatus;
  const FILTERS: Filter[] = ['all', 'working', 'idle', 'needs_input', 'done', 'error'];

  let sessions = $state<Session[]>([]);
  let filter = $state<Filter>('all');
  let selected = $state<Session | null>(null);
  let unlisten: UnlistenFn | undefined;

  async function refresh() {
    try { sessions = await listSessions(); } catch { sessions = []; }
  }

  onMount(async () => {
    await refresh();
    unlisten = await listen('orchestrator://state', () => { void refresh(); });
  });
  onDestroy(() => { unlisten?.(); });

  const visible = $derived(
    filter === 'all' ? sessions : sessions.filter((s) => s.status === filter)
  );
</script>

<header class="bar">
  <h1>Sessions</h1>
  <div class="filters">
    {#each FILTERS as f}
      <button class:active={filter === f} onclick={() => (filter = f)} type="button">{f}</button>
    {/each}
  </div>
</header>

<ul class="rows">
  {#each visible as s (s.id)}
    <SessionRow session={s} onclick={() => (selected = s)} />
  {/each}
  {#if visible.length === 0}
    <li class="empty">No sessions match.</li>
  {/if}
</ul>

{#if selected}
  <SessionDrawer session={selected} onclose={() => (selected = null)} />
{/if}

<style>
  .bar { display: flex; align-items: baseline; gap: 1rem; }
  h1 { margin: 0; }
  .filters { display: flex; gap: 0.25rem; }
  .filters button { padding: 0.25rem 0.6rem; border-radius: var(--radius-sm); border: 1px solid var(--border); background: var(--bg); cursor: pointer; font: inherit; color: inherit; }
  .filters button.active { background: var(--accent-subtle); color: var(--accent); }
  .rows { list-style: none; padding: 0; margin: 1rem 0; }
  .empty { padding: 1rem; color: var(--text-muted); font-style: italic; list-style: none; }
</style>
