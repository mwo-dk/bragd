<script>
  // A Game in replay: its board at the step chosen, the moves to step through (← and →, Home and
  // End), the comments, and its evaluation profile as a small graph when it has one. Each
  // step's board is replayed in Rust from the position Rust laid out for it.
  import * as api from "./api.js";
  import { t, i18n } from "./i18n.svelte.js";
  import Board from "./Board.svelte";

  /** @type {{ replay: any, onclose: () => void, onstep?: (step: any) => void }} */
  let { replay, onclose, onstep } = $props();
  let at = $state(0);
  /** @type {any} */
  let st = $state(null);
  let flipped = $state(false);

  const step = $derived(replay.steps[at]);
  // The step's Comment as Rust sees it: whether to offer Translate, and its kept translation.
  /** @type {any} */
  let cv = $state(null);
  let translating = $state(false);
  let trError = $state("");
  $effect(() => {
    const c = step?.comment;
    // Asked again when the reader's language changes: the translation kept is per language.
    void i18n.lang;
    [cv, trError] = [null, ""];
    if (c) api.commentView(c).then((v) => step?.comment === c && (cv = v), () => {});
  });
  function translate() {
    const c = step.comment;
    [translating, trError] = [true, ""];
    api
      .commentTranslate(c)
      .then((tr) => step?.comment === c && (cv = { ...cv, translation: tr }), (e) => (trError = String(e)))
      .finally(() => (translating = false));
  }

  $effect(() => {
    const s = replay.steps[at];
    onstep?.(s);
    if (!replay.variant || !s?.fen) return void (st = null);
    api.replay(replay.variant, s.fen, []).then((x) => (st = x), () => (st = null));
  });

  const go = (/** @type {number} */ n) => (at = Math.max(0, Math.min(replay.steps.length - 1, n)));

  function keys(/** @type {KeyboardEvent} */ e) {
    if (/** @type {HTMLElement} */ (e.target)?.closest?.("input, select, textarea")) return;
    const to = { ArrowLeft: at - 1, ArrowRight: at + 1, Home: 0, End: replay.steps.length - 1 }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      go(to);
    }
  }
</script>

<svelte:window onkeydown={keys} />

<section class="replay">
  <header class="bar">
    <h2>{replay.title} <small>{replay.result}</small></h2>
    <button onclick={() => (flipped = !flipped)} aria-pressed={flipped}>{t("play.flip")}</button>
    <button onclick={onclose}>{t("common.close")}</button>
  </header>
  <div class="game">
    <div class="board-side">
      {#if st}
        <Board state={st} family={replay.family} {flipped} />
      {:else if replay.error}
        <p class="dim">{replay.error}</p>
      {/if}
      <div class="steps" role="group" aria-label={t("replay.steps")}>
        <button onclick={() => go(0)} title={t("replay.first")} aria-label={t("replay.first")}>⏮</button>
        <button onclick={() => go(at - 1)} title={t("replay.back")} aria-label={t("replay.back")}>◀</button>
        <span>{at} / {replay.steps.length - 1}</span>
        <button onclick={() => go(at + 1)} title={t("replay.forward")} aria-label={t("replay.forward")}>▶</button>
        <button onclick={() => go(replay.steps.length - 1)} title={t("replay.last")} aria-label={t("replay.last")}>⏭</button>
      </div>
      <p class="hint">{t("replay.keys")}</p>
    </div>
    <aside class="panel info">
      {#if replay.evp}
        <svg class="evp" viewBox="0 0 {replay.evp.width} {replay.evp.height}" role="img" aria-label={t("replay.evp")}>
          <title>{t("replay.evp")}</title>
          <line x1="0" x2={replay.evp.width} y1={replay.evp.height / 2} y2={replay.evp.height / 2} class="zero" />
          <polyline points={replay.evp.points} />
          {#if step?.evpX != null}<line x1={step.evpX} x2={step.evpX} y1="0" y2={replay.evp.height} class="now" />{/if}
        </svg>
      {/if}
      {#if step?.comment}
        {#if cv?.translation}
          <div class="comment">
            <p>{cv.translation.text}</p>
            <p class="machine">{t("replay.machine_translated", { model: cv.translation.model })}</p>
            <details>
              <summary>{t("replay.original")}</summary>
              <p>{step.comment}</p>
            </details>
          </div>
        {:else}
          <p class="comment">{step.comment}</p>
          {#if cv?.translatable}
            <button class="translate" onclick={translate} disabled={translating}>{translating ? t("replay.translating") : t("replay.translate")}</button>
          {/if}
        {/if}
        {#if trError}<p class="error" role="alert">{trError}</p>{/if}
      {/if}
      <ol class="moves">
        {#each replay.steps.slice(1) as s, i}
          <li><button class:current={at === i + 1} onclick={() => go(i + 1)}>{s.label}</button></li>
        {/each}
      </ol>
      {#if replay.error && st}<p class="error">{replay.error}</p>{/if}
      <details>
        <summary>{t("replay.headers")}</summary>
        <dl>
          {#each replay.headers as [k, v]}
            <dt>{k}</dt>
            <dd>{v}</dd>
          {/each}
        </dl>
      </details>
    </aside>
  </div>
</section>

<style>
  .replay {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }
  h2 {
    flex: 1;
    font-size: 1.1rem;
    margin: 0;
  }
  small,
  .dim,
  .hint {
    color: var(--dim);
  }
  .hint {
    font-size: 0.85em;
    margin: 0;
  }
  .game {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }
  .board-side {
    flex: 1 1 auto;
    min-width: 0;
    max-width: min(100%, calc(100vh - 19rem));
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .steps {
    display: flex;
    gap: 0.3rem;
    align-items: center;
    justify-content: center;
  }
  .info {
    flex: 0 0 16rem;
    max-height: calc(100vh - 9rem);
    overflow: auto;
  }
  .evp {
    width: 100%;
    height: auto;
    background: var(--alt);
    border-radius: var(--r-sm);
  }
  .evp polyline {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .evp .zero {
    stroke: var(--border);
  }
  .evp .now {
    stroke: var(--board-highlight);
    stroke-width: 2;
  }
  .comment {
    font-style: italic;
  }
  .comment p {
    margin: 0 0 0.3rem;
  }
  .machine {
    font-style: normal;
    font-size: 0.85em;
    color: var(--dim);
  }
  .translate {
    margin-bottom: 0.4rem;
  }
  .moves {
    list-style: none;
    padding: 0;
    margin: 0.4rem 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.15rem;
  }
  .moves button {
    background: none;
    border: none;
    padding: 0.1rem 0.3rem;
    font-variant-numeric: tabular-nums;
  }
  .moves button.current {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.1rem 0.6rem;
    font-size: 0.9em;
  }
  dt {
    font-weight: 600;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .error {
    color: var(--red);
  }
  @media (max-width: 760px) {
    .game {
      flex-direction: column;
    }
    .info {
      flex: none;
      width: 100%;
      box-sizing: border-box;
      max-height: none;
    }
    .board-side {
      width: 100%;
      max-width: 100%;
    }
  }
</style>
