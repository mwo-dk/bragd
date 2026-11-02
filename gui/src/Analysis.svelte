<script>
  // Analyse's panel: set up a position, run an engine on it and show its best lines (scored from
  // the first Side's view, moves in the Variant's notation, each one playable), what the
  // tablebases say for a chess ending, and a review of the whole Game: its Annotations, worded
  // by Rust, listed here and marked in the move list; and Commentary on the last move or a
  // reviewed one, which the language model writes from the store's facts through the Helper's
  // queue. Rust runs the engine, reads and writes the Position store, and turns its moves into
  // entries.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";
  import { current } from "./current.svelte.js";

  /** @type {{ family: any, game: {variant: string, initial: string|null, entries: string[]}, over: boolean, onentries: (entries: string[]) => void, onsetup: (initial: string|null, entries?: string[]) => void, marks: string[], titles: string[], onmore: (() => void) | null }} */
  let { family, game, over, onentries, onsetup, marks = $bindable(), titles = $bindable(), onmore } = $props();

  /** The engines there to run. @type {{key: string, name: string}[]} */
  let choices = $state([]);
  let more = $state(false);
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
  /** Commentary on the last move. @type {any} */
  let com = $state(null);
  let writing = $state(false);
  /** Commentary written for reviewed moves, by index. @type {Record<number, any>} */
  let noteCom = $state({});

  $effect(() => {
    api.engineChoices(game.variant).then((c) => {
      choices = c.engines;
      more = c.more;
      if (!c.engines.some((e) => e.key === engine)) engine = c.default ?? c.engines[0]?.key ?? "";
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

  /** @type {ReturnType<typeof setInterval> | undefined} */
  let reviewing;
  // Leaving Analyse stops its engine and its review.
  $effect(() => () => {
    clearInterval(reviewing);
    api.analysisStop();
    api.reviewStop();
  });

  async function startReview() {
    try {
      await api.reviewStart({ variant: game.variant, initial: game.initial, entries: [...game.entries] }, engine || null);
      error = "";
      noteCom = {};
      clearInterval(reviewing);
      reviewing = setInterval(async () => {
        review = await api.reviewNow();
        marks = review?.marks ?? [];
        titles = review?.titles ?? [];
        if (!review || review.done) clearInterval(reviewing);
      }, 500);
    } catch (e) {
      error = String(e);
    }
  }

  // Commentary for the last move: looked up again when the Game moves, the engine changes, or
  // an analysis or review ends and the store may know more.
  // Derived, so the analysis's polling does not ask again each time: only when it settles.
  const settled = $derived(!now?.running && !(review && !review.done));
  $effect(() => {
    const g = { variant: game.variant, initial: game.initial, entries: [...game.entries] };
    void settled;
    api.commentaryView(g, engine || null).then((v) => (com = v), () => (com = null));
  });

  // The last move's Commentary shows beside its Annotation when the review lists it, not twice.
  const lastNoted = $derived(!!review?.notes.some((/** @type {any} */ n) => n.index === game.entries.length - 1));
  /** @param {any} n a reviewed move's note */
  const written = (n) => noteCom[n.index] ?? (n.index === game.entries.length - 1 ? com?.shown : null);

  /** @param {number|null} index the reviewed move's, or null for the last move */
  async function writeCommentary(index) {
    const entries = index === null ? [...game.entries] : game.entries.slice(0, index + 1);
    writing = true;
    try {
      const w = await api.commentaryWrite({ variant: game.variant, initial: game.initial, entries }, engine || null);
      if (index === null) com = { ...com, shown: w };
      else noteCom[index] = w;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      writing = false;
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
      <Picker value={engine} choices={choices.map((c) => ({ value: c.key, label: c.name }))} label={t("analyse.engine")} onchange={(v) => (engine = v)} />
      <button class="primary" onclick={toggle} aria-pressed={on} disabled={over && !on}>{on ? t("analyse.stop") : t("analyse.start")}</button>
    </div>
  {:else if more}
    <p class="dim">{t("engines.none_ready")}</p>
  {:else}
    <p class="dim">{t("engines.none_for")}</p>
  {/if}
  {#if more && onmore}
    <button class="link" onclick={onmore}>{t("engines.get_more")}</button>
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
    {#if now.note}
      <p class="note"><span class="label">{now.note.label}{now.note.mark}</span> {now.note.text}</p>
    {/if}
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
    {#if review?.notes.length}
      <h3>{t("analyse.notes")}</h3>
      <ul class="notes">
        {#each review.notes as n (n.index)}
          <li>
            <span class="label" class:bad={n.mark.startsWith("?")}>{n.label}{n.mark}</span> {n.text}
            {#if written(n)}
              <p class="written"><span class="machine">{t("commentary.label", { model: written(n).model })}</span> {written(n).text}</p>
            {:else if com?.model}
              <button class="link" onclick={() => writeCommentary(n.index)} disabled={writing}>{t("commentary.write")}</button>
            {/if}
          </li>
        {/each}
      </ul>
    {:else if review?.done && !review.error}
      <p class="dim">{t("analyse.notes_none")}</p>
    {/if}
  {/if}

  {#if com && !lastNoted}
    <section class="commentary" aria-label={t("commentary.title")}>
      {#if com.shown}
        <p class="written"><span class="machine">{t("commentary.label", { model: com.shown.model })}</span> {com.shown.text}</p>
      {:else}
        <div class="row">
          <button onclick={() => writeCommentary(null)} disabled={!com.offered || writing} title={com.why ?? ""}>{writing ? t("commentary.writing") : t("commentary.write")}</button>
          {#if com.why}<span class="dim">{com.why}</span>{/if}
        </div>
      {/if}
    </section>
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
  .row :global(.picker) {
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
  .notes {
    margin: 0;
    padding-inline-start: 1.2rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .note {
    margin: 0.2rem 0;
  }
  .label {
    font-weight: 600;
    margin-inline-end: 0.2rem;
  }
  .label.bad {
    color: var(--red);
  }
  .written {
    margin: 0.2rem 0;
    padding-inline-start: 0.5rem;
    border-inline-start: 3px solid var(--dim);
  }
  .machine {
    display: block;
    color: var(--dim);
    font-size: 0.85em;
  }
</style>
