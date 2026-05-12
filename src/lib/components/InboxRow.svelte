<script lang="ts">
  import { onMount } from 'svelte';
  import { getTranscriptTail, type Session, type TranscriptTurn } from '$lib/orchestrator';
  import InboxComposer from './InboxComposer.svelte';

  let { session }: { session: Session } = $props();

  let lastAssistantTurn = $state<TranscriptTurn | null>(null);
  let expanded = $state(false);

  onMount(async () => {
    const turns = await getTranscriptTail(session.id, 4);
    for (let i = turns.length - 1; i >= 0; i--) {
      if (turns[i].role === 'assistant') { lastAssistantTurn = turns[i]; break; }
    }
  });

  const kindLabel = $derived({
    project: 'Project', ado: 'ADO', confluence: 'Confluence', github_pr: 'GitHub PR'
  }[session.artifact_kind ?? 'project']);

  const cwdShort = $derived(
    session.cwd.length > 40 ? '…' + session.cwd.slice(-39) : session.cwd
  );
</script>

<article class="row">
  <header>
    <span class="dot" aria-label="needs input"></span>
    <h3 class="label">{session.label ?? session.id}</h3>
    {#if session.artifact_kind}
      <span class="pill" title={session.artifact_url ?? ''}>
        <span class="kind">{kindLabel}</span>
        <span class="title">{session.artifact_title ?? session.artifact_id}</span>
      </span>
    {:else}
      <span class="pill muted">Unlinked</span>
    {/if}
    <span class="cwd" title={session.cwd}>{cwdShort}</span>
  </header>

  {#if session.last_user_prompt}
    <section>
      <h4>Last user prompt</h4>
      <p class="prompt">{session.last_user_prompt}</p>
    </section>
  {/if}

  <section>
    <button class="expand" onclick={() => (expanded = !expanded)} type="button">
      <span class="chev">{expanded ? '▼' : '▶'}</span>
      Last assistant turn
    </button>
    {#if expanded && lastAssistantTurn}
      <pre class="turn">{lastAssistantTurn.text}</pre>
    {:else if expanded}
      <p class="muted">No assistant turn captured (transcript unavailable).</p>
    {/if}
  </section>

  <section class="composer">
    <InboxComposer sessionId={session.id} />
  </section>
</article>

<style>
  .row {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1rem;
    margin-bottom: 1rem;
    background: var(--bg);
  }
  header { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  h3 { margin: 0; font-size: 1rem; }
  h4 { margin: 0 0 0.25rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted); }
  .dot {
    width: 10px; height: 10px; border-radius: 50%;
    background: #f39c12;
  }
  .label { flex: 1 1 auto; }
  .cwd { color: var(--text-muted); font-size: 0.8125rem; font-family: var(--font-mono, monospace); }
  .pill {
    background: var(--bg-muted); padding: 0.125rem 0.5rem;
    border-radius: 999px; font-size: 0.75rem; display: inline-flex; gap: 0.25rem;
  }
  .pill.muted { color: var(--text-muted); }
  .pill .kind { font-weight: 600; }
  section { margin-top: 0.75rem; }
  .prompt { background: var(--bg-muted); padding: 0.5rem; border-radius: var(--radius-sm); margin: 0; font-size: 0.875rem; }
  .expand { background: none; border: none; padding: 0; cursor: pointer; font: inherit; color: var(--text-secondary); }
  .chev { display: inline-block; width: 1em; }
  .turn { background: var(--bg-muted); padding: 0.5rem; border-radius: var(--radius-sm); margin: 0.25rem 0 0; font-size: 0.8125rem; white-space: pre-wrap; word-wrap: break-word; max-height: 200px; overflow: auto; }
  .muted { color: var(--text-muted); }
  .composer { margin-top: 1rem; padding-top: 0.75rem; border-top: 1px solid var(--border); }
</style>
