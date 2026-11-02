<script>
  // Ur's pyramid dice as Rust hands them: each die, whether a marked tip landed up. A new throw
  // tumbles them in; with reduced motion they simply appear.
  import { t } from "./i18n.svelte.js";

  /** @type {{ pyramids: boolean[], roll: number }} */
  let { pyramids, roll } = $props();
  const marked = $derived(pyramids.filter(Boolean).length);
</script>

{#key roll}
  <div class="dice" role="img" aria-label={t("play.pyramids", { marked, dice: pyramids.length })}>
    {#each pyramids as up, i}
      <svg class="die" class:up viewBox="0 0 45 45" style="animation-delay: {i * 60}ms" aria-hidden="true">
        <use href={up ? "#ur-die-marked" : "#ur-die-plain"} />
      </svg>
    {/each}
  </div>
{/key}

<style>
  .dice {
    display: flex;
    gap: 0.35rem;
    align-items: end;
  }
  .die {
    width: 2.6rem;
    height: 2.6rem;
    animation: tumble 480ms cubic-bezier(0.2, 0.8, 0.3, 1.2) both;
  }
  .die.up {
    filter: drop-shadow(0 0 0.25rem var(--board-highlight, #e8c547));
  }
  @keyframes tumble {
    from {
      transform: translateY(-1.2rem) rotate(-220deg) scale(0.6);
      opacity: 0;
    }
    to {
      transform: none;
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .die {
      animation: none;
    }
  }
</style>
