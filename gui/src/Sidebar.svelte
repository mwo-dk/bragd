<script>
  // The sidebar, as bragd-core works it out from what is on: the Families as outer items when two
  // or more are on, one Family's items at the top when only it is, and always Settings, Help and
  // What's new.
  import { t } from "./i18n.svelte.js";
  // The rune wordmark: outlines from Noto Sans Runic, drawn in the sidebar's gold.
  import wordmark from "../../assets/brand/wordmark-runes.svg?raw";
  const runes = wordmark.replaceAll("#004225", "currentColor");

  /** @type {{ snap: any, page: any, collapsed: boolean, go: (page: any) => void, toggle: () => void }} */
  let { snap, page, collapsed, go, toggle } = $props();
  /** Families folded open. @type {string[]} */
  let open = $state([]);

  const name = (/** @type {string} */ id) => snap.families.find((/** @type {any} */ f) => f.id === id)?.name ?? id;
  const families = $derived(
    snap.sidebar.kind === "families" ? snap.sidebar.families : snap.sidebar.kind === "one" ? [snap.sidebar.family] : [],
  );
  const isCurrent = (/** @type {string} */ family, /** @type {string} */ item) =>
    page.kind === "item" && page.family === family && page.item === item;
  $effect(() => {
    if (page.kind === "item" && !open.includes(page.family)) open = [...open, page.family];
  });
</script>

<nav class="sidebar" class:collapsed aria-label="Bragd">
  <div class="brand">
    <span class="runes">{@html runes}</span>
    {#if !collapsed}<span class="word">Bragd</span>{/if}
  </div>
  {#if !collapsed}
    <div class="items">
      {#each families as f (f.id)}
        {#if snap.sidebar.kind === "families"}
          <button
            class="family"
            aria-expanded={open.includes(f.id)}
            onclick={() => (open = open.includes(f.id) ? open.filter((x) => x !== f.id) : [...open, f.id])}
          >
            {name(f.id)}
          </button>
        {/if}
        {#if snap.sidebar.kind === "one" || open.includes(f.id)}
          {#each f.items as item}
            <button
              class="item"
              class:nested={snap.sidebar.kind === "families"}
              class:current={isCurrent(f.id, item)}
              onclick={() => go({ kind: "item", family: f.id, item })}
            >
              {t(`sidebar.${item}`)}
            </button>
          {/each}
        {/if}
      {/each}
    </div>
    <div class="always">
      <button class:current={page.kind === "settings"} onclick={() => go({ kind: "settings" })}>{t("sidebar.settings")}</button>
      <button class:current={page.kind === "help"} onclick={() => go({ kind: "help" })}>{t("sidebar.help")} <kbd>F1</kbd></button>
      <button class:current={page.kind === "whats_new"} onclick={() => go({ kind: "whats_new" })}>{t("sidebar.whats_new")}</button>
    </div>
  {/if}
  <button class="fold" onclick={toggle} title={collapsed ? t("sidebar.expand") : t("sidebar.collapse")} aria-label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}>
    {collapsed ? "»" : "«"}
  </button>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 14rem;
    flex: 0 0 auto;
    background: var(--alt);
    border-inline-end: 1px solid var(--border);
    overflow: hidden auto;
  }
  .sidebar.collapsed {
    width: 3.2rem;
  }
  .brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 0.8rem 0.4rem 0.6rem;
    background: var(--brand);
    color: #d8b74a;
  }
  .runes :global(svg) {
    display: block;
    width: 8rem;
    height: auto;
  }
  .collapsed .runes :global(svg) {
    width: 2.6rem;
  }
  .word {
    color: var(--brand-pale);
    font-size: 0.85rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
  }
  .items,
  .always {
    display: flex;
    flex-direction: column;
    padding: 0.4rem;
  }
  .items {
    flex: 1;
  }
  .always {
    border-top: 1px solid var(--border);
  }
  button {
    text-align: start;
    background: none;
    border: none;
    color: inherit;
    padding: 0.4rem 0.6rem;
    border-radius: var(--r-sm);
    cursor: pointer;
    font: inherit;
  }
  button:hover {
    background: color-mix(in srgb, var(--sel-bg) 35%, transparent);
  }
  button.current {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }
  .family {
    font-weight: 600;
  }
  .nested {
    padding-inline-start: 1.5rem;
  }
  kbd {
    float: inline-end;
    font-size: 0.75em;
    color: var(--dim);
  }
  .fold {
    margin: 0.4rem;
    text-align: center;
  }
  @media (max-width: 760px) {
    .sidebar:not(.collapsed) {
      position: absolute;
      z-index: 10;
      inset-block: 0;
      inset-inline-start: 0;
      box-shadow: var(--shadow);
    }
  }
</style>
