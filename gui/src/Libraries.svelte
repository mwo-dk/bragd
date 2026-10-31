<script>
  // Libraries: each with its Games, size, Duplicates and Near-duplicates; browse one, review
  // what was merged and what looks cut short (never merged on its own), "Dedupe now", its
  // Slices, and the Source priority that decides whose headers win.
  import * as k from "./knowledge.js";
  import { t, num } from "./i18n.svelte.js";
  import GameList from "./GameList.svelte";
  import Replay from "./Replay.svelte";
  import JobLine from "./JobLine.svelte";
  import SliceEditor from "./SliceEditor.svelte";

  /** @type {{ family: string, variants: any[], slice: any, position: any, onstep: (family: string, step: any) => void, onslices: () => void }} */
  let { family, variants, slice = null, position, onstep, onslices } = $props();
  /** @type {any[]} */
  let libs = $state([]);
  /** @type {any} */
  let lib = $state(null);
  let view = $state("games");
  let player = $state("");
  /** @type {any} */
  let game = $state(null);
  /** @type {any} */
  let review = $state(null);
  /** @type {number | null} */
  let job = $state(null);
  /** The Export to Parquet job. @type {number | null} */
  let exporting = $state(null);
  /** @type {string[]} */
  let priority = $state([]);
  let newSource = $state("");
  let error = $state("");
  let message = $state("");
  /** @type {any} */
  let editing = $state(null);
  /** @type {number | null} */
  let dragging = $state(null);

  async function load() {
    try {
      libs = await k.libraries();
      priority = await k.priorityGet();
      if (lib) lib = libs.find((l) => l.path === lib.path) ?? null;
    } catch (e) {
      error = k.why(e);
    }
  }
  $effect(() => {
    load();
  });

  // A Slice chosen in the sidebar: its Library, open on it.
  $effect(() => {
    if (!slice || !libs.length) return;
    const l = libs.find((x) => x.path === slice.library);
    if (l) (lib = l), (view = "slice"), (editing = l.slices.find((/** @type {any} */ s) => s.name === slice.name) ?? null);
  });

  $effect(() => {
    if (lib && view === "review") k.libraryReview(lib.path).then((r) => (review = r), (e) => (error = k.why(e)));
  });

  const fetch = (/** @type {number} */ from, /** @type {number} */ count) => k.libraryRows(lib.path, { players: player.trim() ? [player] : [], family }, from, count);

  function choose(/** @type {any} */ l) {
    lib = l;
    view = "games";
    game = null;
    review = null;
  }

  async function exportLib() {
    error = "";
    try {
      exporting = (await k.libraryExport(lib.path)) ?? exporting;
    } catch (e) {
      error = k.why(e);
    }
  }

  async function saveOrder(/** @type {string[]} */ list) {
    error = message = "";
    try {
      priority = await k.prioritySave(list);
      message = t("priority.saved");
    } catch (e) {
      error = k.why(e);
    }
  }

  function drop(/** @type {number} */ to) {
    if (dragging === null || dragging === to) return;
    const list = [...priority];
    const [moved] = list.splice(dragging, 1);
    list.splice(to, 0, moved);
    dragging = null;
    saveOrder(list);
  }

  function move(/** @type {number} */ i, /** @type {number} */ by) {
    const j = i + by;
    if (j < 0 || j >= priority.length) return;
    const list = [...priority];
    [list[i], list[j]] = [list[j], list[i]];
    saveOrder(list);
  }
</script>

<div class="libs">
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if !libs.length}
    <p class="dim">{t("lib.none")}</p>
  {/if}
  <table class="list">
    <thead>
      <tr><th>{t("lib.name")}</th><th>{t("lib.games")}</th><th>{t("lib.size")}</th><th>{t("lib.duplicates")}</th><th>{t("lib.near")}</th></tr>
    </thead>
    <tbody>
      {#each libs as l (l.path)}
        <tr class:current={lib?.path === l.path}>
          <td><button class="link" onclick={() => choose(l)} title={l.path}>{l.name}</button>{#if l.error}<span class="error"> {l.error}</span>{/if}</td>
          <td class="num">{num(l.games)}</td>
          <td class="num">{l.size}</td>
          <td class="num">{num(l.duplicates)}</td>
          <td class="num">{num(l.nearDuplicates)}</td>
        </tr>
      {/each}
    </tbody>
  </table>

  {#if lib}
    <nav class="tabs">
      {#each ["games", "review", "slice"] as v}
        <button class:current={view === v} onclick={() => ((view = v), (game = null), v === "slice" && !slice && (editing = null))}>{t(`lib.tab.${v}`)}</button>
      {/each}
      {#each lib.slices as s}
        <button class:current={view === "slice" && editing?.name === s.name} onclick={() => ((view = "slice"), (editing = s))}>{s.name}{s.materialised ? " ⧉" : ""}</button>
      {/each}
    </nav>
    <div class="bar">
      <button onclick={exportLib} title={t("lib.export_hint")}>{t("lib.export")}</button>
      {#if exporting !== null}<JobLine id={exporting} />{/if}
    </div>

    {#if view === "games"}
      {#if game}
        <Replay replay={game} onclose={() => (game = null)} onstep={(s) => onstep(game.family, s)} />
      {:else}
        <input type="text" bind:value={player} placeholder={t("db.filter_player")} aria-label={t("db.filter_player")} />
        <GameList {fetch} version={[lib.path, player, lib.games, family]} onopen={async (r) => (game = await k.libraryGame(lib.path, r.id))} />
      {/if}
    {:else if view === "review"}
      <div class="bar">
        <button class="primary" onclick={async () => (job = await k.libraryDedupe(lib.path).catch((e) => ((error = k.why(e)), null)))}>{t("lib.dedupe")}</button>
        <span class="hint">{t("lib.dedupe_hint")}</span>
      </div>
      {#if job !== null}<JobLine id={job} ondone={load} />{/if}
      {#if review}
        <h3>{t("lib.duplicates")} ({num(review.duplicates.length)})</h3>
        <ul class="review">
          {#each review.duplicates as [g, sources]}
            <li>
              <button class="link" onclick={async () => ((game = await k.libraryGame(lib.path, g.id)), (view = "games"))}>{g.white} – {g.black}, {g.date} ({g.result})</button>
              <small>{sources.join(" · ")}</small>
            </li>
          {/each}
        </ul>
        <h3>{t("lib.near")} ({num(review.near.length)})</h3>
        <p class="hint">{t("lib.near_hint")}</p>
        <ul class="review">
          {#each review.near as [short, long]}
            <li>
              <button class="link" onclick={async () => ((game = await k.libraryGame(lib.path, short.id)), (view = "games"))}>{short.white} – {short.black} ({t("lib.moves", { n: short.moves })})</button>
              ⊂
              <button class="link" onclick={async () => ((game = await k.libraryGame(lib.path, long.id)), (view = "games"))}>{long.white} – {long.black} ({t("lib.moves", { n: long.moves })})</button>
            </li>
          {/each}
        </ul>
      {/if}
    {:else if view === "slice"}
      {#key `${lib.path}/${editing?.name ?? ""}`}
        <SliceEditor library={lib.path} slice={editing} {family} {variants} {position} {onstep} onsaved={() => (load(), onslices())} />
      {/key}
    {/if}
  {/if}

  <section class="priority">
    <h3>{t("priority.title")}</h3>
    <p class="hint">{t("priority.hint")}</p>
    <ol>
      {#each priority as s, i (s)}
        <li
          draggable="true"
          class:dragging={dragging === i}
          ondragstart={() => (dragging = i)}
          ondragover={(e) => e.preventDefault()}
          ondrop={() => drop(i)}
        >
          <span class="grip" aria-hidden="true">⠿</span>
          <span class="name">{s}</span>
          <button onclick={() => move(i, -1)} disabled={i === 0} aria-label={t("priority.up", { source: s })}>↑</button>
          <button onclick={() => move(i, 1)} disabled={i === priority.length - 1} aria-label={t("priority.down", { source: s })}>↓</button>
          <button onclick={() => saveOrder(priority.filter((x) => x !== s))} aria-label={t("priority.remove", { source: s })}>✕</button>
        </li>
      {/each}
    </ol>
    <div class="bar">
      <input type="text" bind:value={newSource} placeholder={t("priority.add_hint")} aria-label={t("priority.add_hint")} />
      <button onclick={() => newSource.trim() && (saveOrder([...priority, newSource]), (newSource = ""))}>{t("priority.add")}</button>
    </div>
    {#if message}<p role="status">{message}</p>{/if}
  </section>
</div>

<style>
  .libs {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .list {
    border-collapse: collapse;
    width: 100%;
    max-width: 48rem;
  }
  th,
  td {
    text-align: start;
    padding: 0.25rem 0.5rem;
    border-bottom: 1px solid var(--border);
  }
  .num {
    text-align: end;
    font-variant-numeric: tabular-nums;
  }
  tr.current {
    background: color-mix(in srgb, var(--sel-bg) 35%, transparent);
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: inherit;
    text-decoration: underline;
    text-align: start;
  }
  .tabs,
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    align-items: center;
  }
  .tabs button.current {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }
  .review {
    margin: 0;
    padding-inline-start: 1.2rem;
  }
  .review small {
    display: block;
    color: var(--dim);
  }
  h3 {
    font-size: 1rem;
    margin: 0.6rem 0 0.2rem;
  }
  .hint,
  .dim {
    color: var(--dim);
    font-size: 0.9em;
    margin: 0;
  }
  .priority ol {
    padding: 0;
    list-style: none;
    max-width: 22rem;
    margin: 0.3rem 0;
  }
  .priority li {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.2rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    margin-bottom: 0.2rem;
    background: var(--raised);
    cursor: grab;
  }
  .priority li.dragging {
    opacity: 0.5;
  }
  .priority .name {
    flex: 1;
  }
  .priority li button {
    padding: 0 0.4rem;
  }
  .grip {
    color: var(--dim);
  }
  .error {
    color: var(--red);
  }
  @media (max-width: 760px) {
    th:nth-child(4),
    td:nth-child(4),
    th:nth-child(5),
    td:nth-child(5) {
      display: none;
    }
  }
</style>
