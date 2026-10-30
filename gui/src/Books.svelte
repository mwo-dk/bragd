<script>
  // Chess's Opening books item: the Polyglot books whose moves Play and Analyse show. Rust
  // checks that a file is a book and reads it.
  import * as api from "./api.js";
  import { t, num } from "./i18n.svelte.js";

  /** @type {any[]} */
  let books = $state([]);
  let path = $state("");
  let error = $state("");

  const load = () => api.booksPage().then((b) => (books = b));
  load();
  async function run(/** @type {Promise<any>} */ p) {
    try {
      await p;
      error = "";
      path = "";
    } catch (e) {
      error = String(e);
    }
    await load();
  }
</script>

<section class="books">
  <h1>{t("sidebar.opening_books")}</h1>
  <p>{t("books.intro")}</p>
  {#each books as b (b.path)}
    <article class="panel">
      <strong>{b.path}</strong>
      {#if b.error}
        <p class="error">{b.error}</p>
      {:else}
        <p>{t("books.entries", { n: num(b.entries) })}</p>
        <p class="dim">{t("books.first", { moves: b.first.join(", ") })}</p>
      {/if}
      <button onclick={() => run(api.bookRemove(b.path))}>{t("engines.remove")}</button>
    </article>
  {:else}
    <p class="dim">{t("books.none")}</p>
  {/each}
  <form class="row" onsubmit={(e) => (e.preventDefault(), run(api.bookAdd(path)))}>
    <input type="text" bind:value={path} placeholder={t("books.path")} aria-label={t("books.path")} />
    <button class="primary" disabled={!path.trim()}>{t("books.add")}</button>
  </form>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .books {
    max-width: 52rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  p {
    margin: 0.3rem 0;
  }
  .row {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }
  .row input {
    flex: 1;
    min-width: 12rem;
  }
  .dim {
    color: var(--dim);
  }
  .error {
    color: var(--red);
  }
</style>
