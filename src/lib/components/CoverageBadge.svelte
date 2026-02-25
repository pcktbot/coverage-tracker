<script lang="ts">
  interface Props {
    pct?: number | null;
    size?: 'sm' | 'md';
  }
  let { pct = undefined, size = 'md' }: Props = $props();

  function color(p: number | null | undefined): string {
    if (p == null) return 'badge-gray';
    if (p >= 80) return 'badge-green';
    if (p >= 60) return 'badge-yellow';
    return 'badge-red';
  }
</script>

<span class="badge {color(pct)}" class:sm={size === 'sm'}>
  {#if pct != null}
    {pct.toFixed(1)}%
  {:else}
    —
  {/if}
</span>

<style>
  .sm { font-size: 0.6875rem; padding: 0.1rem 0.375rem; }
</style>
