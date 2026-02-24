<script lang="ts">
  import { onMount } from 'svelte';
  import { orgs, activeOrg, refreshOrgs } from '$lib/stores/repos';
  import { getSettings, saveSettings, addOrg, removeOrg, setActiveOrg, type Settings } from '$lib/api';

  let settings = $state<Settings>({ github_token: '', clone_root: '' });
  let saved = $state(false);
  let saving = $state(false);
  let error = $state('');
  let newOrg = $state('');
  let addingOrg = $state(false);
  let orgError = $state('');

  onMount(async () => {
    try {
      settings = await getSettings();
      await refreshOrgs();
    } catch (e: any) {
      error = e.message;
    }
  });

  async function save() {
    saving = true;
    error = '';
    saved = false;
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
    addingOrg = true;
    orgError = '';
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
    try {
      await removeOrg(name);
      await refreshOrgs();
    } catch (e: any) {
      orgError = e.message;
    }
  }

  async function doActivateOrg(name: string) {
    try {
      await setActiveOrg(name);
      activeOrg.set(name);
      await refreshOrgs();
    } catch (e: any) {
      orgError = e.message;
    }
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
      <input
        id="token"
        type="password"
        bind:value={settings.github_token}
        placeholder="ghp_…"
        autocomplete="off"
      />
      <p class="hint">Needs <code>repo</code> scope to read private repos and Gemfiles.</p>
    </div>
  </section>

  <!-- Clone path -->
  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">Local storage</h2>
    <div class="form-group">
      <label for="clone-root">Clone root directory</label>
      <input
        id="clone-root"
        type="text"
        bind:value={settings.clone_root}
        placeholder="/Users/you/repos"
      />
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
      <input
        type="text"
        bind:value={newOrg}
        placeholder="github-org-name"
        onkeydown={(e) => e.key === 'Enter' && doAddOrg()}
        style="flex:1"
      />
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
  {#if saved}
    <span class="badge badge-green">Saved!</span>
  {/if}
</div>

<style>
  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
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
</style>
