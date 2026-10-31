<script>
  // Settings → Overview: what opens outside Bragd (config.toml in the system's editor, a
  // translation issue in the browser) and the install wizard again. Rust opens them.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import { guide } from "./guide.svelte.js";

  let shown = $state("");
  let error = $state("");
  const run = (/** @type {Promise<any>} */ p) => ((error = ""), p.catch((e) => (error = String(e))));
</script>

<div class="extras">
  <div class="row">
    <button onclick={() => (guide.wizard = true)}>{t("settings.run_wizard")}</button>
    <button onclick={() => run(api.openConfig())}>{t("settings.open_config")}</button>
  </div>
  <div class="row">
    <input type="text" bind:value={shown} placeholder={t("settings.suggest_which")} aria-label={t("settings.suggest_which")} />
    <button onclick={() => run(api.suggestTranslation(shown))}>{t("settings.suggest_translation")}</button>
  </div>
  <p class="hint">{t("settings.suggest_translation_hint")}</p>
  {#if error}<p class="err" role="alert">{error}</p>{/if}
</div>

<style>
  .extras {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    max-width: 32rem;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  input {
    flex: 1;
    min-width: 12rem;
  }
  .hint {
    margin: 0;
    color: var(--dim);
    font-size: 0.9em;
  }
  .err {
    margin: 0;
    color: var(--red);
  }
</style>
