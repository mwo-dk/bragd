<script>
  // A Strip rule's fields (bragd_library::strip::StripRule), edited in place. Every part set must
  // agree for a Game to be kept; a part left empty keeps everything.
  import { t } from "./i18n.svelte.js";

  /** @type {{ rule: any, page: any, disabled?: boolean }} */
  let { rule, page, disabled = false } = $props();

  const list = (/** @type {string} */ s) => s.split(",").map((x) => x.trim()).filter(Boolean);
  function toggle(/** @type {string} */ field, /** @type {any} */ value, /** @type {boolean} */ on) {
    rule[field] = on ? [...rule[field], value] : rule[field].filter((/** @type {any} */ v) => v !== value);
  }
</script>

<fieldset class="strip" {disabled}>
  <legend>{t("strip.title")}</legend>
  <p class="hint">{t("strip.hint")}</p>
  <div class="row">
    <label>{t("strip.min_rating")} <input type="number" min="0" step="50" bind:value={rule.min_rating} /></label>
    <label
      >{t("strip.rating_of_label")}
      <select bind:value={rule.rating_of}>
        {#each page.ratingOf as p}<option value={p.value}>{p.label}</option>{/each}
      </select>
    </label>
    <label>{t("strip.min_plies")} <input type="number" min="0" bind:value={rule.min_plies} /></label>
  </div>
  <div class="row">
    <span class="label">{t("strip.speeds")}</span>
    {#each page.speeds as p}
      <label class="check"><input type="checkbox" checked={rule.speeds.includes(p.value)} onchange={(e) => toggle("speeds", p.value, e.currentTarget.checked)} /> {p.label}</label>
    {/each}
  </div>
  <div class="row">
    <span class="label">{t("strip.results")}</span>
    {#each page.results as p}
      <label class="check"><input type="checkbox" checked={rule.results.includes(p.value)} onchange={(e) => toggle("results", p.value, e.currentTarget.checked)} /> {p.label}</label>
    {/each}
  </div>
  <div class="row">
    <label class="check"><input type="checkbox" bind:checked={rule.rated_only} /> {t("strip.rated_only")}</label>
    <label class="check"><input type="checkbox" bind:checked={rule.titled_only} /> {t("strip.titled_only")}</label>
  </div>
  <div class="row">
    <label>{t("strip.date_from")} <input type="text" bind:value={rule.date_from} placeholder="2024.01" size="10" /></label>
    <label>{t("strip.date_to")} <input type="text" bind:value={rule.date_to} placeholder="2024.12.31" size="10" /></label>
    <label
      >{t("strip.variants")}
      <input type="text" value={rule.variants.join(", ")} onchange={(e) => (rule.variants = list(e.currentTarget.value))} placeholder="standard, chess960" />
    </label>
  </div>
  <label class="wide"
    >{t("strip.players")}
    <input type="text" value={rule.players.join(", ")} onchange={(e) => (rule.players = list(e.currentTarget.value))} placeholder={t("strip.players_hint")} />
  </label>
</fieldset>

<style>
  .strip {
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 0.4rem 0.8rem 0.7rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  legend {
    font-weight: 600;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1rem;
    align-items: flex-end;
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
    gap: 0.3rem;
  }
  .label {
    font-size: 0.9em;
    font-weight: 600;
  }
  .wide input {
    width: 100%;
    box-sizing: border-box;
  }
  input[type="number"] {
    width: 6rem;
    font: inherit;
  }
  .hint {
    color: var(--dim);
    font-size: 0.85em;
    margin: 0;
  }
</style>
