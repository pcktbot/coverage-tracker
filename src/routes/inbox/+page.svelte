<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import InboxRow from '$lib/components/InboxRow.svelte';
  import { listSessions, type Session, type SessionStatus } from '$lib/orchestrator';

  type Filter = 'all' | 'needs_input' | 'dismissed' | SessionStatus;
  const FILTERS: Filter[] = ['needs_input', 'all', 'working', 'idle', 'done', 'error', 'dismissed'];

  let sessions = $state<Session[]>([]);
  let filter = $state<Filter>('needs_input');
  let unlisten: UnlistenFn | undefined;

  async function refresh() {
    try { sessions = await listSessions(); } catch { sessions = []; }
  }

  onMount(async () => {
    await refresh();
    unlisten = await listen('orchestrator://state', () => { void refresh(); });
  });
  onDestroy(() => { unlisten?.(); });

  const visible = $derived.by(() => {
    const showingDismissed = filter === 'dismissed';
    const base = sessions.filter((s) => {
      const isDismissed = s.dismissed_at != null;
      return showingDismissed ? isDismissed : !isDismissed;
    });
    const filtered = (filter === 'all' || filter === 'dismissed')
      ? base
      : base.filter((s) => s.status === filter);
    return [...filtered].sort((a, b) => {
      if (a.status === 'needs_input' && b.status !== 'needs_input') return -1;
      if (b.status === 'needs_input' && a.status !== 'needs_input') return 1;
      return b.updated_at - a.updated_at;
    });
  });

  const needsInputCount = $derived(
    sessions.filter((s) => s.status === 'needs_input' && s.dismissed_at == null).length
  );
</script>

<header class="bar">
  <h1>Sessions {#if needsInputCount > 0}<span class="badge">{needsInputCount}</span>{/if}</h1>
  <div class="filters">
    {#each FILTERS as f}
      <button class:active={filter === f} onclick={() => (filter = f)} type="button">{f}</button>
    {/each}
  </div>
</header>

<ul class="rows">
  {#each visible as s (s.id)}
    <li><InboxRow session={s} onChange={refresh} /></li>
  {/each}
  {#if visible.length === 0}
    <li class="empty">No sessions match.</li>
  {/if}
</ul>

<style>
  .bar { display: flex; align-items: baseline; gap: 1rem; }
  h1 { margin: 0; }
  .badge { background: #f39c12; color: white; border-radius: 999px; font-size: 0.75rem; padding: 0.125rem 0.5rem; margin-left: 0.25rem; vertical-align: middle; }
  .filters { display: flex; gap: 0.25rem; flex-wrap: wrap; }
  .filters button { padding: 0.25rem 0.6rem; border-radius: var(--radius-sm); border: 1px solid var(--border); background: var(--bg); cursor: pointer; font: inherit; color: inherit; }
  .filters button.active { background: var(--accent-subtle); color: var(--accent); }
  .rows {
    list-style: none; padding: 0; margin: 1rem 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 480px));
    gap: 1rem;
    align-items: start;
  }
  .rows li { list-style: none; }
  .empty { padding: 1rem; color: var(--text-muted); font-style: italic; grid-column: 1 / -1; }
</style>
