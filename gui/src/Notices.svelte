<script>
  // The notice area: notices (shown once), hints (a few times) and the update notice, with its
  // upgrade command to copy. Which ones show, and their texts, come from bragd-core.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import { guide } from "./guide.svelte.js";

  /** @type {{ snap: any, go: (page: any) => void }} */
  let { snap, go } = $props();
  let copied = $state("");

  const areaName = (/** @type {string} */ id) => snap.areas.find((/** @type {any} */ a) => a[0] === id)?.[1] ?? id;
  function dismiss(/** @type {string} */ id) {
    guide.notices = guide.notices.filter((n) => n.id !== id);
    if (!id.startsWith("local:")) api.noticeDismiss(id);
  }
  function settings(/** @type {any} */ n) {
    guide.area = n.settings;
    go({ kind: "settings" });
  }
  async function copy(/** @type {any} */ n) {
    await navigator.clipboard?.writeText(n.copy).catch(() => {});
    copied = n.id;
  }
</script>

{#if guide.notices.length && !guide.wizard && !guide.cards.length}
  <aside class="notices" aria-live="polite">
    {#each guide.notices as n (n.id)}
      <div class="notice panel" class:hintish={n.id.startsWith("hint:")}>
        <p>{n.text}</p>
        {#if n.copy}
          <p class="copy"><code>{n.copy}</code> <button onclick={() => copy(n)}>{copied === n.id ? t("common.copied") : t("common.copy")}</button></p>
        {/if}
        <div class="actions">
          {#if n.settings}<button class="link" onclick={() => settings(n)}>{t("notice.open_settings", { area: areaName(n.settings) })}</button>{/if}
          <button onclick={() => dismiss(n.id)} aria-label={t("common.close")} title={t("common.close")}>×</button>
        </div>
      </div>
    {/each}
  </aside>
{/if}

<style>
  .notices {
    position: fixed;
    inset-block-end: 0.8rem;
    inset-inline-end: 0.8rem;
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: min(24rem, calc(100vw - 1.6rem));
  }
  .notice {
    box-shadow: var(--shadow);
    padding: 0.6rem 0.8rem;
  }
  .hintish {
    border-inline-start: 3px solid var(--accent);
  }
  p {
    margin: 0 0 0.4rem;
  }
  .copy {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }
  code {
    font-family: var(--mono-font);
    user-select: all;
    background: var(--alt);
    padding: 0.1rem 0.4rem;
    border-radius: var(--r-sm);
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.4rem;
  }
  .link {
    border: none;
    background: none;
    color: var(--accent);
    text-decoration: underline;
  }
</style>
