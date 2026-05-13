<script lang="ts">
  let { snapshot }: { snapshot: string | null } = $props();

  type Parsed = { plugins: string[]; mcp_servers: string[]; skills: string[] };

  const parsed = $derived.by<Parsed | null>(() => {
    if (!snapshot) return null;
    try {
      const v = JSON.parse(snapshot);
      return {
        plugins: Array.isArray(v.plugins) ? v.plugins : [],
        mcp_servers: Array.isArray(v.mcp_servers) ? v.mcp_servers : [],
        skills: Array.isArray(v.skills) ? v.skills : [],
      };
    } catch {
      return null;
    }
  });
</script>

<section class="loaded">
  <h4>Loaded at spawn</h4>
  {#if !parsed}
    <p class="muted">No snapshot captured.</p>
  {:else}
    <dl>
      <dt>Plugins</dt>
      <dd>
        {#each parsed.plugins as p}<span class="chip">{p}</span>{:else}<span class="muted">none</span>{/each}
      </dd>
      <dt>MCP servers</dt>
      <dd>
        {#each parsed.mcp_servers as m}<span class="chip">{m}</span>{:else}<span class="muted">none</span>{/each}
      </dd>
      <dt>Skills</dt>
      <dd>
        {#each parsed.skills as s}<span class="chip">{s}</span>{:else}<span class="muted">none</span>{/each}
      </dd>
    </dl>
  {/if}
</section>

<style>
  .loaded { margin-top: 0.5rem; }
  h4 { margin: 0 0 0.25rem; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--text-muted); }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 0.75rem; margin: 0; font-size: 0.8125rem; }
  dt { color: var(--text-muted); }
  dd { margin: 0; display: flex; flex-wrap: wrap; gap: 0.25rem; }
  .chip { background: var(--bg-muted); border-radius: 999px; padding: 0.0625rem 0.5rem; font-size: 0.75rem; font-family: var(--font-mono, monospace); }
  .muted { color: var(--text-muted); font-style: italic; }
</style>
