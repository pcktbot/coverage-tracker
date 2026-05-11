<script lang="ts">
  import { tick } from 'svelte';
  import { sendAIMessage, type AIChatMessage, type AgentProfile } from '$lib/api';

  interface Props {
    open?: boolean;
    title?: string;
    contextTitle?: string;
    contextLines?: string[];
    agentProfiles?: AgentProfile[];
    mcpEnabled?: boolean;
    anthropicModel?: string;
    systemPrompt?: string;
    tokenConfigured?: boolean;
  }

  let {
    open = true,
    title = 'Claude Workspace',
    contextTitle = 'Current view',
    contextLines = [],
    agentProfiles = [],
    mcpEnabled = true,
    anthropicModel = '',
    systemPrompt = '',
    tokenConfigured = false,
  }: Props = $props();

  let messages = $state<AIChatMessage[]>([]);
  let draft = $state('');
  let sending = $state(false);
  let error = $state('');
  let scroller = $state<HTMLDivElement | null>(null);
  let hydratedRouteKey = $state('');
  let selectedProfileId = $state<number | null>(null);
  const routeKey = $derived(contextLines.join('|'));
  const selectedProfile = $derived(
    agentProfiles.find((profile) => profile.id === selectedProfileId)
      ?? agentProfiles.find((profile) => profile.is_active)
      ?? agentProfiles[0]
      ?? null,
  );

  $effect(() => {
    if (!routeKey || routeKey === hydratedRouteKey) return;
    hydratedRouteKey = routeKey;
    messages = [];
    draft = '';
    error = '';
  });

  $effect(() => {
    if (selectedProfileId != null || agentProfiles.length === 0) return;
    selectedProfileId = (agentProfiles.find((profile) => profile.is_active) ?? agentProfiles[0]).id;
  });

  function usePrompt(seed: string) {
    draft = seed;
  }

  function clearSession() {
    messages = [];
    draft = '';
    error = '';
  }

  async function scrollToBottom() {
    await tick();
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: 'smooth' });
  }

  async function submit() {
    if (!draft.trim() || sending) return;

    const userMessage: AIChatMessage = { role: 'user', content: draft.trim() };
    const history = [...messages, userMessage];
    messages = history;
    draft = '';
    sending = true;
    error = '';
    await scrollToBottom();

    try {
      const response = await sendAIMessage(history, contextLines);
      messages = [...history, { role: 'assistant', content: response }];
      await scrollToBottom();
    } catch (e: any) {
      messages = history;
      error = e.message ?? String(e);
    } finally {
      sending = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
      event.preventDefault();
      void submit();
    }
  }
</script>

{#if open}
  <aside class="ai-aside">
    <div class="aside-header">
      <div>
        <h2>{title}</h2>
        <p class="aside-subtitle">
          {tokenConfigured
            ? `Anthropic API${anthropicModel ? ` · ${anthropicModel}` : ''}`
            : 'Anthropic API key not configured'}
        </p>
        <p class="aside-meta">
          MCP access {mcpEnabled ? 'enabled' : 'disabled'}
        </p>
      </div>
    </div>

    <section class="aside-section">
      <details class="context-accordion">
        <summary>
          <h3>{contextTitle}</h3>
        </summary>
        <div class="context-list">
          {#each contextLines as line}
            <span class="context-chip">{line}</span>
          {/each}
        </div>
      </details>
    </section>

    <section class="aside-section session-meta">
      <div>
        <h3>Session</h3>
        <p class="aside-meta">Local only. Clears on app reload.</p>
      </div>
      <button class="btn-ghost" onclick={clearSession} disabled={messages.length === 0 && !draft}>Clear</button>
    </section>

    <section class="aside-section grow">
      <div class="prompt-header">
        <h3>Chat</h3>
        {#if systemPrompt}
          <span class="badge badge-gray">system prompt active</span>
        {/if}
      </div>

      <div class="chat-log" bind:this={scroller}>
        {#if messages.length === 0}
          <div class="empty-state">
            <p>No messages yet.</p>
            <p class="aside-meta">The current app view is included with each request.</p>
          </div>
        {:else}
          {#each messages as message}
            <div class:message-row={true} class:user={message.role === 'user'} class:assistant={message.role === 'assistant'}>
              <div class="message-role">{message.role === 'user' ? 'You' : 'Claude'}</div>
              <div class="message-body">{message.content}</div>
            </div>
          {/each}
        {/if}
      </div>

      {#if error}
        <div class="error-msg" style="margin-top:0.75rem">{error}</div>
      {/if}
    </section>

    <div class="aside-actions">
      <textarea
        class="prompt-editor"
        bind:value={draft}
        spellcheck="false"
        rows="4"
        onkeydown={handleKeydown}
        placeholder={tokenConfigured
          ? 'Ask Claude to summarize, prioritize, compare, or identify risks from this view. Cmd/Ctrl+Enter to send.'
          : 'Add an Anthropic API key in Settings to use the embedded assistant.'}
        disabled={!tokenConfigured || sending}
      ></textarea>
      <div class="action-row">
        <div class="action-group">
          <select class="profile-select" bind:value={selectedProfileId}>
            {#each agentProfiles as profile}
              <option value={profile.id}>{profile.name}</option>
            {/each}
          </select>
          <button class="btn-secondary" onclick={() => usePrompt(selectedProfile ? `${selectedProfile.goal || 'Help me reason about this view.'}\n\n${selectedProfile.instructions || 'What matters most right now? What is unclear? What should I check next?'}` : `Help me reason about this view.\n\nWhat matters most right now? What is unclear? What should I check next?`)} disabled={!tokenConfigured || sending}>
          Seed
          </button>
        </div>
        <button class="btn-primary" onclick={submit} disabled={!tokenConfigured || sending || !draft.trim()}>
          {sending ? 'Sending…' : 'Send'}
        </button>
      </div>
    </div>
  </aside>
{/if}

<style>
  .ai-aside {
    flex: 0 0 440px;
    width: 440px;
    max-width: 440px;
    min-width: 440px;
    border-left: 1px solid var(--border);
    background: linear-gradient(180deg, #ffffff 0%, #fbfcff 100%);
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .aside-header,
  .aside-section,
  .aside-actions {
    padding: 1rem;
  }

  .aside-header {
    border-bottom: 1px solid var(--border);
  }

  .aside-subtitle {
    margin: 0.35rem 0 0;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .aside-meta {
    margin: 0.25rem 0 0;
    color: var(--text-muted);
    line-height: 1.4;
    font-size: 0.75rem;
  }

  .aside-section {
    border-bottom: 1px solid var(--border-subtle);
  }

  .session-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
  }

  .aside-section.grow {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .context-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 0.65rem;
  }

  .context-accordion summary {
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: pointer;
    list-style: none;
  }

  .context-accordion summary::-webkit-details-marker {
    display: none;
  }

  .context-accordion summary::after {
    content: '+';
    font-family: var(--font-mono);
    font-size: 0.9rem;
    color: var(--text-secondary);
  }

  .context-accordion[open] summary::after {
    content: '−';
  }

  .context-chip {
    display: inline-flex;
    align-items: center;
    font-size: 0.75rem;
    padding: 0.25rem 0.55rem;
    border-radius: 999px;
    background: var(--bg-subtle);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }

  .prompt-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    margin-bottom: 0.65rem;
  }

  .chat-log {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    gap: 0.75rem;
    padding-right: 0.25rem;
  }

  .empty-state {
    padding: 1rem;
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.7);
  }

  .empty-state p {
    margin: 0;
  }

  .message-row {
    display: grid;
    gap: 0.35rem;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: #fff;
  }

  .message-row.user {
    background: #f8fbff;
    border-color: #cfe0ff;
  }

  .message-row.assistant {
    background: #fbfbfc;
  }

  .message-role {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .message-body {
    white-space: pre-wrap;
    line-height: 1.55;
    color: var(--text);
  }

  .aside-actions {
    display: grid;
    gap: 0.75rem;
    border-top: 1px solid var(--border);
  }

  .prompt-editor {
    width: 100%;
    resize: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.85rem;
    font-family: var(--font);
    font-size: 0.875rem;
    line-height: 1.55;
    background: #fff;
    color: var(--text);
  }

  .action-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }

  .action-group {
    display: flex;
    gap: 0.5rem;
    min-width: 0;
    flex: 1;
  }

  .profile-select {
    min-width: 0;
    max-width: 100%;
    flex: 1;
  }

  @media (max-width: 1200px) {
    .ai-aside {
      flex-basis: 380px;
      width: 380px;
      max-width: 380px;
      min-width: 380px;
    }
  }

  @media (max-width: 960px) {
    .ai-aside {
      display: none;
    }
  }
</style>
