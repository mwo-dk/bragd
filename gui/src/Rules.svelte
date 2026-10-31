<script>
  // A Family's Rules: its pages from docs/rules, made into HTML by Rust, and its teaching
  // positions, each drawn from the board Rust replayed.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import Board from "./Board.svelte";
  import BackgammonBoard from "./BackgammonBoard.svelte";

  /** @type {{ family: any }} */
  let { family } = $props();
  /** @type {{html: string, positions: any[]} | null} */
  let page = $state(null);

  $effect(() => {
    const id = family.id;
    page = null;
    api.rulesPage(id).then((p) => (page = p));
  });

  // The pages link to each other and to the repository: they are read here, not followed.
  const stay = (/** @type {MouseEvent} */ e) => {
    if (e.target instanceof Element && e.target.closest("a")) e.preventDefault();
  };

  // A diagram wider than the page scrolls on its own; the keyboard can reach it to scroll it.
  const scrollable = (/** @type {HTMLElement} */ node) => {
    for (const pre of node.querySelectorAll("pre")) pre.tabIndex = 0;
  };
</script>

<section class="rules">
  {#if page}
    {#if page.positions.length}
      <h2>{t("rules.positions")}</h2>
      <div class="lessons">
        {#each page.positions as l}
          <article class="panel">
            <h3>{l.position.title}</h3>
            <div class="mini">
              {#if family.id === "backgammon"}
                <BackgammonBoard state={l.state} />
              {:else}
                <Board state={l.state} family={family.id} flipped={l.position.toMove === 1 && family.id !== "go"} />
              {/if}
            </div>
            <p>{l.position.text}</p>
            {#if l.position.answer}
              <p class="answer">{t("rules.answer", { move: l.position.answer })}</p>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="doc" onclick={stay} use:scrollable>{@html page.html}</div>
  {/if}
</section>

<style>
  .rules {
    max-width: 60rem;
  }
  .lessons {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
    gap: 0.8rem;
  }
  .mini {
    max-width: 16rem;
  }
  h3 {
    margin: 0 0 0.4rem;
    font-size: 1rem;
  }
  .answer {
    color: var(--dim);
  }
  /* A long reference or a wide table never widens the page: words break, tables scroll. */
  .doc {
    overflow-wrap: anywhere;
  }
  .doc :global(table) {
    border-collapse: collapse;
    font-size: 0.9em;
    display: block;
    max-width: 100%;
    overflow-x: auto;
  }
  .doc :global(td),
  .doc :global(th) {
    border: 1px solid var(--border);
    padding: 0.2rem 0.4rem;
  }
  .doc :global(code) {
    font-family: var(--mono-font);
    overflow-wrap: anywhere;
    word-break: break-all;
  }
  .doc :global(pre) {
    max-width: 100%;
    overflow-x: auto;
  }
  .doc :global(pre code) {
    word-break: normal;
  }
  .doc :global(a) {
    color: var(--accent);
    cursor: default;
  }
</style>
