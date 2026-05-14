<script lang="ts">
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

  // Title prefers first_user_prompt, falls back to label, falls back to id.
  // Truncate to one visible line; full text revealed on click.
  const titleFull = $derived(
    session.first_user_prompt ?? session.label ?? session.id
  );
  const titleShort = $derived(
    titleFull.length > 100 ? titleFull.slice(0, 100) + '…' : titleFull
  );
  const titleClickable = $derived(titleFull.length > 100);

  const dotColor = $derived({
    working: '#3498db',
    idle: '#95a5a6',
    needs_input: '#f39c12',
    done: '#2ecc71',
    error: '#e74c3c',
    unknown: '#7f8c8d',
  }[session.status]);
</script>

<article class="row"
  class:needs-input={session.status === 'needs_input'}
  class:dismissed={session.dismissed_at != null}
>
  <header>
    <span class="dot" style="background: {dotColor}" aria-label={session.status}></span>

    {#if titleClickable}
      <button
        class="title-btn"
        onclick={() => (promptOpen = !promptOpen)}
        title={promptOpen ? 'Collapse' : 'Show full prompt'}
        type="button"
      >
        <span class="chev-inline">{promptOpen ? '▼' : '▶'}</span>
        {#if promptOpen}
          <span class="title-full">{titleFull}</span>
        {:else}
          <span class="title-short">{titleShort}</span>
        {/if}
      </button>
    {:else}
      <span class="title-static">{titleFull}</span>
    {/if}

    {#if session.artifact_kind}
      <span class="pill" title={session.artifact_url ?? ''}>
        <span class="kind">{kindLabel}</span>
        <span class="title">{session.artifact_title ?? session.artifact_id}</span>
      </span>
    {:else}
      <span class="pill muted">Unlinked</span>
    {/if}

    <span class="cwd" title={session.cwd}>{cwdShort}</span>

    {#if session.dismissed_at != null}
      <button class="dismiss-btn" onclick={undismiss} type="button" title="Restore">↩ restore</button>
    {:else}
      <button class="dismiss-btn" onclick={dismiss} type="button" title="Dismiss">✕</button>
    {/if}
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
  .row.dismissed { opacity: 0.55; }

  header { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  h4 { margin: 0 0 0.25rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted); }
  .dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }

  .title-btn {
    flex: 1 1 auto; min-width: 0;
    display: inline-flex; align-items: center; gap: 0.25rem;
    background: none; border: none; padding: 0;
    text-align: left; font: inherit; color: inherit;
    cursor: pointer;
  }
  .title-static { flex: 1 1 auto; min-width: 0; font-size: 1rem; font-weight: 600; }
  .title-short {
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    font-size: 1rem; font-weight: 600;
  }
  .title-full { font-size: 1rem; font-weight: 600; white-space: pre-wrap; }
  .chev-inline { color: var(--text-muted); font-size: 0.75rem; }

  .cwd { color: var(--text-muted); font-size: 0.8125rem; font-family: var(--font-mono, monospace); }
  .pill { background: var(--bg-muted); padding: 0.125rem 0.5rem; border-radius: 999px; font-size: 0.75rem; display: inline-flex; gap: 0.25rem; }
  .pill.muted { color: var(--text-muted); }
  .pill .kind { font-weight: 600; }
  .pill .title { font-size: 0.75rem; font-weight: 400; }

  .dismiss-btn {
    background: none; border: 1px solid var(--border); color: var(--text-muted);
    padding: 0.0625rem 0.4rem; border-radius: var(--radius-sm);
    cursor: pointer; font: inherit; font-size: 0.75rem;
  }
  .dismiss-btn:hover { color: var(--text); border-color: var(--text-muted); }

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
