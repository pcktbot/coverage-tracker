<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { orgs, activeOrg, refreshOrgs, refreshRepos } from '$lib/stores/repos';
  import { setActiveOrg } from '$lib/api';

  onMount(async () => {
    await refreshOrgs();
    const org = $activeOrg;
    if (org) await refreshRepos(org);
  });

  async function switchOrg(name: string) {
    await setActiveOrg(name);
    activeOrg.set(name);
    await refreshRepos(name);
  }

  const navItems = [
    { href: '/', label: 'Dashboard' },
    { href: '/settings', label: 'Settings' },
  ];
</script>

<div class="layout">
  <nav class="sidebar">
    <div class="sidebar-header">
      <span class="app-name">Coverage</span>
    </div>

    <!-- Org switcher -->
    {#if $orgs.length > 0}
      <div class="org-section">
        <span class="section-label">Org</span>
        {#each $orgs as org}
          <button
            class="org-btn"
            class:active={org.name === $activeOrg}
            on:click={() => switchOrg(org.name)}
          >
            {org.name}
          </button>
        {/each}
      </div>
    {/if}

    <ul class="nav-list">
      {#each navItems as item}
        <li>
          <a
            href={item.href}
            class="nav-link"
            class:active={$page.url.pathname === item.href}
          >{item.label}</a>
        </li>
      {/each}
    </ul>
  </nav>

  <main class="main-content">
    <slot />
  </main>
</div>

<style>
  .layout {
    display: flex;
    height: 100vh;
    width: 100%;
    overflow: hidden;
  }

  .sidebar {
    width: var(--nav-width);
    min-width: var(--nav-width);
    background: var(--bg-subtle);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 0;
  }

  .sidebar-header {
    padding: 1rem;
    border-bottom: 1px solid var(--border);
  }

  .app-name {
    font-size: 0.9375rem;
    font-weight: 700;
    color: var(--text);
    letter-spacing: -0.01em;
  }

  .org-section {
    padding: 0.75rem 0.5rem 0.5rem;
    border-bottom: 1px solid var(--border-subtle);
  }

  .section-label {
    display: block;
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    padding: 0 0.5rem 0.375rem;
  }

  .org-btn {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.3125rem 0.625rem;
    font-size: 0.8125rem;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .org-btn:hover { background: var(--bg-muted); color: var(--text); }
  .org-btn.active { background: var(--accent-subtle); color: var(--accent); font-weight: 500; }

  .nav-list {
    list-style: none;
    margin: 0;
    padding: 0.5rem;
  }

  .nav-link {
    display: block;
    padding: 0.375rem 0.625rem;
    border-radius: var(--radius-sm);
    font-size: 0.8125rem;
    color: var(--text-secondary);
    text-decoration: none;
  }
  .nav-link:hover { background: var(--bg-muted); color: var(--text); text-decoration: none; }
  .nav-link.active { background: var(--accent-subtle); color: var(--accent); font-weight: 500; }

  .main-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
  }
</style>
