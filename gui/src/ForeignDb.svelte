<script>
  // A Foreign database read in place: open one with the file dialog, list its Games (filter and
  // sort them), open one in replay, import the file into the Library as a Helper job, or save it
  // as another file. The file stays open in Rust while the window runs.
  import * as k from "./knowledge.js";
  import * as api from "./api.js";
  import { t, i18n } from "./i18n.svelte.js";
  import GameList from "./GameList.svelte";
  import Replay from "./Replay.svelte";
  import JobLine from "./JobLine.svelte";

  /** @type {{ family: any, onstep: (family: string, step: any) => void, onsnap: (snap: any) => void }} */
  let { family, onstep, onsnap } = $props();
  // The page Family's formats from Rust, joined as the language joins "this or that".
  const formats = $derived(new Intl.ListFormat(i18n.lang, { type: "disjunction" }).format(family.formats));
  /** @type {any} */
  let file = $state(null);
  let player = $state("");
  let opening = $state("");
  let sort = $state("number");
  let desc = $state(false);
  /** @type {any} */
  let game = $state(null);
  /** @type {number | null} */
  let selected = $state(null);
  /** @type {number | null} */
  let job = $state(null);
  /** @type {any} */
  let confirm = $state(null);
  let message = $state("");
  let error = $state("");
  let version = $state(0);

  let gone = false;
  let polling = false;
  // The file open now, and how far it is read: asked again until it is done, by one chain only.
  async function poll() {
    if (polling) return;
    polling = true;
    try {
      while (!gone) {
        file = await k.dbCurrent();
        version++;
        if (!file || file.done) break;
        await new Promise((r) => setTimeout(r, 700));
      }
    } finally {
      polling = false;
    }
  }
  $effect(() => {
    poll();
    return () => (gone = true);
  });

  const fail = (/** @type {any} */ e) => (error = k.why(e));

  async function pick() {
    error = message = "";
    try {
      const f = await k.dbPick();
      if (!f) return;
      file = f;
      game = null;
      selected = null;
      poll();
    } catch (e) {
      fail(e);
    }
  }

  const query = $derived({ player, opening, sort, desc });
  const fetch = (/** @type {number} */ from, /** @type {number} */ count) => k.dbRows(query, from, count);

  function sortBy(/** @type {string} */ col) {
    if (sort === col) desc = !desc;
    else (sort = col), (desc = false);
  }

  async function open(/** @type {any} */ row) {
    selected = row.id;
    try {
      game = await k.dbGame(row.id);
    } catch (e) {
      fail(e);
    }
  }

  async function saveAs() {
    error = message = "";
    try {
      const r = await k.dbSaveAs();
      if (r?.confirm) confirm = r;
      else if (r) message = t("db.saved", { path: r.path });
    } catch (e) {
      fail(e);
    }
  }

  async function saveOver() {
    try {
      const r = await k.dbSaveTo();
      message = t("db.saved", { path: r.path });
    } catch (e) {
      fail(e);
    }
    confirm = null;
  }
</script>

<div class="foreign">
  <div class="bar">
    <button class="primary" onclick={pick}>{t("db.open")}</button>
    {#if file}
      <button onclick={async () => (job = await k.dbImport().catch((e) => (fail(e), null)))}>{t("db.import")}</button>
      <button onclick={saveAs}>{t("db.save_as")}</button>
      <button onclick={() => k.dbClose().then(() => ((file = null), (game = null)))}>{t("common.close")}</button>
    {/if}
  </div>

  {#if file}
    <p class="file">
      <strong>{file.name}</strong> · {file.familyName} · {file.status}
      {#if !file.done}<progress max="1" value={file.read}></progress>{/if}
    </p>
    {#if file.familyOff}
      <p class="notice" role="status">
        {t("db.family_off", { family: file.familyName })}
        <button onclick={() => api.familyTurnOn(file.family).then((s) => (onsnap(s), k.dbCurrent().then((f) => (file = f))), fail)}>{t("db.turn_on")}</button>
      </p>
    {/if}
    {#if file.downloaded}
      <p class="notice">{t("db.downloaded")}</p>
    {/if}
  {:else}
    <p class="dim">{family.formats.length ? t("db.none_hint", { formats }) : t("db.no_formats", { family: family.name })}</p>
  {/if}

  {#if confirm}
    <div class="notice" role="alertdialog" aria-label={t("db.overwrite_title")}>
      <p>{t("db.overwrite", { path: confirm.path })}</p>
      <button onclick={saveOver}>{t("db.overwrite_yes")}</button>
      <button class="primary" onclick={() => (confirm = null)}>{t("common.cancel")}</button>
    </div>
  {/if}
  {#if message}<p role="status">{message}</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if job !== null}<JobLine id={job} />{/if}

  {#if game}
    <Replay replay={game} onclose={() => (game = null)} onstep={(s) => onstep(game.family, s)} />
  {:else if file}
    <div class="filters">
      <input type="text" bind:value={player} placeholder={t("db.filter_player")} aria-label={t("db.filter_player")} />
      <input type="text" bind:value={opening} placeholder={t("db.filter_opening")} aria-label={t("db.filter_opening")} />
    </div>
    <GameList {fetch} version={[version, player, opening, sort, desc]} {sort} {desc} onsort={sortBy} onopen={open} {selected} />
  {/if}
</div>

<style>
  .foreign {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
    flex: 1;
  }
  .bar,
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .filters input {
    flex: 1;
    min-width: 8rem;
  }
  .file {
    margin: 0;
  }
  progress {
    vertical-align: middle;
  }
  .dim {
    color: var(--dim);
  }
  .notice {
    margin: 0;
    padding: 0.4rem 0.6rem;
    border-inline-start: 3px solid var(--board-highlight);
    background: var(--alt);
  }
  .notice button {
    margin-inline-end: 0.4rem;
  }
  .error {
    color: var(--red);
  }
</style>
