<script lang="ts">
  import DOMPurify from 'dompurify';
  import { marked } from 'marked';

  interface Props {
    markdown?: string;
  }

  let { markdown = '' }: Props = $props();

  const html = $derived.by(() => {
    const rendered = marked.parse(markdown, {
      async: false,
      breaks: true,
      gfm: true,
    }) as string;
    return DOMPurify.sanitize(rendered, { USE_PROFILES: { html: true } });
  });
</script>

<div class="markdown-body">
  {@html html}
</div>

<style>
  .markdown-body {
    color: var(--text);
    line-height: 1.65;
    font-size: 0.92rem;
  }

  .markdown-body :global(h1),
  .markdown-body :global(h2),
  .markdown-body :global(h3),
  .markdown-body :global(h4) {
    margin: 1.5rem 0 0.75rem;
    line-height: 1.25;
  }

  .markdown-body :global(h1) {
    font-size: 1.55rem;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.5rem;
  }

  .markdown-body :global(h2) {
    font-size: 1.15rem;
  }

  .markdown-body :global(p),
  .markdown-body :global(ul),
  .markdown-body :global(ol),
  .markdown-body :global(blockquote),
  .markdown-body :global(pre),
  .markdown-body :global(table) {
    margin: 0 0 1rem;
  }

  .markdown-body :global(ul),
  .markdown-body :global(ol) {
    padding-left: 1.25rem;
  }

  .markdown-body :global(li + li) {
    margin-top: 0.3rem;
  }

  .markdown-body :global(code) {
    font-family: var(--font-mono);
    font-size: 0.88em;
    background: var(--bg-subtle);
    border-radius: 4px;
    padding: 0.1rem 0.3rem;
  }

  .markdown-body :global(pre) {
    background: #0f172a;
    color: #e2e8f0;
    border-radius: var(--radius);
    padding: 0.9rem 1rem;
    overflow-x: auto;
  }

  .markdown-body :global(pre code) {
    background: transparent;
    color: inherit;
    padding: 0;
  }

  .markdown-body :global(blockquote) {
    margin-left: 0;
    padding: 0.1rem 0 0.1rem 1rem;
    border-left: 3px solid var(--border);
    color: var(--text-secondary);
  }

  .markdown-body :global(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .markdown-body :global(th),
  .markdown-body :global(td) {
    border: 1px solid var(--border);
    padding: 0.5rem 0.625rem;
    text-align: left;
    vertical-align: top;
  }

  .markdown-body :global(img) {
    max-width: 100%;
    border-radius: var(--radius-sm);
  }

  .markdown-body :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 1.5rem 0;
  }
 </style>
