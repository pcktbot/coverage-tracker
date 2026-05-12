<script lang="ts">
  import { onMount } from 'svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { listEvents, listArtifacts, type Session, type OrchestratorEvent, type Artifact } from '$lib/orchestrator';
  import InboxComposer from './InboxComposer.svelte';
  import ArtifactLinker from './ArtifactLinker.svelte';

  let { session, onclose }: { session: Session; onclose: () => void } = $props();
  let events = $state<OrchestratorEvent[]>([]);
  let artifacts = $state<Artifact[]>([]);

  onMount(async () => {
    events = await listEvents(session.id);
    artifacts = await listArtifacts(session.id);
  });
</script>

<aside class="drawer">
  <header>
    <h2>{session.label ?? session.id}</h2>
    <button class="close" onclick={onclose} aria-label="Close">×</button>
  </header>

  <ArtifactLinker {session} />

  <section>
    <h3>Timeline</h3>
    <ol class="timeline">
      {#each events as e (e.id)}
        <li>
          <span class="kind">{e.kind}</span>
          <span class="ts">{new Date(e.ts * 1000).toLocaleTimeString()}</span>
        </li>
      {/each}
      {#if events.length === 0}
        <li class="empty">No events yet.</li>
      {/if}
    </ol>
  </section>

  <section>
    <h3>Artifacts</h3>
    <ul class="artifacts">
      {#each artifacts as a (a.id)}
        <li>
          <button onclick={() => void openPath(a.path)}>
            {a.label ?? a.path}
          </button>
        </li>
      {/each}
      {#if artifacts.length === 0}
        <li class="empty">No artifacts yet.</li>
      {/if}
    </ul>
  </section>

  <section>
    <h3>Message session</h3>
    <InboxComposer sessionId={session.id} />
  </section>
</aside>

<style>
  .drawer {
    position: fixed;
    right: 0; top: 0; bottom: 0;
    width: 420px;
    background: var(--bg);
    border-left: 1px solid var(--border);
    padding: 1rem;
    overflow-y: auto;
    z-index: 10;
  }
  header { display: flex; justify-content: space-between; align-items: baseline; }
  .close { background: none; border: none; font-size: 1.5rem; cursor: pointer; color: var(--text-secondary); }
  section { margin-top: 1rem; }
  h3 { font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted); margin: 0 0 0.5rem; }
  .timeline, .artifacts { list-style: none; padding: 0; margin: 0; }
  .timeline li { padding: 0.25rem 0; border-bottom: 1px solid var(--border); display: flex; justify-content: space-between; }
  .artifacts button { background: none; border: 1px solid var(--border); padding: 0.25rem 0.5rem; border-radius: var(--radius-sm); cursor: pointer; text-align: left; width: 100%; }
  .kind { font-weight: 600; }
  .ts { color: var(--text-muted); font-size: 0.8125rem; }
  .empty { color: var(--text-muted); font-style: italic; padding: 0.25rem 0; }
</style>
