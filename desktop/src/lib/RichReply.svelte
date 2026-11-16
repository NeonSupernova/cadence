<script lang="ts">
  import { blocks, type Part } from '$lib/richText';
  let { text, known, onref }: { text: string; known: (id: number) => boolean; onref: (id: number) => void } = $props();
  const content = $derived(blocks(text));
</script>

{#snippet parts(list: Part[])}{#each list as part, i (i)}{#if part.kind === 'bold'}<strong>{part.text}</strong>{:else if part.kind === 'ref' && known(part.id)}<button class="inline-ref" onclick={() => onref(part.id)}>{part.text}</button>{:else}{part.text}{/if}{/each}{/snippet}

<div class="rich-reply">
  {#each content as block, b (b)}
    {#if block.kind === 'p'}
      <p>{#each block.lines as line, l (l)}{#if l > 0}<br />{/if}{@render parts(line)}{/each}</p>
    {:else if block.kind === 'ul'}
      <ul>{#each block.items as item, i (i)}<li>{@render parts(item)}</li>{/each}</ul>
    {:else}
      <ol>{#each block.items as item, i (i)}<li>{@render parts(item)}</li>{/each}</ol>
    {/if}
  {/each}
</div>

<style>
  .rich-reply { display: grid; gap: 0.55em; white-space: normal; }
  p, ul, ol { margin: 0; }
  ul, ol { padding-left: 1.15em; display: grid; gap: 0.3em; }
  li::marker { color: var(--accent, currentColor); }
  strong { font-weight: 650; }
  .inline-ref {
    font: inherit; font-size: 0.86em; font-weight: 600; letter-spacing: 0.01em;
    padding: 0.05em 0.45em; margin: 0 0.1em; border-radius: 999px; cursor: pointer;
    border: 1px solid color-mix(in oklab, var(--accent, #7c5cff) 45%, transparent);
    background: color-mix(in oklab, var(--accent, #7c5cff) 14%, transparent);
    color: inherit; vertical-align: baseline;
  }
  .inline-ref:hover { background: color-mix(in oklab, var(--accent, #7c5cff) 26%, transparent); }
  .inline-ref:focus-visible { outline: 2px solid var(--accent, #7c5cff); outline-offset: 2px; }
</style>
