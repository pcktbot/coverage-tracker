<script lang="ts">
  import type { CoverageTrendPoint } from '$lib/api';

  export let points: CoverageTrendPoint[] = [];
  export let width = 80;
  export let height = 28;

  $: values = points
    .map((p) => p.overall_coverage)
    .filter((v): v is number => v !== undefined && v !== null);

  $: min = values.length ? Math.min(...values) : 0;
  $: max = values.length ? Math.max(...values) : 100;
  $: range = max - min || 1;

  function toX(i: number): number {
    return values.length < 2 ? width / 2 : (i / (values.length - 1)) * width;
  }
  function toY(v: number): number {
    return height - ((v - min) / range) * (height - 4) - 2;
  }

  $: polyline = values.map((v, i) => `${toX(i)},${toY(v)}`).join(' ');
  $: lastVal = values[values.length - 1];
</script>

{#if values.length > 1}
  <svg {width} {height} aria-label="coverage trend">
    <polyline
      points={polyline}
      fill="none"
      stroke={lastVal !== undefined && lastVal >= 80 ? 'var(--success)' : lastVal !== undefined && lastVal >= 60 ? 'var(--warning)' : 'var(--danger)'}
      stroke-width="1.5"
      stroke-linejoin="round"
      stroke-linecap="round"
    />
    {#if lastVal !== undefined}
      <circle
        cx={toX(values.length - 1)}
        cy={toY(lastVal)}
        r="2.5"
        fill={lastVal >= 80 ? 'var(--success)' : lastVal >= 60 ? 'var(--warning)' : 'var(--danger)'}
      />
    {/if}
  </svg>
{:else}
  <span class="text-muted" style="font-size:0.75rem">—</span>
{/if}
