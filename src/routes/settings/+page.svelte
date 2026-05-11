<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { orgs, activeOrg, repos, refreshOrgs, refreshRepos } from '$lib/stores/repos';
  import {
    getSettings, saveSettings, addOrg, removeOrg, setActiveOrg,
    syncOrgRepos, setRepoEnabled, diagnoseGithubAuth,
    createAgentProfile, deleteAgentProfile, listAgentProfiles, saveAgentProfile,
    adoPreview,
    type Settings, type GithubAuthDiagnostics, type AgentProfile, type AdoPreview,
  } from '$lib/api';

  let settings = $state<Settings>({
    github_token: '',
    clone_root: '',
    tfs_base_url: '',
    tfs_pat: '',
    tfs_collection: '',
    tfs_default_project: 'Consumer Solutions',
    tfs_default_area_path: 'MKT-Websites',
    confluence_base_url: '',
    confluence_username: '',
    confluence_token: '',
    microsoft_tenant_id: '',
    microsoft_client_id: '',
    microsoft_client_secret: '',
    mcp_enabled: true,
    anthropic_api_key: '',
    anthropic_model: 'claude-sonnet-4-5',
    ai_system_prompt: 'Focus on prioritization, blockers, missing context, and next checks.',
  });
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
  let authChecking = $state(false);
  let authDiagnostics = $state<GithubAuthDiagnostics | null>(null);
  let authError = $state('');
  let agentProfiles = $state<AgentProfile[]>([]);
  let newAgentName = $state('');
  let agentError = $state('');
  let savingAgentId = $state<number | null>(null);
  let adoChecking = $state(false);
  let adoPreviewData = $state<AdoPreview | null>(null);
  let adoError = $state('');

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
        agentProfiles = await listAgentProfiles();
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

  async function runAuthCheck() {
    authChecking = true;
    authError = '';
    try {
      authDiagnostics = await diagnoseGithubAuth($activeOrg ?? undefined);
    } catch (e: any) {
      authError = e.message;
    } finally {
      authChecking = false;
    }
  }

  async function addAgentProfile() {
    if (!newAgentName.trim()) return;
    agentError = '';
    try {
      await createAgentProfile(newAgentName.trim());
      agentProfiles = await listAgentProfiles();
      newAgentName = '';
    } catch (e: any) {
      agentError = e.message ?? String(e);
    }
  }

  async function persistAgentProfile(profile: AgentProfile) {
    savingAgentId = profile.id;
    agentError = '';
    try {
      await saveAgentProfile(profile);
      agentProfiles = await listAgentProfiles();
    } catch (e: any) {
      agentError = e.message ?? String(e);
    } finally {
      savingAgentId = null;
    }
  }

  async function removeAgentProfile(profileId: number) {
    agentError = '';
    try {
      await deleteAgentProfile(profileId);
      agentProfiles = await listAgentProfiles();
    } catch (e: any) {
      agentError = e.message ?? String(e);
    }
  }

  async function runAdoPreview() {
    adoChecking = true;
    adoError = '';
    try {
      adoPreviewData = await adoPreview();
    } catch (e: any) {
      adoError = e.message ?? String(e);
    } finally {
      adoChecking = false;
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
      <input id="token" type="password" bind:value={settings.github_token}
        placeholder="ghp_…" autocomplete="off" />
      <p class="hint">Needs repo access for the org, and if the org enforces SSO the token must be explicitly authorized there.</p>
    </div>
    <div style="display:flex;align-items:center;gap:0.75rem;flex-wrap:wrap">
      <button class="btn-secondary" onclick={runAuthCheck} disabled={authChecking}>
        {authChecking ? 'Checking…' : 'Run auth check'}
      </button>
      {#if authDiagnostics?.org}
        <span class="text-muted" style="font-size:0.75rem">Target org: {authDiagnostics.org}</span>
      {/if}
    </div>
    {#if authError}
      <div class="error-msg" style="margin-top:0.75rem">{authError}</div>
    {/if}
    {#if authDiagnostics}
      <div class="auth-results">
        <div class="auth-row">
          <div>
            <strong>GitHub API</strong>
            <p class="auth-message">{authDiagnostics.api.message}</p>
            {#if authDiagnostics.api.hint}<p class="hint" style="margin-top:0.35rem">{authDiagnostics.api.hint}</p>{/if}
          </div>
          <span class="badge {authDiagnostics.api.ok ? 'badge-green' : authDiagnostics.api.status === 'skipped' ? 'badge-gray' : 'badge-red'}">
            {authDiagnostics.api.status}
          </span>
        </div>
        <div class="auth-row">
          <div>
            <strong>Git HTTPS</strong>
            <p class="auth-message">{authDiagnostics.git.message}</p>
            {#if authDiagnostics.git.hint}<p class="hint" style="margin-top:0.35rem">{authDiagnostics.git.hint}</p>{/if}
          </div>
          <span class="badge {authDiagnostics.git.ok ? 'badge-green' : authDiagnostics.git.status === 'skipped' ? 'badge-gray' : 'badge-red'}">
            {authDiagnostics.git.status}
          </span>
        </div>
      </div>
    {/if}
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

  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">TFS / ADO</h2>
    <div class="form-group">
      <label for="tfs-base-url">Base URL</label>
      <input id="tfs-base-url" type="text" bind:value={settings.tfs_base_url}
        placeholder="https://tfs.internal.example.com/tfs" />
    </div>
    <div class="form-group">
      <label for="tfs-collection">Collection or org</label>
      <input id="tfs-collection" type="text" bind:value={settings.tfs_collection}
        placeholder="DefaultCollection" />
    </div>
    <div class="form-group">
      <label for="tfs-default-project">Default project</label>
      <input id="tfs-default-project" type="text" bind:value={settings.tfs_default_project}
        placeholder="Consumer Soutions" />
    </div>
    <div class="form-group">
      <label for="tfs-default-area">Default area path</label>
      <input id="tfs-default-area" type="text" bind:value={settings.tfs_default_area_path}
        placeholder="MKT-Websites" />
    </div>
    <div class="form-group" style="margin-bottom:0">
      <label for="tfs-pat">PAT / access token</label>
      <input id="tfs-pat" type="password" bind:value={settings.tfs_pat}
        placeholder="TFS token" autocomplete="off" />
      <p class="hint">Defaults are used to scope read-only work item and release pulls. The preview below exercises the same ADO client the app will use for project ingestion.</p>
    </div>
    <div style="display:flex;align-items:center;gap:0.75rem;flex-wrap:wrap;margin-top:1rem">
      <button class="btn-secondary" onclick={runAdoPreview} disabled={adoChecking}>
        {adoChecking ? 'Checking…' : 'Run ADO preview'}
      </button>
      {#if adoPreviewData}
        <span class="text-muted" style="font-size:0.75rem">API version: {adoPreviewData.api_version}</span>
      {/if}
    </div>
    {#if adoError}
      <div class="error-msg" style="margin-top:0.75rem">{adoError}</div>
    {/if}
    {#if adoPreviewData}
      <div class="auth-results" style="margin-top:0.75rem">
        <div class="auth-row">
          <div>
            <strong>Preview scope</strong>
            <p class="auth-message">{adoPreviewData.project} · {adoPreviewData.area_path}</p>
          </div>
          <span class="badge badge-green">{adoPreviewData.work_items.length} work items</span>
        </div>
      </div>
      <pre class="config-sample" style="margin-top:0.75rem;max-height:22rem;overflow:auto"><code>{JSON.stringify(adoPreviewData, null, 2)}</code></pre>
    {/if}
  </section>

  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">Confluence</h2>
    <div class="form-group">
      <label for="confluence-base-url">Base URL</label>
      <input id="confluence-base-url" type="text" bind:value={settings.confluence_base_url}
        placeholder="https://confluence.internal.example.com" />
    </div>
    <div class="form-group">
      <label for="confluence-username">Username / email</label>
      <input id="confluence-username" type="text" bind:value={settings.confluence_username}
        placeholder="name@example.com" />
    </div>
    <div class="form-group" style="margin-bottom:0">
      <label for="confluence-token">Token</label>
      <input id="confluence-token" type="password" bind:value={settings.confluence_token}
        placeholder="Confluence token" autocomplete="off" />
      <p class="hint">Used by the app’s direct Confluence Cloud client for page lookup and search. External Claude/Codex workflows can use Atlassian’s official MCP server separately.</p>
    </div>
  </section>

  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">Microsoft 365</h2>
    <div class="form-group">
      <label for="microsoft-tenant-id">Tenant ID</label>
      <input id="microsoft-tenant-id" type="text" bind:value={settings.microsoft_tenant_id}
        placeholder="00000000-0000-0000-0000-000000000000" />
    </div>
    <div class="form-group">
      <label for="microsoft-client-id">Client ID</label>
      <input id="microsoft-client-id" type="text" bind:value={settings.microsoft_client_id}
        placeholder="00000000-0000-0000-0000-000000000000" />
    </div>
    <div class="form-group" style="margin-bottom:0">
      <label for="microsoft-client-secret">Client secret</label>
      <input id="microsoft-client-secret" type="password" bind:value={settings.microsoft_client_secret}
        placeholder="Microsoft Graph app secret" autocomplete="off" />
      <p class="hint">Staged shared connector settings for Microsoft Teams and Microsoft Loop. No active data pulls yet.</p>
    </div>
  </section>

  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">Embedded Claude</h2>
    <div class="form-group">
      <label for="anthropic-api-key">Anthropic API key</label>
      <input id="anthropic-api-key" type="password" bind:value={settings.anthropic_api_key}
        placeholder="sk-ant-..." autocomplete="off" />
    </div>
    <div class="form-group">
      <label for="anthropic-model">Model</label>
      <input id="anthropic-model" bind:value={settings.anthropic_model}
        placeholder="claude-sonnet-4-5" />
    </div>
    <div class="form-group" style="margin-bottom:0">
      <label for="ai-system-prompt">Default system prompt</label>
      <textarea id="ai-system-prompt" class="agent-textarea" bind:value={settings.ai_system_prompt} rows="4" placeholder="Focus on prioritization, blockers, missing context, and next checks."></textarea>
    </div>
  </section>

  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">MCP access</h2>
    <label class="toggle-row" style="margin-bottom:0.75rem">
      <input type="checkbox" bind:checked={settings.mcp_enabled} />
      <span>Enable MCP access for this app</span>
    </label>
    <p class="hint" style="margin-bottom:1rem">This only controls whether you intend to expose the app to external MCP clients. The embedded aside does not use MCP.</p>

    <div class="form-group">
      <div class="sample-label">Claude Desktop sample</div>
      <pre class="config-sample"><code>{`{
  "mcpServers": {
    "coverage-manager": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "<path-to-coverage-manager>/mcp-server/Cargo.toml"]
    }
  }
}`}</code></pre>
    </div>

    <div class="form-group" style="margin-bottom:0">
      <div class="sample-label">Codex sample</div>
      <pre class="config-sample"><code>{`{
  "mcp_servers": {
    "coverage-manager": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "<path-to-coverage-manager>/mcp-server/Cargo.toml"]
    }
  }
}`}</code></pre>
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

  <section class="card" style="padding:1.25rem">
    <h2 style="margin-bottom:1rem">Agent profiles</h2>
    {#if agentError}
      <div class="error-msg" style="margin-bottom:0.75rem">{agentError}</div>
    {/if}
    <div class="add-org-row" style="margin-bottom:0.75rem">
      <input type="text" bind:value={newAgentName} placeholder="Daily Focus"
        onkeydown={(e) => e.key === 'Enter' && addAgentProfile()} style="flex:1" />
      <button class="btn-secondary" onclick={addAgentProfile} disabled={!newAgentName.trim()}>
        Add profile
      </button>
    </div>
    <div class="agent-list">
      {#each agentProfiles as profile}
        <div class="agent-card">
          <div class="agent-card-header">
            <input bind:value={profile.name} />
            <label class="toggle-row">
              <input type="checkbox" bind:checked={profile.is_active} />
              <span>Active</span>
            </label>
          </div>
          <div class="form-group">
            <label for={`agent-goal-${profile.id}`}>Goal</label>
            <input id={`agent-goal-${profile.id}`} bind:value={profile.goal} placeholder="Rank what needs my attention now." />
          </div>
          <div class="form-group">
            <label for={`agent-instructions-${profile.id}`}>Instructions</label>
            <textarea id={`agent-instructions-${profile.id}`} class="agent-textarea" bind:value={profile.instructions} rows="3" placeholder="Explain priorities, blockers, and missing context across sources."></textarea>
          </div>
          <div class="agent-grid">
            <div class="form-group">
              <label for={`agent-scope-${profile.id}`}>Scope</label>
              <input id={`agent-scope-${profile.id}`} bind:value={profile.project_scope} placeholder="all_active_projects" />
            </div>
            <div class="form-group">
              <label for={`agent-sources-${profile.id}`}>Source types</label>
              <input id={`agent-sources-${profile.id}`} value={profile.source_types.join(', ')} oninput={(e) => { profile.source_types = (e.currentTarget as HTMLInputElement).value.split(',').map((v) => v.trim()).filter(Boolean); }} placeholder="github, docs, tfs, confluence, meetings" />
            </div>
            <div class="form-group">
              <label for={`agent-manual-weight-${profile.id}`}>Manual priority weight</label>
              <input id={`agent-manual-weight-${profile.id}`} type="number" bind:value={profile.weight_manual_priority} />
            </div>
            <div class="form-group">
              <label for={`agent-release-weight-${profile.id}`}>Release risk weight</label>
              <input id={`agent-release-weight-${profile.id}`} type="number" bind:value={profile.weight_release_risk} />
            </div>
            <div class="form-group">
              <label for={`agent-doc-weight-${profile.id}`}>Doc gap weight</label>
              <input id={`agent-doc-weight-${profile.id}`} type="number" bind:value={profile.weight_doc_gap} />
            </div>
            <div class="form-group">
              <label for={`agent-meeting-weight-${profile.id}`}>Meeting follow-up weight</label>
              <input id={`agent-meeting-weight-${profile.id}`} type="number" bind:value={profile.weight_meeting_followup} />
            </div>
          </div>
          <div class="agent-actions">
            <button class="btn-primary" onclick={() => persistAgentProfile(profile)} disabled={savingAgentId === profile.id}>
              {savingAgentId === profile.id ? 'Saving…' : 'Save profile'}
            </button>
            <button class="btn-ghost btn-danger-ghost" onclick={() => removeAgentProfile(profile.id)}>Delete</button>
          </div>
        </div>
      {/each}
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
  .auth-results { margin-top: 0.9rem; display: grid; gap: 0.75rem; }
  .auth-row {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-subtle);
  }
  .auth-message { margin: 0.25rem 0 0; font-size: 0.8125rem; color: var(--text-secondary); }
  code { font-family: var(--font-mono); background: var(--bg-muted); padding: 0.1em 0.3em; border-radius: 3px; }
  .org-list { list-style: none; margin: 0 0 0.75rem; padding: 0; display: flex; flex-direction: column; gap: 0.25rem; }
  .org-item { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 0; }
  .org-name { flex: 1; font-size: 0.875rem; }
  .add-org-row { display: flex; gap: 0.5rem; }
  .agent-list { display:grid; gap:0.75rem; }
  .agent-card { border:1px solid var(--border); border-radius:var(--radius-sm); padding:0.75rem; background:var(--bg-subtle); }
  .agent-card-header { display:flex; justify-content:space-between; gap:0.75rem; margin-bottom:0.75rem; }
  .agent-grid { display:grid; grid-template-columns:1fr 1fr; gap:0.75rem; }
  .agent-actions { display:flex; gap:0.5rem; justify-content:flex-end; }
  .agent-textarea { width:100%; resize:vertical; min-height:90px; padding:0.625rem; border:1px solid var(--border); border-radius:var(--radius-sm); font:inherit; }
  .toggle-row { display:flex; align-items:center; gap:0.4rem; font-size:0.8125rem; white-space:nowrap; }
  .btn-danger-ghost { color: var(--text-muted); font-size: 0.75rem; }
  .btn-danger-ghost:hover { color: var(--danger); }
  .sample-label { font-size: 0.8125rem; font-weight: 500; color: var(--text-secondary); }
  .config-sample {
    margin: 0.35rem 0 0;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: #0f172a;
    color: #e2e8f0;
    overflow: auto;
    font: 0.75rem/1.5 var(--font-mono);
  }

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
