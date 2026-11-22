<script lang="ts">
  import { invoke, desktop } from '$lib/bridge';
  let { text = '', html = '' }: { text?: string; html?: string } = $props();
  let links = $state<{ label: string; url: string }[]>([]);
  let error = $state('');
  $effect(() => {
    if (typeof DOMParser === 'undefined' || !html) { links = []; return; }
    const doc = new DOMParser().parseFromString(html, 'text/html');
    links = [...doc.querySelectorAll('a[href]')].map(a => ({label: a.textContent?.trim() || 'Linked resource', url: a.getAttribute('href') || ''}))
      .filter(a => /^(https?:\/\/|\/(?!\/))/.test(a.url)).filter((a, i, all) => all.findIndex(b => b.url === a.url) === i);
  });
  async function open(url: string) { try { await invoke('open_canvas_link', {path: url}); } catch(e) { error = String(e); } }
</script>
{#if text}<div class="cw-prose">{text}</div>{/if}
{#if links.length}<div class="cw-links">{#each links as link}<button disabled={!desktop()} onclick={() => open(link.url)}>{link.label} ↗</button>{/each}</div>{/if}
{#if error}<p role="alert" class="cw-error">{error}</p>{/if}
