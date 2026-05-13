<script lang="ts">
  import { onMount } from 'svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import {
    getTranscriptTail, listEvents, listArtifacts,
    type Session, type TranscriptTurn, type OrchestratorEvent, type Artifact,
  } from '$lib/orchestrator';
  import InboxComposer from './InboxComposer.svelte';
  import ArtifactLinker from './ArtifactLinker.svelte';
  import ToolTimeline from './ToolTimeline.svelte';
  import LoadedSnapshot from './LoadedSnapshot.svelte';

  let { session }: { session: Session } = $props();

  let lastAssistantTurn = $state<TranscriptTurn | null>(null);
  let events = $state<OrchestratorEvent[]>([]);
  let artifacts = $state<Artifact[]>([]);
  let expanded = $state(false);

  onMount(async () => {
    const turns = await getTranscriptTail(session.id, 4);
    for (let i = turns.length - 1; i >= 0; i--) {
      if (turns[i].role === 'assistant') { lastAssistantTurn = turns[i]; break; }
    }
  });

  async function toggle() {
    expanded = !expanded;
    if (expanded && events.length === 0) {
      events = await listEvents(session.id);
      artifacts = await listArtifacts(session.id);
    }
  }

  const kindLabel = $derived({
    project: 'Project', ado: 'ADO', confluence: 'Confluence', github_pr: 'GitHub PR'
  }[session.artifact_kind ?? 'project']);

  const cwdShort = $derived(
    session.cwd.length > 40 ? '…' + session.cwd.slice(-39) : session.cwd
  );

  const dotColor = $derived({
    working: '#3498db',
    idle: '#95a5a6',
    needs_input: '#f39c12',
    done: '#2ecc71',
    error: '#e74c3c',
    unknown: '#7f8c8d',
  }[session.status]);
</script>

<article class="row" class:needs-input={session.status === 'needs_input'}>
  <header>
    <span class="dot" style="background: {dotColor}" aria-label={session.status}></span>
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

  {#if lastAssistantTurn}
    <section>
      <h4>Last assistant turn</h4>
      <pre class="turn">{lastAssistantTurn.text}</pre>
    </section>
  {/if}

  <section>
    <button class="expand" onclick={toggle} type="button">
      <span class="chev">{expanded ? '▼' : '▶'}</span>
      Details
    </button>

    {#if expanded}
      <div class="details">
        <ArtifactLinker {session} />

        <ToolTimeline sessionId={session.id} />

        <LoadedSnapshot snapshot={session.loaded_snapshot} />

        <section class="sub">
          <h4>Event timeline</h4>
          <ol class="timeline">
            {#each events as e (e.id)}
              <li>
                <span class="kind">{e.kind}</span>
                <span class="ts">{new Date(e.ts * 1000).toLocaleTimeString()}</span>
              </li>
            {:else}
              <li class="empty">No events yet.</li>
            {/each}
          </ol>
        </section>

        <section class="sub">
          <h4>Artifacts</h4>
          <ul class="artifacts">
            {#each artifacts as a (a.id)}
              <li>
                <button onclick={() => void openPath(a.path)}>
                  {a.label ?? a.path}
                </button>
              </li>
            {:else}
              <li class="empty">No artifacts yet.</li>
            {/each}
          </ul>
        </section>
      </div>
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
  .row.needs-input { border-left: 3px solid #f39c12; }
  header { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  h3 { margin: 0; font-size: 1rem; }
  h4 { margin: 0 0 0.25rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted); }
  .dot { width: 10px; height: 10px; border-radius: 50%; }
  .label { flex: 1 1 auto; }
  .cwd { color: var(--text-muted); font-size: 0.8125rem; font-family: var(--font-mono, monospace); }
  .pill { background: var(--bg-muted); padding: 0.125rem 0.5rem; border-radius: 999px; font-size: 0.75rem; display: inline-flex; gap: 0.25rem; }
  .pill.muted { color: var(--text-muted); }
  .pill .kind { font-weight: 600; }
  section { margin-top: 0.75rem; }
  .prompt { background: var(--bg-muted); padding: 0.5rem; border-radius: var(--radius-sm); margin: 0; font-size: 0.875rem; }
  .turn { background: var(--bg-muted); padding: 0.5rem; border-radius: var(--radius-sm); margin: 0.25rem 0 0; font-size: 0.8125rem; white-space: pre-wrap; word-wrap: break-word; max-height: 200px; overflow: auto; }
  .expand { background: none; border: none; padding: 0; cursor: pointer; font: inherit; color: var(--text-secondary); }
  .chev { display: inline-block; width: 1em; }
  .details { margin-top: 0.5rem; padding-top: 0.5rem; border-top: 1px dashed var(--border); }
  .sub { margin-top: 0.5rem; }
  .timeline, .artifacts { list-style: none; padding: 0; margin: 0; font-size: 0.8125rem; }
  .timeline li { display: flex; justify-content: space-between; padding: 0.125rem 0; border-bottom: 1px solid var(--border); }
  .artifacts button { background: none; border: 1px solid var(--border); padding: 0.25rem 0.5rem; border-radius: var(--radius-sm); cursor: pointer; text-align: left; width: 100%; }
  .kind { font-weight: 600; }
  .ts { color: var(--text-muted); }
  .empty { color: var(--text-muted); font-style: italic; padding: 0.25rem 0; }
  .composer { margin-top: 1rem; padding-top: 0.75rem; border-top: 1px solid var(--border); }
</style>
