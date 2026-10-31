<script>
  // What's new in the sidebar: every version, newest first, with its tour cards (bragd-core tour.rs).
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";

  /** @type {[string, any[]][]} */
  let versions = $state([]);
  api.whatsNew().then((v) => (versions = v));
</script>

<section class="news">
  <h1>{t("sidebar.whats_new")}</h1>
  {#each versions as [version, cards] (version)}
    <h2>{t("whats_new.version", { version })}</h2>
    {#each cards as c (c.id)}
      <article class="panel">
        <h3>{c.title}</h3>
        <p>{c.text}</p>
      </article>
    {/each}
  {/each}
</section>

<style>
  .news {
    max-width: 48rem;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }
  h2 {
    margin: 0.4rem 0 0;
    font-size: 1.15rem;
  }
  h3 {
    margin: 0 0 0.3rem;
    font-size: 1.05rem;
  }
  p {
    margin: 0;
  }
</style>
