<script lang="ts">
  import { sendInbox } from '$lib/orchestrator';

  let { sessionId }: { sessionId: string } = $props();
  let text = $state('');
  let sending = $state(false);

  async function submit() {
    if (!text.trim()) return;
    sending = true;
    try {
      await sendInbox(sessionId, text);
      text = '';
    } finally {
      sending = false;
    }
  }
</script>

<form onsubmit={(e) => { e.preventDefault(); void submit(); }}>
  <textarea bind:value={text} rows="3" placeholder="Message this session"></textarea>
  <button type="submit" disabled={sending || !text.trim()}>Send</button>
</form>

<style>
  form { display: flex; flex-direction: column; gap: 0.5rem; }
  textarea { font-family: var(--font); padding: 0.5rem; border: 1px solid var(--border); border-radius: var(--radius-sm); resize: vertical; }
  button { padding: 0.4rem 0.8rem; align-self: flex-end; }
</style>
