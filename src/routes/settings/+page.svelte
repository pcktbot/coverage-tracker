<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { orgs, activeOrg, repos, refreshOrgs, refreshRepos } from '$lib/stores/repos';
  import {
    getSettings, saveSettings, addOrg, removeOrg, setActiveOrg,
    syncOrgRepos, setRepoEnabled,
    type Settings,
  } from '$lib/api';

  let settings = $state<Settings>({ github_token: '', clone_root: '' });
  let saved = $state(false);
  let saving = $state(false);
  let error = $state('');
  let newOrg = $state('');
  let addingOrg = $state(false);
  let orgError = $state('');

  // Repo management
  let repoFilter = $state('');
  let syncing = $state(false);
  let syncProgress = $state<{ done: number; total: number; name: string } | null>(null);
  let syncError = $state('');
  let togglingId = $state<number | null>(null);

  let filteredRepos = $derived(
    $repos.filter((r) => r.name.toLowerCase().includes(repoFilter.toLowerCase()))
  );

  let unlistenSync: (() => void) | null = null;

  onMount(() => {
    (async () => {
      try {
        settings = await getSettings();
        await refreshOrgs();
        if ($activeOrg) await refreshRepos($activeOrg);
      } catch (e: any) {
        error = e.message;
      }

      // Listen for sync progress events
      unlistenSync = await listen<{ done: number; total: number; name: string }>(
        'sync-progress',
        (e) => { syncProgress = e.payload; }
      );
    })();
    return () => { unlistenSync?.(); };
  });

  async function save() {
    saving = true; error = ''; saved = false;
    try {
      await saveSettings(settings);
      saved = true;
      setTimeout(() => (saved = false), 2000);
    } catch (e: any) {
      error = e.message;
    } finally {
      saving = false;
    }
  }

  async function doAddOrg() {
    if (!newOrg.trim()) return;
    addingOrg = true; orgError = '';
    try {
      await addOrg(newOrg.trim());
      await refreshOrgs();
      newOrg = '';
    } catch (e: any) {
      orgError = e.message;
    } finally {
      addingOrg = false;
    }
  }

  async function doRemoveOrg(name: string) {
    try { await removeOrg(name); await refreshOrgs(); }
    catch (e: any) { orgError = e.message; }
  }

  async function doActivateOrg(name: string) {
    try {
      await setActiveOrg(name);
      activeOrg.set(name);
      await refreshOrgs();
      await refreshRepos(name);
    } catch (e: any) { orgError = e.message; }
  }

  async function doSync() {
    if (!$activeOrg) return;
    syncing = true; syncProgress = null; syncError = '';
    try {
      const count = await syncOrgRepos($activeOrg);
      await refreshRepos($activeOrg);
      syncProgress = { done: count, total: count, name: '' };
    } catch (e: any) {
      syncError = e.message;
    } finally {
      syncing = false;
    }
  }

  async function toggleRepo(id: number, enabled: boolean) {
    togglingId = id;
    try {
      await setRepoEnabled(id, enabled);
      repos.update((list) => list.map((r) => r.id === id ? { ...r, enabled } : r));
    } catch (e: any) {
      error = e.message;
    } finally {
      togglingId = null;
    }
  }

  function enableAll() {
    filteredRepos.forEach((r) => { if (!r.enabled) toggleRepo(r.id, true); });
  }
  function disableAll() {
    filteredRepos.forEach((r) => { if (r.enabled) toggleRepo(r.id, false); });
  }
</script>

<h1 style="margin-bottom:1.5rem">Settings</h1>

{#if error}
  <div class="error-msg" style="margin-bottom:1rem">{error}</div>
{/if}

<div class="settings-grid">
  <!-- GitHub token -->
  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">GitHub</h2>
    <div class="form-group">
      <label for="token">Personal Access Token</label>
      <input id="token" type="password" bind:value={settings.github_token}
        placeholder="ghp_…" autocomplete="off" />
      <p class="hint">Needs <code>repo</code> scope to read private repos and Gemfiles.</p>
    </div>
  </section>

  <!-- Clone path -->
  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">Local storage</h2>
    <div class="form-group">
      <label for="clone-root">Clone root directory</label>
      <input id="clone-root" type="text" bind:value={settings.clone_root}
        placeholder="/Users/you/repos" />
      <p class="hint">Repos will be cloned to <code>&lt;root&gt;/&lt;org&gt;/&lt;repo&gt;</code>.</p>
    </div>
  </section>

  <!-- GitHub orgs -->
  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">GitHub orgs</h2>
    {#if orgError}
      <div class="error-msg" style="margin-bottom:0.75rem">{orgError}</div>
    {/if}
    <ul class="org-list">
      {#each $orgs as org}
        <li class="org-item">
          <span class="org-name">{org.name}</span>
          {#if org.is_active}
            <span class="badge badge-green" style="font-size:0.6875rem">active</span>
          {:else}
            <button class="btn-ghost" style="font-size:0.75rem" onclick={() => doActivateOrg(org.name)}>
              Set active
            </button>
          {/if}
          <button class="btn-ghost btn-danger-ghost" onclick={() => doRemoveOrg(org.name)}>✕</button>
        </li>
      {/each}
    </ul>
    <div class="add-org-row">
      <input type="text" bind:value={newOrg} placeholder="github-org-name"
        onkeydown={(e) => e.key === 'Enter' && doAddOrg()} style="flex:1" />
      <button class="btn-secondary" onclick={doAddOrg} disabled={addingOrg || !newOrg.trim()}>
        {addingOrg ? 'Adding…' : 'Add org'}
      </button>
    </div>
  </section>
</div>

<div style="margin-top:1.25rem;display:flex;align-items:center;gap:0.75rem">
  <button class="btn-primary" onclick={save} disabled={saving}>
    {saving ? 'Saving…' : 'Save settings'}
  </button>
  {#if saved}<span class="badge badge-green">Saved!</span>{/if}
</div>

<!-- ── Repo management ───────────────────────────────────────────── -->
<div style="margin-top:2rem">
  <div class="repo-mgmt-header">
    <h2>Repos — {$activeOrg ?? '…'}</h2>
    <div style="display:flex;gap:0.5rem;align-items:center">
      <button class="btn-secondary" onclick={doSync} disabled={syncing || !$activeOrg}>
        {syncing ? 'Syncing…' : 'Sync from GitHub'}
      </button>
    </div>
  </div>

  {#if syncError}
    <div class="error-msg" style="margin-bottom:0.75rem">{syncError}</div>
  {/if}

  {#if syncing && syncProgress}
    <div class="sync-progress">
      <div class="sync-bar-wrap">
        <div class="sync-bar" style="width:{(syncProgress.done/syncProgress.total)*100}%"></div>
      </div>
      <span class="text-muted" style="font-size:0.75rem">
        {syncProgress.done}/{syncProgress.total} — {syncProgress.name}
      </span>
    </div>
  {/if}

  {#if $repos.length > 0}
    <div class="repo-controls">
      <input type="text" bind:value={repoFilter} placeholder="Filter repos…" style="max-width:240px" />
      <button class="btn-ghost" onclick={enableAll} style="font-size:0.8125rem">Enable all</button>
      <button class="btn-ghost" onclick={disableAll} style="font-size:0.8125rem">Disable all</button>
      <span class="text-muted" style="font-size:0.8125rem">
        {$repos.filter(r => r.enabled).length} / {$repos.length} enabled
      </span>
    </div>

    <div class="card" style="margin-top:0.5rem">
      <table>
        <thead>
          <tr>
            <th>Enabled</th>
            <th>Repository</th>
            <th>Runtime</th>
            <th>Local path</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredRepos as repo}
            <tr>
              <td style="width:56px;text-align:center">
                <label class="toggle" aria-label="Toggle {repo.name}">
                  <input
                    type="checkbox"
                    checked={repo.enabled}
                    disabled={togglingId === repo.id}
                    onchange={(e) => toggleRepo(repo.id, (e.target as HTMLInputElement).checked)}
                  />
                  <span class="slider"></span>
                </label>
              </td>
              <td style="font-weight:500">{repo.name}</td>
              <td class="mono text-muted" style="font-size:0.75rem">{repo.node_version ? `node ${repo.node_version}` : repo.ruby_version ? `ruby ${repo.ruby_version}` : '—'}</td>
              <td class="mono text-muted" style="font-size:0.6875rem;max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">
                {repo.local_path ?? '—'}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else if !syncing}
    <p class="text-muted" style="margin-top:0.75rem">
      No repos synced yet. Click <em>Sync from GitHub</em> to fetch the repo list.
    </p>
  {/if}
</div>

<style>
  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 1rem;
    align-items: start;
  }
  .hint { font-size: 0.75rem; color: var(--text-muted); margin: 0.25rem 0 0; }
  code { font-family: var(--font-mono); background: var(--bg-muted); padding: 0.1em 0.3em; border-radius: 3px; }
  .org-list { list-style: none; margin: 0 0 0.75rem; padding: 0; display: flex; flex-direction: column; gap: 0.25rem; }
  .org-item { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 0; }
  .org-name { flex: 1; font-size: 0.875rem; }
  .add-org-row { display: flex; gap: 0.5rem; }
  .btn-danger-ghost { color: var(--text-muted); font-size: 0.75rem; }
  .btn-danger-ghost:hover { color: var(--danger); }

  .repo-mgmt-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; }
  .repo-controls { display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; margin-bottom: 0.25rem; }

  .sync-progress { display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.75rem; }
  .sync-bar-wrap { flex: 1; max-width: 260px; height: 6px; background: var(--bg-muted); border-radius: 3px; overflow: hidden; }
  .sync-bar { height: 100%; background: var(--accent); border-radius: 3px; transition: width 0.2s; }

  /* Toggle switch */
  .toggle { position: relative; display: inline-flex; align-items: center; cursor: pointer; }
  .toggle input { opacity: 0; width: 0; height: 0; position: absolute; }
  .slider {
    display: inline-block; width: 32px; height: 18px;
    background: var(--border); border-radius: 9px;
    transition: background 0.2s;
    position: relative;
  }
  .slider::after {
    content: ''; position: absolute;
    top: 3px; left: 3px;
    width: 12px; height: 12px;
    background: white; border-radius: 50%;
    transition: transform 0.2s;
  }
  .toggle input:checked + .slider { background: var(--success); }
  .toggle input:checked + .slider::after { transform: translateX(14px); }
  .toggle input:disabled + .slider { opacity: 0.5; cursor: not-allowed; }
</style>
