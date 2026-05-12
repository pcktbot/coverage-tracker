<script lang="ts">
  import { sendInbox } from '$lib/orchestrator';

  let { sessionId }: { sessionId: string } = $props();
  let text = $state('');
  let sending = $state(false);
  let justSent = $state(false);

  async function submit() {
    if (!text.trim()) return;
    sending = true;
    try {
      await sendInbox(sessionId, text);
      text = '';
      justSent = true;
      setTimeout(() => { justSent = false; }, 2500);
    } finally {
      sending = false;
    }
  }
</script>

<form onsubmit={(e) => { e.preventDefault(); void submit(); }}>
  <textarea bind:value={text} rows="3" placeholder="Type reply — sent next time this session asks for input"></textarea>
  <button type="submit" disabled={sending || !text.trim()}>Send</button>
  {#if justSent}
    <div class="toast">Queued. Type anything in the session's terminal to deliver.</div>
  {/if}
</form>

<style>
  form { display: flex; flex-direction: column; gap: 0.5rem; }
  textarea { font-family: var(--font); padding: 0.5rem; border: 1px solid var(--border); border-radius: var(--radius-sm); resize: vertical; }
  button { padding: 0.4rem 0.8rem; align-self: flex-end; }
  .toast {
    align-self: flex-end;
    background: #ecfdf3;
    border: 1px solid #b7e4c7;
    color: #157347;
    padding: 0.25rem 0.5rem;
    border-radius: var(--radius-sm);
    font-size: 0.75rem;
    animation: fade 2.5s ease-out forwards;
  }
  @keyframes fade {
    0% { opacity: 0; }
    10% { opacity: 1; }
    80% { opacity: 1; }
    100% { opacity: 0; }
  }
</style>
