<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import InboxRow from '$lib/components/InboxRow.svelte';
  import { listSessions, type Session } from '$lib/orchestrator';

  let sessions = $state<Session[]>([]);
  let unlisten: UnlistenFn | undefined;

  async function refresh() {
    try {
      const all = await listSessions();
      sessions = all.filter((s) => s.status === 'needs_input');
    } catch {
      sessions = [];
    }
  }

  onMount(async () => {
    await refresh();
    unlisten = await listen('orchestrator://state', () => { void refresh(); });
  });
  onDestroy(() => { unlisten?.(); });
</script>

<header class="bar">
  <h1>Inbox</h1>
  <span class="count">{sessions.length} session{sessions.length === 1 ? '' : 's'} waiting</span>
  <button onclick={() => void refresh()} type="button" class="refresh">Refresh</button>
</header>

{#if sessions.length === 0}
  <p class="empty">No sessions need input — nice. Reply text you compose here is queued and prepended to a session's next user prompt; it isn't pushed live. Type something in the terminal to deliver.</p>
{:else}
  {#each sessions as session (session.id)}
    <InboxRow {session} />
  {/each}
{/if}

<style>
  .bar { display: flex; align-items: baseline; gap: 1rem; margin-bottom: 1rem; }
  h1 { margin: 0; }
  .count { color: var(--text-muted); }
  .refresh { margin-left: auto; padding: 0.25rem 0.75rem; border: 1px solid var(--border); background: var(--bg); border-radius: var(--radius-sm); cursor: pointer; font: inherit; color: inherit; }
  .empty { padding: 2rem; color: var(--text-muted); font-style: italic; text-align: center; max-width: 600px; margin: 2rem auto; line-height: 1.5; }
</style>
