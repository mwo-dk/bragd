<script>
  // *Analyse in background* for a saved Slice: its Games of the Family shown, the Engines to ask
  // (those that play one of the Family's Variants), each with the oldest version that may answer,
  // a depth or a time per position, and how many of its most frequent positions.
  // Rust builds the jobs and the Helper runs them; their progress shows here and under Jobs.
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";
  import JobLine from "./JobLine.svelte";

  /** @type {{ library: string, name: string, family: string, variants: {key: string}[], onclose: () => void }} */
  let { library, name, family, variants, onclose } = $props();
  /** @type {{key: string, name: string}[]} */
  let engines = $state([]);
  let more = $state(false);
  /** @type {Record<string, boolean>} */
  let picked = $state({});
  /** @type {Record<string, string>} */
  let min = $state({});
  let by = $state("depth");
  let depth = $state(20);
  let seconds = $state(1);
  let top = $state(1000);
  /** @type {number[]} */
  let jobs = $state([]);
  let error = $state("");

  $effect(() => {
    k.backgroundEngines(variants.map((v) => v.key)).then((c) => {
      engines = c.engines;
      more = c.more;
      for (const d of c.defaults) picked[d] = true;
    }, (e) => (error = k.why(e)));
  });

  async function queue() {
    error = "";
    try {
      const chosen = engines.filter((e) => picked[e.key]).map((e) => ({ engine: e.key, minVersion: min[e.key] ?? "" }));
      jobs = await k.sliceAnalyse(library, name, family, chosen, by === "depth" ? Number(depth) : null, by === "time" ? Number(seconds) : null, Number(top));
    } catch (e) {
      error = k.why(e);
    }
  }
</script>

<section class="panel analyse" aria-labelledby="analyse-title">
  <h3 id="analyse-title">{t("background.title", { name })}</h3>
  <p class="hint">{t("background.intro")}</p>
  {#if !engines.length}
    <p class="hint">{t("background.no_engines")}</p>
  {/if}
  <fieldset>
    <legend>{t("background.engines")}</legend>
    {#each engines as e (e.key)}
      <div class="row">
        <label class="check"><input type="checkbox" bind:checked={picked[e.key]} /> {e.name}</label>
        <input type="text" bind:value={min[e.key]} placeholder={t("background.min_hint")} aria-label={t("background.min", { engine: e.name })} disabled={!picked[e.key]} size="14" />
      </div>
    {/each}
    {#if more}<p class="hint">{t("background.more")}</p>{/if}
  </fieldset>
  <div class="row">
    <label class="check"><input type="radio" bind:group={by} value="depth" /> {t("background.depth")}</label>
    <input type="number" min="1" max="99" bind:value={depth} disabled={by !== "depth"} aria-label={t("background.depth")} />
    <label class="check"><input type="radio" bind:group={by} value="time" /> {t("background.seconds")}</label>
    <input type="number" min="0.1" step="0.1" bind:value={seconds} disabled={by !== "time"} aria-label={t("background.seconds")} />
  </div>
  <label class="row">{t("background.top")} <input type="number" min="1" max="100000" bind:value={top} /></label>
  <div class="row">
    <button class="primary" onclick={queue} disabled={!engines.some((e) => picked[e.key])}>{t("background.queue")}</button>
    <button onclick={onclose}>{t("common.close")}</button>
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if jobs.length}
    <p role="status">{t("background.queued")}</p>
    {#each jobs as id (id)}<JobLine {id} />{/each}
  {/if}
</section>

<style>
  .analyse {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  h3 {
    font-size: 1rem;
    margin: 0;
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }
  input[type="number"] {
    width: 6rem;
    font: inherit;
  }
  .hint {
    color: var(--dim);
    font-size: 0.9em;
    margin: 0;
  }
  .error {
    color: var(--red);
  }
</style>
