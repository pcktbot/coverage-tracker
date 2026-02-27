<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { orgs, activeOrg, refreshOrgs, refreshRepos } from '$lib/stores/repos';
  import { setActiveOrg } from '$lib/api';

  const FONT_SIZE_KEY = 'cm-font-size';
  const DEFAULT_FONT_SIZE = 14;
  const MIN_FONT_SIZE = 12;
  const MAX_FONT_SIZE = 24;
  const STEP = 2;

  let fontSize = DEFAULT_FONT_SIZE;

  function applyFontSize(size: number) {
    document.documentElement.style.fontSize = `${size}px`;
  }

  function increase() {
    if (fontSize < MAX_FONT_SIZE) {
      fontSize = Math.min(fontSize + STEP, MAX_FONT_SIZE);
      localStorage.setItem(FONT_SIZE_KEY, String(fontSize));
      applyFontSize(fontSize);
    }
  }

  function decrease() {
    if (fontSize > MIN_FONT_SIZE) {
      fontSize = Math.max(fontSize - STEP, MIN_FONT_SIZE);
      localStorage.setItem(FONT_SIZE_KEY, String(fontSize));
      applyFontSize(fontSize);
    }
  }

  onMount(async () => {
    const saved = localStorage.getItem(FONT_SIZE_KEY);
    if (saved) {
      fontSize = Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, Number(saved)));
      applyFontSize(fontSize);
    }
    console.log('[layout] onMount start');
    try {
      await refreshOrgs();
      console.log('[layout] refreshOrgs done, activeOrg=', $activeOrg);
      let org = $activeOrg;
      // Auto-select first org if none is active
      if (!org && $orgs.length > 0) {
        org = $orgs[0].name;
        await setActiveOrg(org);
        activeOrg.set(org);
      }
      if (org) {
        await refreshRepos(org);
        console.log('[layout] refreshRepos done');
      }
    } catch (e) {
      console.error('[layout] onMount error:', e);
    }
  });

  async function switchOrg(name: string) {
    await setActiveOrg(name);
    activeOrg.set(name);
    await refreshRepos(name);
    goto('/');
  }
</script>

<div class="layout">
  <nav class="toolbar">
    {#if $orgs.length > 0}
      <div class="nav-group">
        <span class="group-label">Dashboards</span>
        <ul class="nav-list">
          {#each $orgs as org}
            <li>
              <button
                class="nav-link"
                class:active={org.name === $activeOrg && $page.url.pathname === '/'}
                on:click={() => switchOrg(org.name)}
              >
                {org.name}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    <div class="font-size-controls">
      <button
        class="font-btn"
        on:click={decrease}
        disabled={fontSize <= MIN_FONT_SIZE}
        title="Decrease font size"
        aria-label="Decrease font size"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
      </button>
      <span class="font-size-label">{fontSize}px</span>
      <button
        class="font-btn"
        on:click={increase}
        disabled={fontSize >= MAX_FONT_SIZE}
        title="Increase font size"
        aria-label="Increase font size"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
      </button>
    </div>

    <a
      href="/settings"
      class="settings-link"
      class:active={$page.url.pathname === '/settings'}
      title="Settings"
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3"/>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
      </svg>
    </a>
  </nav>

  <main class="main-content">
    <slot />
  </main>
</div>

<style>
  .layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100%;
    overflow: hidden;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    background: var(--bg-subtle);
    border-bottom: 1px solid var(--border);
    padding: 0 1rem;
    height: 48px;
    min-height: 48px;
  }

  .nav-group {
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  .group-label {
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    margin-right: 0.125rem;
  }

  .nav-list {
    display: flex;
    align-items: center;
    list-style: none;
    margin: 0;
    padding: 0;
    gap: 0.25rem;
  }

  .nav-link {
    display: block;
    padding: 0.375rem 0.625rem;
    border-radius: var(--radius-sm);
    font-size: 0.8125rem;
    color: var(--text-secondary);
    text-decoration: none;
    white-space: nowrap;
    background: none;
    border: none;
    cursor: pointer;
    font-family: var(--font);
  }
  .nav-link:hover { background: var(--bg-muted); color: var(--text); text-decoration: none; }
  .nav-link.active { background: var(--accent-subtle); color: var(--accent); font-weight: 500; }

  .font-size-controls {
    display: flex;
    align-items: center;
    gap: 0;
    margin-left: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--bg);
  }

  .font-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    margin: 0;
    border: none;
    border-radius: 0;
    background: var(--bg);
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }
  .font-btn:hover:not(:disabled) { background: var(--bg-muted); color: var(--text); }
  .font-btn:disabled { opacity: 0.35; cursor: not-allowed; }

  .font-size-label {
    font-size: 0.6875rem;
    font-weight: 600;
    color: var(--text-secondary);
    min-width: 36px;
    text-align: center;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
    padding: 0 0.25rem;
    line-height: 28px;
    user-select: none;
  }

  .settings-link {
    display: flex;
    align-items: center;
    justify-content: center;
    margin-left: 0.5rem;
    padding: 0.375rem;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    text-decoration: none;
  }
  .settings-link:hover { background: var(--bg-muted); color: var(--text); text-decoration: none; }
  .settings-link.active { background: var(--accent-subtle); color: var(--accent); }

  .main-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
  }
</style>
