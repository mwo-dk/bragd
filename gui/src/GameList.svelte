<script>
  // A list of Games of any length: only the rows in sight (and a few around them) are asked of
  // Rust, as the list scrolls. A header click sorts by that column, when the list can sort.
  import { t, num } from "./i18n.svelte.js";

  /** @type {{ fetch: (from: number, count: number) => Promise<{total: number, rows: any[]}>, version: any, sort?: string, desc?: boolean, onsort?: (col: string) => void, onopen: (row: any) => void, selected?: number | null }} */
  let { fetch, version, sort = "", desc = false, onsort, onopen, selected = null } = $props();

  const ROW = 28;
  const COLS = ["number", "white", "white_elo", "black", "black_elo", "result", "date", "event", "eco", "moves"];
  let total = $state(0);
  /** @type {any[]} */
  let rows = $state([]);
  let first = $state(0);
  let top = $state(0);
  let height = $state(400);
  let error = $state("");

  async function load(/** @type {number} */ from) {
    const count = Math.ceil(height / ROW) + 20;
    try {
      const r = await fetch(from, count);
      total = r.total;
      rows = r.rows;
      first = from;
      error = "";
    } catch (e) {
      error = String(/** @type {any} */ (e)?.reason ?? e);
    }
  }

  // Again from the top when what is listed changes.
  $effect(() => {
    version;
    load(Math.max(0, Math.floor(top / ROW) - 10));
  });

  function scrolled(/** @type {Event} */ e) {
    top = /** @type {HTMLElement} */ (e.currentTarget).scrollTop;
    const at = Math.floor(top / ROW);
    if (at < first + 5 || at + Math.ceil(height / ROW) > first + rows.length - 5) load(Math.max(0, at - 10));
  }

  function key(/** @type {KeyboardEvent} */ e, /** @type {any} */ r) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onopen(r);
    }
  }
</script>

<div class="list" role="table" aria-rowcount={total}>
  <div class="row head" role="row">
    {#each COLS as c}
      <span class="c {c}" role="columnheader" aria-sort={sort === c ? (desc ? "descending" : "ascending") : "none"}>
        {#if onsort}
          <button onclick={() => onsort(c)}>{t(`db.col.${c}`)}{sort === c ? (desc ? " ▾" : " ▴") : ""}</button>
        {:else}
          {t(`db.col.${c}`)}
        {/if}
      </span>
    {/each}
  </div>
  <div class="body" onscroll={scrolled} bind:clientHeight={height}>
    <div style="height: {total * ROW}px; position: relative">
      {#each rows as r, i (r.id)}
        <div
          class="row"
          class:selected={selected === r.id}
          role="row"
          tabindex="0"
          style="top: {(first + i) * ROW}px"
          onclick={() => onopen(r)}
          onkeydown={(e) => key(e, r)}
        >
          {#each COLS as c}
            <span class="c {c}" role="cell">{c === "number" ? num(r.n) : r[c === "white_elo" ? "whiteElo" : c === "black_elo" ? "blackElo" : c]}</span>
          {/each}
        </div>
      {/each}
    </div>
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    min-height: 12rem;
    flex: 1;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--raised);
    font-size: 0.9em;
  }
  .body {
    flex: 1;
    overflow-y: auto;
    min-height: 10rem;
  }
  .row {
    display: grid;
    grid-template-columns: 3.5rem 1fr 3.5rem 1fr 3.5rem 4.5rem 6rem 1fr 3rem 3.5rem;
    align-items: center;
    height: 28px;
    box-sizing: border-box;
    padding: 0 0.4rem;
    gap: 0.4rem;
    position: absolute;
    inset-inline: 0;
    cursor: pointer;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }
  .row:hover,
  .row:focus-visible {
    background: color-mix(in srgb, var(--sel-bg) 35%, transparent);
  }
  .row.selected {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }
  .row.head {
    position: static;
    font-weight: 600;
    cursor: default;
    background: var(--alt);
  }
  .head button {
    background: none;
    border: none;
    padding: 0;
    font-weight: 600;
    color: inherit;
  }
  .c {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .number,
  .white_elo,
  .black_elo,
  .moves {
    text-align: end;
    font-variant-numeric: tabular-nums;
  }
  .error {
    color: var(--red);
    margin: 0.3rem;
  }
  @media (max-width: 760px) {
    .row {
      grid-template-columns: 1fr 1fr 4rem;
    }
    .number,
    .white_elo,
    .black_elo,
    .date,
    .event,
    .eco,
    .moves {
      display: none;
    }
  }
</style>
