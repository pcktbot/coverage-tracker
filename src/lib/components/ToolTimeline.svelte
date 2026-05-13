<script lang="ts">
  import { onMount } from 'svelte';
  import { listEvents, type OrchestratorEvent } from '$lib/orchestrator';

  let { sessionId, limit = 8 }: { sessionId: string; limit?: number } = $props();
  let toolEvents = $state<OrchestratorEvent[]>([]);

  onMount(async () => {
    try {
      const all = await listEvents(sessionId, 400);
      toolEvents = all.filter((e) => e.kind === 'pre_tool').slice(0, limit).reverse();
    } catch {
      toolEvents = [];
    }
  });

  function toolName(payload: string): string {
    try {
      const v = JSON.parse(payload);
      return typeof v === 'string' ? v : payload;
    } catch {
      return payload;
    }
  }
</script>

<section class="tool-timeline">
  <h4>Recent tool calls</h4>
  {#if toolEvents.length === 0}
    <p class="muted">No tool calls yet.</p>
  {:else}
    <ol>
      {#each toolEvents as e (e.id)}
        <li>
          <span class="tool">{toolName(e.payload)}</span>
          <span class="ts">{new Date(e.ts * 1000).toLocaleTimeString()}</span>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .tool-timeline { margin-top: 0.5rem; }
  h4 { margin: 0 0 0.25rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted); }
  ol { list-style: none; padding: 0; margin: 0; font-size: 0.8125rem; }
  ol li { display: flex; justify-content: space-between; padding: 0.125rem 0; border-bottom: 1px solid var(--border); }
  .tool { font-family: var(--font-mono, monospace); }
  .ts { color: var(--text-muted); }
  .muted { color: var(--text-muted); font-style: italic; font-size: 0.8125rem; margin: 0; }
</style>
