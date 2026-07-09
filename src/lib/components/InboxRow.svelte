<script lang="ts">
  import { onMount } from 'svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import {
    getTranscriptTail, listEvents, listArtifacts,
    dismissSession, undismissSession,
    type Session, type TranscriptTurn, type OrchestratorEvent, type Artifact,
  } from '$lib/orchestrator';
  import InboxComposer from './InboxComposer.svelte';
  import ArtifactLinker from './ArtifactLinker.svelte';
  import ToolTimeline from './ToolTimeline.svelte';
  import LoadedSnapshot from './LoadedSnapshot.svelte';

  let { session, onChange }: { session: Session; onChange?: () => void } = $props();

  let lastAssistantTurn = $state<TranscriptTurn | null>(null);
  let events = $state<OrchestratorEvent[]>([]);
  let artifacts = $state<Artifact[]>([]);
  let expanded = $state(false);
  let promptOpen = $state(false);
  let loading = $state(false);

  // For needs_input sessions, eagerly fetch the latest assistant turn so the
  // user has the question/blocker visible right above the composer.
  onMount(async () => {
    if (session.status !== 'needs_input') return;
    try {
      const turns = await getTranscriptTail(session.id, 4);
      for (let i = turns.length - 1; i >= 0; i--) {
        if (turns[i].role === 'assistant') { lastAssistantTurn = turns[i]; break; }
      }
    } catch { /* best-effort */ }
  });

  async function toggle() {
    expanded = !expanded;
    if (expanded && !loading && events.length === 0) {
      loading = true;
      try {
        const [evs, arts, turns] = await Promise.all([
          listEvents(session.id),
          listArtifacts(session.id),
          getTranscriptTail(session.id, 4),
        ]);
        events = evs;
        artifacts = arts;
        for (let i = turns.length - 1; i >= 0; i--) {
          if (turns[i].role === 'assistant') { lastAssistantTurn = turns[i]; break; }
        }
      } finally {
        loading = false;
      }
    }
  }

  async function dismiss() {
    try { await dismissSession(session.id); onChange?.(); } catch (e) { console.error(e); }
  }
  async function undismiss() {
    try { await undismissSession(session.id); onChange?.(); } catch (e) { console.error(e); }
  }

  const kindLabel = $derived({
    project: 'Project', ado: 'ADO', confluence: 'Confluence', github_pr: 'GitHub PR'
  }[session.artifact_kind ?? 'project']);

  const cwdShort = $derived(
    session.cwd.length > 40 ? '…' + session.cwd.slice(-39) : session.cwd
  );

  const initialPrompt = $derived(session.first_user_prompt);

  const dotColor = $derived({
    working: '#3498db',
    idle: '#95a5a6',
    needs_input: '#f39c12',
    done: '#2ecc71',
    error: '#e74c3c',
    unknown: '#7f8c8d',
  }[session.status]);

  const statusLabel = $derived({
    working: 'working',
    idle: 'idle',
    needs_input: 'needs input',
    done: 'done',
    error: 'error',
    unknown: 'unknown',
  }[session.status]);
</script>

<article class="row"
  class:needs-input={session.status === 'needs_input'}
  class:dismissed={session.dismissed_at != null}
>
  <header>
    <span class="status-pill" title={session.status}>
      <span class="dot" style="background: {dotColor}"></span>
      <span class="status-label">{statusLabel}</span>
    </span>

    {#if session.artifact_kind}
      <span class="pill" title={session.artifact_url ?? ''}>
        <span class="kind">{kindLabel}</span>
        <span class="artifact-title">{session.artifact_title ?? session.artifact_id}</span>
      </span>
    {:else}
      <span class="pill muted">Unlinked</span>
    {/if}

    <span class="cwd" title={session.cwd}>{cwdShort}</span>

    {#if session.dismissed_at != null}
      <button class="dismiss-btn" onclick={undismiss} type="button" title="Restore">↩</button>
    {:else}
      <button class="dismiss-btn" onclick={dismiss} type="button" title="Dismiss">✕</button>
    {/if}
  </header>

  {#if initialPrompt}
    <section>
      <h4>Initial prompt</h4>
      <button
        type="button"
        class="initial-prompt-wrap"
        class:expanded={promptOpen}
        onclick={() => (promptOpen = !promptOpen)}
        title={promptOpen ? 'Collapse' : 'Expand'}
      >
        <pre class="initial-prompt">{initialPrompt}</pre>
      </button>
    </section>
  {/if}

  {#if session.last_user_prompt}
    <section>
      <h4>Last user prompt</h4>
      <p class="prompt">{session.last_user_prompt}</p>
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

  {#if lastAssistantTurn}
    <section>
      <h4>Latest assistant response</h4>
      <pre class="turn">{lastAssistantTurn.text}</pre>
    </section>
  {/if}

  <section class="composer">
    <InboxComposer sessionId={session.id} />
  </section>
</article>

<style>
  .row {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1rem;
    background: var(--bg);
  }
  .row.needs-input { box-shadow: inset 0 -3px 0 #f39c12; padding-bottom: calc(1rem + 3px); }
  .row.dismissed { opacity: 0.55; }

  header { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  h4 { margin: 0 0 0.25rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted); }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }

  .status-pill {
    display: inline-flex; align-items: center; gap: 0.3rem;
    background: var(--bg-muted); padding: 0.125rem 0.5rem;
    border-radius: 999px; font-size: 0.75rem; font-weight: 600;
  }
  .status-label { text-transform: lowercase; }

  .pill { background: var(--bg-muted); padding: 0.125rem 0.5rem; border-radius: 999px; font-size: 0.75rem; display: inline-flex; gap: 0.25rem; max-width: 100%; min-width: 0; }
  .pill.muted { color: var(--text-muted); }
  .pill .kind { font-weight: 600; }
  .pill .artifact-title { font-weight: 400; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }

  .cwd {
    color: var(--text-muted); font-size: 0.75rem;
    font-family: var(--font-mono, monospace);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    flex: 1 1 auto; min-width: 0;
  }

  .dismiss-btn {
    background: none; border: 1px solid var(--border); color: var(--text-muted);
    padding: 0.0625rem 0.4rem; border-radius: var(--radius-sm);
    cursor: pointer; font: inherit; font-size: 0.75rem;
    flex-shrink: 0;
  }
  .dismiss-btn:hover { color: var(--text); border-color: var(--text-muted); }

  .initial-prompt-wrap {
    display: block; width: 100%;
    background: none; border: none; padding: 0; margin: 0;
    text-align: left; font: inherit; color: inherit;
    cursor: pointer;
  }
  .initial-prompt {
    margin: 0; padding: 0.5rem 0.6rem;
    background: var(--bg-muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.8125rem; line-height: 1.4;
    white-space: pre-wrap; word-break: break-word; overflow-wrap: anywhere;
    max-height: 5.2em; overflow: hidden;
    position: relative;
  }
  .initial-prompt-wrap:not(.expanded) .initial-prompt::after {
    content: ''; position: absolute; left: 0; right: 0; bottom: 0; height: 1.5em;
    background: linear-gradient(transparent, var(--bg-muted));
    pointer-events: none;
  }
  .initial-prompt-wrap.expanded .initial-prompt { max-height: none; }

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
