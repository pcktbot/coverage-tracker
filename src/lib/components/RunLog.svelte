<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';

  export let repoId: number;
  export let runId: number | null = null;

  interface LineEvent {
    repo_id: number;
    run_id: number;
    line: string;
  }

  let lines: string[] = [];
  let el: HTMLPreElement;
  let unlisten: (() => void) | null = null;

  onMount(async () => {
    unlisten = await listen<LineEvent>('rspec-output', (event) => {
      if (event.payload.repo_id !== repoId) return;
      if (runId !== null && event.payload.run_id !== runId) return;
      lines = [...lines, event.payload.line];
      // auto-scroll
      requestAnimationFrame(() => {
        if (el) el.scrollTop = el.scrollHeight;
      });
    });
  });

  onDestroy(() => {
    unlisten?.();
  });

  export function clear() {
    lines = [];
  }
</script>

<pre class="run-log" bind:this={el}>{#if lines.length === 0}<span class="text-muted">Waiting for output…</span>{:else}{lines.join('\n')}{/if}</pre>

<style>
  .run-log {
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 0.75rem;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.6;
    overflow-y: auto;
    max-height: 400px;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
    color: var(--text-secondary);
  }
</style>
