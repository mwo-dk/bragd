<script>
  // F1 → Features: every feature with what it does and the questions people ask, searched in
  // Rust (case and accents aside). "Show me" opens its docs page in the browser.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";

  let query = $state("");
  /** @type {any[]} */
  let features = $state([]);
  $effect(() => {
    api.featureList(query).then((f) => (features = f));
  });
</script>

<section class="help">
  <h1>{t("features.title")}</h1>
  <input type="search" bind:value={query} placeholder={t("features.filter")} aria-label={t("features.filter")} />
  {#each features as f (f.id)}
    <article class="panel">
      <h2>{f.title}</h2>
      <p>{f.what}</p>
      <dl>
        {#each f.qa as [q, a]}
          <dt>{q}</dt>
          <dd>{a}</dd>
        {/each}
      </dl>
      <button class="link" onclick={() => api.openUrl(f.url)}>{t("features.show_me")}</button>
    </article>
  {:else}
    <p>{t("features.none")}</p>
  {/each}
</section>

<style>
  .help {
    max-width: 48rem;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }
  h2 {
    margin: 0 0 0.3rem;
    font-size: 1.1rem;
  }
  p {
    margin: 0 0 0.5rem;
  }
  input {
    font: inherit;
    color: var(--fg);
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 0.3rem 0.7rem;
  }
  dt {
    font-weight: 600;
  }
  dd {
    margin: 0 0 0.5rem;
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }
</style>
