<script>
  // Analyse's panel: set up a position, run an engine on it and show its best lines (scored from
  // the first Side's view, moves in the Variant's notation, each one playable), what the
  // tablebases say for a chess ending, and a review of the whole Game whose marks the move list
  // shows. Rust runs the engine and turns its moves into entries.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import { current } from "./current.svelte.js";

  /** @type {{ family: any, game: {variant: string, initial: string|null, entries: string[]}, over: boolean, onentries: (entries: string[]) => void, onsetup: (initial: string|null, entries?: string[]) => void, marks: string[] }} */
  let { family, game, over, onentries, onsetup, marks = $bindable() } = $props();

  /** @type {{key: string, name: string, ready: boolean}[]} */
  let choices = $state([]);
  let engine = $state("");
  let on = $state(false);
  /** @type {any} */
  let now = $state(null);
  /** @type {any} */
  let table = $state(null);
  /** @type {any} */
  let review = $state(null);
  let text = $state("");
  let error = $state("");

  $effect(() => {
    api.engineChoices(game.variant).then((c) => {
      choices = c.engines;
      if (!c.engines.some((e) => e.key === engine)) engine = c.default ?? "";
    });
  });

  // The engine follows the position while it is on.
  $effect(() => {
    const g = { variant: game.variant, initial: game.initial, entries: [...game.entries] };
    if (!on || over) return;
    api
      .analysisStart(g, engine || null, 3)
      .then(() => (error = ""))
      .catch((e) => ((error = String(e)), (on = false)));
  });
  $effect(() => {
    if (!on) return;
    const id = setInterval(async () => {
      now = await api.analysisNow();
    }, 400);
    return () => clearInterval(id);
  });

  // What the tablebase says, for chess with seven pieces or fewer.
  $effect(() => {
    const g = { variant: game.variant, initial: game.initial, entries: [...game.entries] };
    table = null;
    if (family.id === "chess") api.tablebaseProbe(g).then((r) => (table = r)).catch(() => (table = null));
  });

  function toggle() {
    if (on) {
      on = false;
      api.analysisStop();
    } else on = true;
  }

  async function startReview() {
    try {
      await api.reviewStart({ variant: game.variant, initial: game.initial, entries: [...game.entries] }, engine || null);
      error = "";
      const id = setInterval(async () => {
        review = await api.reviewNow();
        marks = review?.marks ?? [];
        if (!review || review.done) clearInterval(id);
      }, 500);
    } catch (e) {
      error = String(e);
    }
  }

  function setUp() {
    api
      .positionSetup(game.variant, text)
      .then(() => (onsetup(text.trim()), (error = "")))
      .catch((e) => (error = String(e?.reason ?? e)));
  }
  const fromGame = $derived(current[family.id]);
  const play = (/** @type {any[]} */ moves, /** @type {number} */ upTo) => onentries([...game.entries, ...moves.slice(0, upTo + 1).map((m) => m.entry)]);
</script>

<div class="analysis">
  <details>
    <summary>{t("analyse.setup")}</summary>
    <textarea rows="2" bind:value={text} placeholder={t("analyse.paste")} aria-label={t("analyse.paste")}></textarea>
    <div class="row">
      <button onclick={setUp} disabled={!text.trim()}>{t("analyse.set_up")}</button>
      <button onclick={() => onsetup(null)}>{t("analyse.start_position")}</button>
      {#if fromGame}
        <button onclick={() => onsetup(fromGame.initial, fromGame.entries)}>{t("analyse.from_game")}</button>
      {/if}
    </div>
  </details>

  {#if choices.length}
    <div class="row">
      <select bind:value={engine} aria-label={t("analyse.engine")}>
        {#each choices as c}
          <option value={c.key}>{c.name}{c.ready ? "" : ` (${t("engines.not_ready")})`}</option>
        {/each}
      </select>
      <button class="primary" onclick={toggle} aria-pressed={on} disabled={over && !on}>{on ? t("analyse.stop") : t("analyse.start")}</button>
    </div>
  {:else}
    <p class="dim">{t("engines.none_for")}</p>
  {/if}

  {#if now && on}
    <p class="dim">{now.engine}{now.running ? " …" : ""}</p>
    <ol class="lines">
      {#each now.lines as l}
        <li>
          <span class="score">{l.score}</span>
          {#if l.depth}<span class="dim">{t("analyse.depth", { n: l.depth })}</span>{/if}
          <span class="pv">
            {#each l.moves as m, i}
              <button class="mv" onclick={() => play(l.moves, i)}>{m.notation}</button>
            {/each}
          </span>
        </li>
      {/each}
    </ol>
    {#if now.error}<p class="error">{now.error}</p>{/if}
  {/if}

  {#if table}
    <div class="table">
      <h3>{t("analyse.tablebase", { n: table.pieces })}</h3>
      {#if table.local}
        <p>{t("analyse.tablebase_local")}</p>
      {:else if table.verdict}
        <p><strong>{table.verdict}</strong>{#if table.dtz !== null} · DTZ {table.dtz}{/if}{#if table.dtm !== null} · DTM {table.dtm}{/if}</p>
        <div class="row">
          {#each table.moves as [entry, san, verdict]}
            <button class="mv" title={verdict} onclick={() => onentries([...game.entries, entry])}>{san}</button>
          {/each}
        </div>
      {:else if table.unavailable}
        <p class="dim">{t("analyse.tablebase_off")}</p>
      {:else if table.error}
        <p class="error">{table.error}</p>
      {/if}
    </div>
  {/if}

  {#if family.id !== "backgammon" && game.entries.length}
    <div class="row">
      <button onclick={startReview} disabled={review && !review.done}>{t("analyse.review")}</button>
      {#if review}
        <span class="dim">{review.done ? t("analyse.reviewed", { engine: review.engine }) : t("analyse.reviewing", { done: review.reviewed, total: review.total })}</span>
      {/if}
    </div>
    {#if review?.error}<p class="error">{review.error}</p>{/if}
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .analysis {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin: 0.4rem 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    align-items: center;
  }
  .row select {
    flex: 1 1 8rem;
    min-width: 0;
    max-width: 100%;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    font-family: var(--mono, monospace);
  }
  .lines {
    margin: 0;
    padding-inline-start: 1.2rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .score {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    margin-inline-end: 0.3rem;
  }
  .pv {
    display: inline;
  }
  .mv {
    padding: 0 0.2rem;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    text-decoration: underline dotted;
  }
  .mv:hover {
    background: color-mix(in srgb, var(--sel-bg) 35%, transparent);
  }
  h3 {
    font-size: 0.95rem;
    margin: 0.3rem 0 0;
  }
  p {
    margin: 0.2rem 0;
  }
  .dim {
    color: var(--dim);
    font-size: 0.9em;
  }
  .error {
    color: var(--red);
  }
</style>
