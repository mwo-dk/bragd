<script>
  // Sources: where Games come from. Each is off until turned on, shows what one fetch downloads,
  // and has its schedule, the metered rule, a disk cap, what becomes of its downloads, and its
  // Strip rule with Preview (a dry run) and Apply. The Sources this version cannot fetch yet are
  // shown with their notice. A custom URL can be added.
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";
  import StripEditor from "./StripEditor.svelte";
  import JobLine from "./JobLine.svelte";

  /** @type {{ family: string }} */
  let { family } = $props();
  /** @type {any} */
  let page = $state(null);
  /** @type {string | null} */
  let open = $state(null);
  /** @type {any} */
  let draft = $state(null);
  let cap = $state("");
  /** @type {Record<string, number>} */
  let jobs = $state({});
  let error = $state("");
  let message = $state("");
  let name = $state("");
  let url = $state("");

  const emptyRule = () => ({ min_rating: null, rating_of: "both", speeds: [], rated_only: false, variants: [], date_from: "", date_to: "", min_plies: null, results: [], titled_only: false, players: [] });

  async function load() {
    try {
      page = await k.sources();
      error = "";
    } catch (e) {
      error = k.why(e);
    }
  }
  $effect(() => {
    load();
  });

  const shown = $derived(page ? page.sources.filter((/** @type {any} */ s) => !s.family || s.family === family) : []);

  function expand(/** @type {any} */ s) {
    if (open === s.id) return void (open = null);
    open = s.id;
    draft = JSON.parse(JSON.stringify(s.options));
    draft.strip ??= emptyRule();
    cap = s.diskCap;
  }

  async function save(/** @type {string} */ id, /** @type {any} */ options, /** @type {string | null} */ capText = null) {
    error = message = "";
    try {
      const o = { ...options, strip: options.strip ? { ...options.strip, min_rating: options.strip.min_rating || null, min_plies: options.strip.min_plies || null } : null };
      await k.sourceSet(id, o, capText);
      message = t("source.saved");
      await load();
    } catch (e) {
      error = k.why(e);
    }
  }

  async function start(/** @type {string} */ id, /** @type {Promise<number>} */ p) {
    error = "";
    try {
      jobs = { ...jobs, [id]: await p };
    } catch (e) {
      error = k.why(e);
    }
  }

  async function add() {
    error = message = "";
    try {
      await k.sourceAdd(name, url);
      name = url = "";
      await load();
    } catch (e) {
      error = k.why(e);
    }
  }
</script>

<div class="sources">
  <p class="hint">{t("source.intro")}</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  {#if !page && !error}<p class="hint">{t("helper.connecting")}</p>{/if}

  {#each shown as s (s.id)}
    <article class="panel source" class:off={!s.fetchable}>
      <div class="head">
        <label class="check">
          <input type="checkbox" checked={s.options.enabled} disabled={!s.fetchable} onchange={(e) => save(s.id, { ...s.options, enabled: e.currentTarget.checked })} />
          <strong>{s.name}</strong>
        </label>
        <span class="meta">{s.familyName} · {s.size} · {t("source.last", { when: s.last })}</span>
        <button onclick={() => expand(s)} aria-expanded={open === s.id} disabled={!s.fetchable}>{open === s.id ? t("source.less") : t("source.more")}</button>
        <button onclick={() => start(s.id, k.sourceFetch(s.id))} disabled={!s.fetchable || !s.options.enabled}>{t("source.fetch_now")}</button>
      </div>
      {#if s.notice}<p class="notice">{s.notice}</p>{/if}
      {#if s.error}<p class="error">{s.error}</p>{/if}
      {#if s.paused}<p class="notice">{t("source.paused")}</p>{/if}
      <small class="url">{s.url}</small>

      {#if open === s.id && draft}
        <div class="options">
          <label
            >{t("source.schedule")}
            <select bind:value={draft.schedule}>
              {#each page.schedules as p}<option value={p.value}>{p.label}</option>{/each}
            </select>
          </label>
          <label class="check"><input type="checkbox" bind:checked={draft.only_unmetered} /> {t("source.only_unmetered")}</label>
          <label>{t("source.disk_cap")} <input type="text" bind:value={cap} placeholder={t("disk.no_limit")} size="10" /></label>
          <label
            >{t("source.after_import_label")}
            <select bind:value={draft.after_import}>
              {#each page.afterImport as p}<option value={p.value}>{p.label}</option>{/each}
            </select>
          </label>
          {#if s.custom}
            <label class="wide">{t("source.url")} <input type="text" bind:value={draft.url} /></label>
          {/if}
        </div>
        <StripEditor rule={draft.strip} {page} />
        <div class="bar">
          <button class="primary" onclick={() => save(s.id, draft, cap)}>{t("source.save")}</button>
          <button onclick={() => save(s.id, draft, cap).then(() => start(s.id, k.sourceStrip(s.id, true)))} title={t("strip.preview_hint")}>{t("strip.preview")}</button>
          <button onclick={() => save(s.id, draft, cap).then(() => start(s.id, k.sourceStrip(s.id, false)))} title={t("strip.apply_hint")}>{t("strip.apply")}</button>
          <button onclick={() => (draft.strip = emptyRule())}>{t("strip.clear")}</button>
        </div>
      {/if}
      {#if jobs[s.id] !== undefined}<JobLine id={jobs[s.id]} ondone={load} />{/if}
    </article>
  {/each}

  <section class="panel add">
    <h3>{t("source.add_title")}</h3>
    <p class="hint">{t("source.add_hint")}</p>
    <div class="bar">
      <input type="text" bind:value={name} placeholder={t("source.add_name")} aria-label={t("source.add_name")} />
      <input class="grow" type="text" bind:value={url} placeholder="https://example.org/games.pgn" aria-label={t("source.url")} />
      <button onclick={add} disabled={!name.trim() || !url.trim()}>{t("source.add")}</button>
    </div>
  </section>
</div>

<style>
  .sources {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    max-width: 52rem;
  }
  .source {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .source.off {
    opacity: 0.75;
  }
  .head,
  .bar,
  .options {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 0.8rem;
    align-items: center;
  }
  .options {
    align-items: flex-end;
  }
  .meta,
  .url,
  .hint {
    color: var(--dim);
    font-size: 0.85em;
  }
  .meta {
    flex: 1;
  }
  .url {
    overflow-wrap: anywhere;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.9em;
  }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 0.35rem;
  }
  .wide,
  .grow {
    flex: 1;
    min-width: 12rem;
  }
  .notice {
    margin: 0;
    padding: 0.3rem 0.6rem;
    border-inline-start: 3px solid var(--board-highlight);
    background: var(--alt);
  }
  .error {
    color: var(--red);
    margin: 0;
  }
  p {
    margin: 0;
  }
  h3 {
    font-size: 1rem;
    margin: 0 0 0.3rem;
  }
</style>
