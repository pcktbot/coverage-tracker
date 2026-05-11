<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { orgs, activeOrg, refreshOrgs, refreshRepos, repos } from '$lib/stores/repos';
  import AICommandAside from '$lib/components/AICommandAside.svelte';
  import { getProject, getSettings, listAgentProfiles, listCachedConfluencePages, listProjects, setActiveOrg, type Settings, type AgentProfile, type ProjectSummary } from '$lib/api';

  let { children } = $props();

  const FONT_SIZE_KEY = 'cm-font-size';
  const DEFAULT_FONT_SIZE = 14;
  const MIN_FONT_SIZE = 12;
  const MAX_FONT_SIZE = 24;
  const STEP = 2;
  const DOCS_STORAGE_KEY = 'coverage-manager-docs-state';
  const COMMAND_CENTER_STATE_KEY = 'coverage-manager-command-center-state';

  let fontSize = $state(DEFAULT_FONT_SIZE);
  let aiOpen = $state(true);
  let projectContextName = $state<string | null>(null);
  let commandCenterContextLines = $state<string[]>([]);
  let projectConfluenceContextLines = $state<string[]>([]);
  let agentProfiles = $state<AgentProfile[]>([]);
  let aiSettings = $state<Pick<Settings, 'mcp_enabled' | 'anthropic_model' | 'ai_system_prompt' | 'anthropic_api_key'>>({
    mcp_enabled: true,
    anthropic_model: '',
    ai_system_prompt: '',
    anthropic_api_key: '',
  });

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
      const settings = await getSettings();
      agentProfiles = await listAgentProfiles();
      aiSettings = {
        mcp_enabled: settings.mcp_enabled,
        anthropic_model: settings.anthropic_model || '',
        ai_system_prompt: settings.ai_system_prompt || '',
        anthropic_api_key: settings.anthropic_api_key || '',
      };
    } catch (e) {
      console.error('[layout] onMount error:', e);
    }
  });

  async function switchOrg(name: string) {
    await setActiveOrg(name);
    activeOrg.set(name);
    await refreshRepos(name);
    goto('/dashboard');
  }

  const currentRepo = $derived.by(() => {
    const pathname = $page.url.pathname;
    if (!pathname.startsWith('/repo/')) return null;
    const id = Number($page.params.id);
    return $repos.find((repo) => repo.id === id) ?? null;
  });

  const currentDocsRepo = $derived.by(() => {
    if ($page.url.pathname !== '/docs') return null;
    try {
      const repoFromUrl = Number($page.url.searchParams.get('repo') ?? $page.url.searchParams.get('leftRepo'));
      if (Number.isFinite(repoFromUrl) && repoFromUrl > 0) {
        return $repos.find((repo) => repo.id === repoFromUrl) ?? null;
      }
      const raw = localStorage.getItem(DOCS_STORAGE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw) as { repoId?: number };
      return parsed.repoId ? ($repos.find((repo) => repo.id === parsed.repoId) ?? null) : null;
    } catch {
      return null;
    }
  });

  const aiContextLines = $derived.by(() => {
    const pathname = $page.url.pathname;
    const lines = [];
    lines.push(`View: ${viewLabel(pathname)}`);
    if ($activeOrg) lines.push(`Active org: ${$activeOrg}`);
    if (pathname === '/') {
      lines.push('Mode: command center');
      lines.push('Goal: identify what needs attention now across active projects');
      lines.push(...commandCenterContextLines);
    }
    if (pathname === '/dashboard') {
      lines.push('Mode: coverage dashboard');
      lines.push('Goal: inspect enabled repos, coverage status, and operational drift');
    }
    if (pathname.startsWith('/projects/')) {
      lines.push(`Project: ${projectContextName ?? `#${$page.params.id}`}`);
      lines.push('Goal: review project status, linked repos, and leadership notes');
      lines.push(...projectConfluenceContextLines);
    }
    if (pathname.startsWith('/repo/') && currentRepo) {
      lines.push(`Repo: ${currentRepo.org}/${currentRepo.name}`);
      if (currentRepo.local_path) lines.push('Local checkout available');
      lines.push('Goal: inspect repo health, docs, and connected sources');
    }
    if (pathname === '/docs' && currentDocsRepo) {
      lines.push(`Docs repo: ${currentDocsRepo.org}/${currentDocsRepo.name}`);
      lines.push('Goal: analyze repo docs and branch-specific context');
    }
    if (pathname === '/settings') {
      lines.push('Goal: inspect connector, auth, and agent-profile configuration');
    }
    return lines;
  });

  $effect(() => {
    const pathname = $page.url.pathname;
    if (!pathname.startsWith('/projects/')) {
      projectContextName = null;
      projectConfluenceContextLines = [];
      return;
    }

    const projectId = Number($page.params.id);
    if (!Number.isFinite(projectId)) {
      projectContextName = null;
      return;
    }

    void (async () => {
      try {
        const project = await getProject(projectId);
        if ($page.url.pathname.startsWith('/projects/') && Number($page.params.id) === projectId) {
          projectContextName = project.name;
          const confluencePageIds = project.doc_refs
            .filter((ref) => ref.kind === 'confluence' && ref.confluence_page_id)
            .map((ref) => ref.confluence_page_id!)
            .filter(Boolean);
          const cachedPages = confluencePageIds.length > 0
            ? await listCachedConfluencePages(confluencePageIds)
            : [];
          if ($page.url.pathname.startsWith('/projects/') && Number($page.params.id) === projectId) {
            projectConfluenceContextLines = cachedPages.slice(0, 3).map((cachedPage, index) =>
              `Confluence ${index + 1}: ${cachedPage.title} · page ${cachedPage.page_id} · cached ${cachedPage.last_fetched_at} · ${cachedPage.excerpt}`,
            );
          }
        }
      } catch {
        if ($page.url.pathname.startsWith('/projects/')) {
          projectContextName = `#${projectId}`;
          projectConfluenceContextLines = [];
        }
      }
    })();
  });

  $effect(() => {
    const pathname = $page.url.pathname;
    if (pathname !== '/') {
      commandCenterContextLines = [];
      return;
    }

    void (async () => {
      try {
        const projects = await listProjects();
        const topProjects = [...projects]
          .sort((a, b) => b.focus_score - a.focus_score)
          .slice(0, 3);

        let selectedProject: ProjectSummary | null = null;
        try {
          const raw = localStorage.getItem(COMMAND_CENTER_STATE_KEY);
          if (raw) {
            const parsed = JSON.parse(raw) as { selectedProjectId?: number };
            if (Number.isFinite(parsed.selectedProjectId)) {
              selectedProject = projects.find((project) => project.id === parsed.selectedProjectId) ?? null;
            }
          }
        } catch {
          // Ignore malformed local state and fall back to top projects.
        }

        if (!selectedProject) {
          selectedProject = topProjects[0] ?? null;
        }

        const nextLines = [
          `Tracked projects: ${projects.length}`,
          `Active projects: ${projects.filter((project) => project.is_active).length}`,
        ];

        if (selectedProject) {
          nextLines.push(
            `Selected project: ${selectedProject.name} · ${selectedProject.platform_name ?? 'No platform'} · ${selectedProject.status.replaceAll('_', ' ')} · focus ${selectedProject.focus_score} · repos ${selectedProject.repo_count} · sources ${selectedProject.source_link_count}`,
          );
        }

        topProjects.forEach((project, index) => {
          nextLines.push(
            `Priority ${index + 1}: ${project.name} · ${project.status.replaceAll('_', ' ')} · focus ${project.focus_score} · priority ${project.manual_priority}`,
          );
        });

        if ($page.url.pathname === '/') {
          if (selectedProject) {
            try {
              const fullProject = await getProject(selectedProject.id);
              const confluencePageIds = fullProject.doc_refs
                .filter((ref) => ref.kind === 'confluence' && ref.confluence_page_id)
                .map((ref) => ref.confluence_page_id!)
                .filter(Boolean);
              const cachedPages = confluencePageIds.length > 0
                ? await listCachedConfluencePages(confluencePageIds)
                : [];

              cachedPages.slice(0, 3).forEach((page, index) => {
                nextLines.push(
                  `Confluence ${index + 1}: ${page.title} · page ${page.page_id} · cached ${page.last_fetched_at} · ${page.excerpt}`,
                );
              });
            } catch {
              // Keep command-center context useful even if Confluence cache lookup fails.
            }
          }
          commandCenterContextLines = nextLines;
        }
      } catch {
        if ($page.url.pathname === '/') {
          commandCenterContextLines = [];
        }
      }
    })();
  });

  function viewLabel(pathname: string): string {
    if (pathname === '/') return 'Projects';
    if (pathname === '/dashboard') return 'Coverage';
    if (pathname === '/docs') return 'Docs';
    if (pathname === '/settings') return 'Settings';
    if (pathname.startsWith('/projects/')) return 'Project Detail';
    if (pathname.startsWith('/repo/')) return 'Repo Detail';
    if (pathname.startsWith('/sessions')) return 'Sessions';
    return pathname;
  }
</script>

<div class="layout">
  <nav class="toolbar">
    {#if $orgs.length > 0}
      <div class="nav-group">
        <span class="group-label">Org</span>
        <ul class="nav-list">
          {#each $orgs as org}
            <li>
              <button
                class="nav-link"
                class:active={org.name === $activeOrg}
                onclick={() => switchOrg(org.name)}
              >
                {org.name}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    <div class="nav-group">
      <span class="group-label">Views</span>
      <ul class="nav-list">
        <li>
          <a
            href="/dashboard"
            class="nav-link"
            class:active={$page.url.pathname === '/dashboard'}
          >
            Coverage
          </a>
        </li>
        <li>
          <a
            href="/"
            class="nav-link"
            class:active={$page.url.pathname === '/'}
          >
            Projects
          </a>
        </li>
        <li>
          <a
            href="/docs"
            class="nav-link"
            class:active={$page.url.pathname === '/docs'}
          >
            Docs
          </a>
        </li>
        <li>
          <a
            href="/sessions"
            class="nav-link"
            class:active={$page.url.pathname.startsWith('/sessions')}
          >
            Sessions
          </a>
        </li>
      </ul>
    </div>

    <div class="font-size-controls">
      <button
        class="font-btn"
        onclick={decrease}
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
        onclick={increase}
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

    <button
      class="ai-toggle"
      class:active={aiOpen}
      onclick={() => (aiOpen = !aiOpen)}
      title="Toggle Claude workspace"
    >
      AI
    </button>

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

  <div class="content-shell">
    <main class="main-content">
      {@render children?.()}
    </main>
    <AICommandAside
      open={aiOpen}
      contextTitle="Current context"
      contextLines={aiContextLines}
      agentProfiles={agentProfiles}
      mcpEnabled={aiSettings.mcp_enabled}
      anthropicModel={aiSettings.anthropic_model}
      systemPrompt={aiSettings.ai_system_prompt}
      tokenConfigured={Boolean(aiSettings.anthropic_api_key)}
    />
  </div>
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

  .ai-toggle {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-secondary);
    margin-left: 0.5rem;
  }
  .ai-toggle.active {
    background: #ecfdf3;
    border-color: #b7e4c7;
    color: #157347;
  }

  .content-shell {
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    overflow: hidden;
  }

  .main-content {
    flex: 1 1 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 1.5rem;
    min-width: 0;
  }

  @media (max-width: 960px) {
    .ai-toggle {
      display: none;
    }
  }
</style>
