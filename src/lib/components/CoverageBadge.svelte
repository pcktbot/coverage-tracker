<script lang="ts">
  export let pct: number | undefined = undefined;
  export let size: 'sm' | 'md' = 'md';

  function color(p: number | undefined): string {
    if (p === undefined) return 'badge-gray';
    if (p >= 80) return 'badge-green';
    if (p >= 60) return 'badge-yellow';
    return 'badge-red';
  }
</script>

<span class="badge {color(pct)}" class:sm={size === 'sm'}>
  {#if pct !== undefined}
    {pct.toFixed(1)}%
  {:else}
    —
  {/if}
</span>

<style>
  .sm { font-size: 0.6875rem; padding: 0.1rem 0.375rem; }
</style>
