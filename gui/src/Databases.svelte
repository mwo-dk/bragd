<script>
  // A Family's Databases: Foreign databases read in place, the Libraries with their Slices,
  // the Sources and Disk use. The position of the Game last stepped through in replay is kept
  // here, for a Slice's "Use current board position".
  import { t } from "./i18n.svelte.js";
  import ForeignDb from "./ForeignDb.svelte";
  import Libraries from "./Libraries.svelte";
  import Sources from "./Sources.svelte";
  import DiskUse from "./DiskUse.svelte";

  /** @type {{ family: any, slice?: any, onslices: () => void, onsnap: (snap: any) => void }} */
  let { family, slice = null, onslices, onsnap } = $props();
  // svelte-ignore state_referenced_locally
  let tab = $state(slice ? "libraries" : "files");
  /** @type {{family: string, fen: string} | null} */
  let position = $state(null);

  $effect(() => {
    if (slice) tab = "libraries";
  });

  const onstep = (/** @type {string} */ fam, /** @type {any} */ step) => (position = step?.fen ? { family: fam, fen: step.fen } : position);
  const TABS = ["files", "libraries", "sources", "disk"];
</script>

<section class="databases">
  <h1>{family.name}: {t("sidebar.databases")}</h1>
  <div class="tabs" role="tablist">
    {#each TABS as id}
      <button role="tab" aria-selected={tab === id} class:current={tab === id} onclick={() => (tab = id)}>{t(`db.tab.${id}`)}</button>
    {/each}
  </div>
  {#if tab === "files"}
    <ForeignDb {onstep} {onsnap} />
  {:else if tab === "libraries"}
    <Libraries family={family.id} variants={family.variants} {slice} {position} {onstep} {onslices} />
  {:else if tab === "sources"}
    <Sources family={family.id} />
  {:else}
    <DiskUse />
  {/if}
</section>

<style>
  .databases {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    min-height: 100%;
  }
  h1 {
    margin: 0;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.4rem;
  }
  .tabs button.current {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }
</style>
