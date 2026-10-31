<script>
  // Chess's opening name (ECO) for the position, and the opening books' moves, each playable.
  // Rust names the opening and reads the books.
  import * as api from "./api.js";
  import { t, num } from "./i18n.svelte.js";

  /** @type {{ game: {variant: string, initial: string|null, entries: string[]}, onplay: (entry: string) => void }} */
  let { game, onplay } = $props();
  /** @type {any} */
  let known = $state(null);

  $effect(() => {
    const g = { variant: game.variant, initial: game.initial, entries: [...game.entries] };
    api.positionKnown(g).then((k) => (known = k)).catch(() => (known = null));
  });
</script>

{#if known?.opening}
  <p class="opening"><strong>{known.eco}</strong> {known.opening}</p>
{/if}
{#if known?.book.length}
  <div class="book">
    <span class="dim">{t("books.moves")}</span>
    {#each known.book as m}
      <button title={m.book} onclick={() => onplay(m.entry)}>{m.notation} <small>{num(Math.round(m.share))}%</small></button>
    {/each}
  </div>
{/if}

<style>
  .opening {
    margin: 0.3rem 0;
  }
  .book {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    align-items: center;
  }
  .dim,
  small {
    color: var(--dim);
  }
</style>
