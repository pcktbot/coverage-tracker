<script lang="ts">
  import type { Session } from '$lib/orchestrator';

  let { session, onclick }: { session: Session; onclick: () => void } = $props();

  const STATUS_COLORS: Record<Session['status'], string> = {
    working: '#3498db',
    idle: '#95a5a6',
    needs_input: '#f39c12',
    done: '#2ecc71',
    error: '#e74c3c',
    unknown: '#bdc3c7',
  };
  const dot = $derived(STATUS_COLORS[session.status]);
</script>

<li>
  <button class="row" {onclick} type="button">
    <span class="dot" style:background-color={dot}></span>
    <span class="label">{session.label ?? session.id}</span>
    <span class="cwd">{session.cwd}</span>
    <span class="tool">{session.current_tool ?? ''}</span>
    <span class="progress">{session.last_progress ?? ''}</span>
  </button>
</li>

<style>
  li { list-style: none; }
  .row {
    display: grid;
    grid-template-columns: 14px 1fr 2fr 1fr 2fr;
    gap: 0.5rem;
    align-items: center;
    width: 100%;
    padding: 0.5rem;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border);
    text-align: left;
    cursor: pointer;
    font: inherit;
    color: inherit;
  }
  .row:hover { background: var(--bg-muted); }
  .dot { width: 10px; height: 10px; border-radius: 50%; }
  .cwd, .tool, .progress { color: var(--text-secondary); font-size: 0.8125rem; }
  .label { font-weight: 500; }
</style>
