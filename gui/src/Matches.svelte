<script>
  // The Engine matches item of a Family with engines: a form that sets a match up and hands it
  // to the Helper, and each match's standings, Elo and SPRT as it goes. Rust checks the setup,
  // runs the match and works out every number shown.
  import * as api from "./api.js";
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";

  /** @type {{ family: any, go: (page: any) => void }} */
  let { family, go } = $props();
  /** @type {any} */
  let page = $state(null);
  let error = $state("");
  let busy = $state(false);

  let variant = $state("");
  /** @type {string[]} */
  let chosen = $state([]);
  let kind = $state("round_robin");
  let rounds = $state(2);
  let ms = $state(1000);
  // A clock for each engine instead of a fixed time per move: seconds, as a tournament sets it.
  let clock = $state(false);
  let base = $state(60);
  let inc = $state(1);
  let period = $state(0);
  let parallel = $state(1);
  /** @type {string|null} */
  let book = $state(null);
  let bookDepth = $state(8);
  let elo0 = $state(0);
  let elo1 = $state(5);
  let alpha = $state(0.05);
  let beta = $state(0.05);
  let resign = $state(true);
  let resignCp = $state(700);
  let resignMoves = $state(4);
  let draw = $state(true);
  let drawCp = $state(10);
  let drawAfter = $state(40);
  let drawMoves = $state(8);
  let maxMoves = $state(200);
  let tablebases = $state(true);
  let library = $state("");

  const fail = (/** @type {any} */ e) => (error = String(e));
  const load = () =>
    api
      .matchesPage(family.id)
      .then((p) => {
        page = p;
        if (!variant || !p.variants.some((/** @type {any} */ v) => v.key === variant)) variant = p.variants[0]?.key ?? "";
        if (!library) library = p.library;
      })
      .catch(fail);
  load();
  // A match under way shows how far it is.
  $effect(() => {
    if (!page?.matches.some((/** @type {any} */ m) => m.live)) return;
    const id = setInterval(load, 2000);
    return () => clearInterval(id);
  });

  const engines = $derived(page?.variants.find((/** @type {any} */ v) => v.key === variant)?.engines ?? []);
  // Engines of another Variant are not offered.
  $effect(() => {
    const keys = engines.map((/** @type {any} */ e) => e.key);
    if (chosen.some((c) => !keys.includes(c))) chosen = chosen.filter((c) => keys.includes(c));
  });
  const toggle = (/** @type {string} */ key, /** @type {boolean} */ on) => (chosen = on ? [...chosen, key] : chosen.filter((c) => c !== key));
  const libraries = $derived(page ? [...(page.libraries.some((/** @type {any} */ l) => l[0] === page.library) ? [] : [[page.library, "matches.bragd"]]), ...page.libraries] : []);

  async function pickBook() {
    try {
      book = (await api.matchBookPick()) ?? book;
    } catch (e) {
      fail(e);
    }
  }

  async function start(/** @type {SubmitEvent} */ ev) {
    ev.preventDefault();
    busy = true;
    const setup = {
      variant,
      engines: chosen,
      kind: kind === "sprt" ? { kind, elo0: +elo0, elo1: +elo1, alpha: +alpha, beta: +beta } : { kind },
      rounds: +rounds,
      limit: { Time: +ms },
      clock: clock ? { base_ms: Math.round(+base * 1000), inc_ms: Math.round(+inc * 1000), moves: +period || null } : null,
      parallel: +parallel,
      book,
      book_depth: +bookDepth,
      adjudication: {
        resign_cp: resign ? +resignCp : null,
        resign_moves: +resignMoves,
        draw_cp: draw ? +drawCp : null,
        draw_after: +drawAfter,
        draw_moves: +drawMoves,
        max_moves: +maxMoves || null,
        tablebases: page.tablebases && tablebases,
      },
      library,
      seed: 0,
    };
    try {
      await api.matchStart(setup);
      error = "";
      await load();
    } catch (e) {
      fail(e);
    }
    busy = false;
  }
</script>

<section class="matches">
  <h1>{family.name}: {t("sidebar.matches")}</h1>
  <p>{t("matches.intro")}</p>
  {#if page}
    <h2>{t("matches.new")}</h2>
    <form class="panel" onsubmit={start}>
      <div class="row">
        <span>{t("matches.variant")}</span>
        <Picker value={variant} choices={page.variants.map((/** @type {any} */ v) => ({ value: v.key, label: v.name }))} label={t("matches.variant")} onchange={(v) => (variant = v)} />
      </div>
      <fieldset>
        <legend>{t("matches.engines")}</legend>
        {#each engines as e (e.key)}
          <label class="check"><input type="checkbox" checked={chosen.includes(e.key)} onchange={(ev) => toggle(e.key, ev.currentTarget.checked)} /> {e.name}</label>
        {:else}
          <p class="dim">{t("matches.none")}</p>
          <button type="button" onclick={() => go({ kind: "item", family: family.id, item: "engines" })}>{t("matches.to_engines")}</button>
        {/each}
        {#if engines.length}<p class="dim">{t("matches.engines_hint")}</p>{/if}
      </fieldset>
      <fieldset>
        <legend>{t("matches.kind")}</legend>
        {#each ["round_robin", "gauntlet", "sprt"] as k (k)}
          <label class="check"><input type="radio" name="m-kind" value={k} bind:group={kind} /> {t(`matches.kind.${k}`)}</label>
        {/each}
        {#if kind === "sprt"}
          <div class="grid">
            <label>{t("matches.elo0")} <input type="number" step="0.5" bind:value={elo0} /></label>
            <label>{t("matches.elo1")} <input type="number" step="0.5" bind:value={elo1} /></label>
            <label>{t("matches.alpha")} <input type="number" step="any" min="0.001" max="0.499" bind:value={alpha} /></label>
            <label>{t("matches.beta")} <input type="number" step="any" min="0.001" max="0.499" bind:value={beta} /></label>
          </div>
        {/if}
      </fieldset>
      <div class="grid">
        <label>{kind === "sprt" ? t("matches.rounds_sprt") : t("matches.rounds")} <input type="number" min="1" bind:value={rounds} /></label>
        <label>{t("matches.parallel")} <input type="number" min="1" max={page.cpus} bind:value={parallel} /></label>
      </div>
      <fieldset>
        <legend>{t("matches.time")}</legend>
        <label class="check"><input type="radio" name="m-time" value={false} bind:group={clock} /> {t("matches.time.move")}</label>
        <label class="check"><input type="radio" name="m-time" value={true} bind:group={clock} /> {t("matches.time.clock")}</label>
        <div class="grid">
          {#if clock}
            <label>{t("matches.base")} <input type="number" min="1" step="any" bind:value={base} /></label>
            <label>{t("matches.inc")} <input type="number" min="0" step="any" bind:value={inc} /></label>
            <label>{t("matches.period")} <input type="number" min="0" bind:value={period} /></label>
          {:else}
            <label>{t("matches.ms")} <input type="number" min="10" step="any" bind:value={ms} /></label>
          {/if}
        </div>
      </fieldset>
      <fieldset>
        <legend>{t("matches.book")}</legend>
        <p class="path">{book ?? t("matches.book_none")}</p>
        <div class="actions">
          <button type="button" onclick={pickBook}>{t("matches.book_pick")}</button>
          {#if book}<button type="button" onclick={() => (book = null)}>{t("matches.book_clear")}</button>{/if}
          {#if book?.toLowerCase().endsWith(".bin")}<label>{t("matches.book_depth")} <input type="number" min="1" max="40" bind:value={bookDepth} /></label>{/if}
        </div>
      </fieldset>
      <fieldset>
        <legend>{t("matches.adjudication")}</legend>
        <label class="check wide"><input type="checkbox" bind:checked={resign} /> {t("matches.resign")}</label>
        <div class="grid">
          <label>{t("matches.resign_cp")} <input type="number" min="1" bind:value={resignCp} disabled={!resign} /></label>
          <label>{t("matches.resign_moves")} <input type="number" min="1" bind:value={resignMoves} disabled={!resign} /></label>
        </div>
        <label class="check wide"><input type="checkbox" bind:checked={draw} /> {t("matches.draw")}</label>
        <div class="grid">
          <label>{t("matches.draw_cp")} <input type="number" min="0" bind:value={drawCp} disabled={!draw} /></label>
          <label>{t("matches.draw_after")} <input type="number" min="0" bind:value={drawAfter} disabled={!draw} /></label>
          <label>{t("matches.draw_moves")} <input type="number" min="1" bind:value={drawMoves} disabled={!draw} /></label>
        </div>
        <div class="grid">
          <label>{t("matches.max_moves")} <input type="number" min="0" bind:value={maxMoves} /></label>
        </div>
        {#if page.tablebases}<label class="check wide"><input type="checkbox" bind:checked={tablebases} /> {t("matches.tablebases")}</label>{/if}
      </fieldset>
      <div class="row">
        <span>{t("matches.library")}</span>
        <Picker value={library} choices={libraries.map((/** @type {any} */ l) => ({ value: l[0], label: l[1] }))} label={t("matches.library")} onchange={(v) => (library = v)} />
      </div>
      <div class="actions">
        <button class="primary" disabled={busy || chosen.length < 2}>{t("matches.start")}</button>
      </div>
    </form>

    <h2>{t("matches.list")}</h2>
    {#if page.error}<p class="error" role="alert">{page.error}</p>{/if}
    {#each page.matches as m (m.id)}
      <article class="panel match">
        <header>
          <strong>{m.title}</strong>
          <span class="dim">{m.kind} · {m.statusText} · {m.library}</span>
        </header>
        {#if m.live}
          <progress max="1" value={m.progress ?? undefined}></progress>
        {/if}
        <small>{m.count}</small>
        <table>
          <thead>
            <tr>
              <th scope="col">{t("matches.th.engine")}</th>
              <th scope="col">{t("matches.th.games")}</th>
              <th scope="col">{t("matches.th.score")}</th>
              <th scope="col">{t("matches.th.points")}</th>
              <th scope="col">{t("matches.th.elo")}</th>
            </tr>
          </thead>
          <tbody>
            {#each m.rows as r (r.name)}
              <tr><td>{r.name}</td><td>{r.games}</td><td>{r.score}</td><td>{r.points}</td><td>{r.elo}</td></tr>
            {/each}
          </tbody>
        </table>
        {#if m.sprt}<p role="status">{m.sprt}</p>{/if}
        {#if m.error}<p class="error">{m.error}</p>{/if}
        {#if m.live}<div class="actions"><button onclick={() => k.jobCancel(m.id).then(load)}>{t("common.cancel")}</button></div>{/if}
      </article>
    {:else}
      <p class="dim">{t("matches.empty")}</p>
    {/each}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .matches {
    max-width: 52rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  h2 {
    font-size: 1.05rem;
    margin: 0.8rem 0 0.2rem;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1rem;
    align-items: center;
  }
  fieldset > p,
  .wide {
    flex-basis: 100%;
    margin: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr));
    gap: 0.4rem 1rem;
    align-items: end;
    flex-basis: 100%;
  }
  .grid label:not(.check) {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .row,
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }
  .row > span {
    min-width: 8rem;
  }
  .path {
    overflow-wrap: anywhere;
  }
  .match header {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 0.4rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    margin: 0.3rem 0;
  }
  th,
  td {
    text-align: start;
    padding: 0.15rem 0.6rem 0.15rem 0;
    font-variant-numeric: tabular-nums;
  }
  td:last-child {
    white-space: nowrap;
  }
  th {
    font-weight: normal;
    color: var(--dim);
  }
  progress {
    width: 100%;
  }
  .dim {
    color: var(--dim);
    font-size: 0.9em;
  }
  .error {
    color: var(--red);
  }
</style>
