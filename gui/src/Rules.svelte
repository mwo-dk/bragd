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

  // A diagram or table wider than the page scrolls on its own; the keyboard can reach it to scroll it.
  const scrollable = (/** @type {HTMLElement} */ node) => {
    for (const box of node.querySelectorAll("pre, table")) {
      /** @type {HTMLElement} */ (box).tabIndex = 0;
      if (box.tagName === "TABLE") box.setAttribute("role", "group");
    }
  };
</script>

<!-- The pieces a seat holds in hand (shogi, Seirawan) or has waiting to enter (Ur, senet), as Play shows them. -->
{#snippet hand(/** @type {any} */ st, /** @type {number} */ seat)}
  {@const pieces = st.board.hands.filter((/** @type {any} */ h) => h.seat === seat).flatMap((/** @type {any} */ h) => h.pieces).filter((/** @type {any} */ p) => p.count)}
  {#if pieces.length}
    <div class="tray">
      {#each pieces as p}
        <span class="hand"><svg viewBox="0 0 1 1"><use href="#{p.sprite}" width="1" height="1" /></svg>{p.count}</span>
      {/each}
    </div>
  {/if}
{/snippet}

<section class="rules">
  <h1>{family.name}: {t("sidebar.rules")}</h1>
  {#if page}
    {#if page.positions.length}
      <h2>{t("rules.positions")}</h2>
      <div class="lessons">
        {#each page.positions as l}
          <article class="panel">
            <h3>{l.title}</h3>
            <div class="mini">
              {#if family.id === "backgammon"}
                <BackgammonBoard state={l.state} />
              {:else}
                {@const flipped = l.position.toMove === 1 && family.id !== "go"}
                {@render hand(l.state, flipped ? 0 : 1)}
                <Board state={l.state} family={family.id} {flipped} />
                {@render hand(l.state, flipped ? 1 : 0)}
              {/if}
            </div>
            <p>{l.text}</p>
            {#if l.answer}
              <p class="answer">{t("rules.answer", { move: l.answer })}</p>
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
  .tray {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0.2rem 0;
  }
  .hand {
    display: inline-flex;
    align-items: center;
    gap: 0.15rem;
  }
  .hand svg {
    width: 1.5rem;
    height: 1.5rem;
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
