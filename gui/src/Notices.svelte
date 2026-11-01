<script>
  // The notice area: notices (shown once), hints (a few times) and the update notice, with its
  // upgrade command to copy. Which ones show, and their texts, come from bragd-core. One at a
  // time, notices before hints, a Tip only when neither waits: closing one shows the next.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import { guide } from "./guide.svelte.js";

  /** @type {{ snap: any, go: (page: any) => void }} */
  let { snap, go } = $props();
  let copied = $state("");

  const areaName = (/** @type {string} */ id) => snap.areas.find((/** @type {any} */ a) => a[0] === id)?.[1] ?? id;
  const shown = $derived([...guide.notices.filter((n) => !n.id.startsWith("hint:")), ...guide.notices.filter((n) => n.id.startsWith("hint:"))][0]);
  async function dismiss(/** @type {string} */ id) {
    guide.notices = guide.notices.filter((n) => n.id !== id);
    if (id.startsWith("local:")) return;
    // Closing a hint brings the next one due.
    const next = await api.noticeDismiss(id);
    if (next && !guide.notices.some((n) => n.id === next.id)) guide.notices = [...guide.notices, next];
  }
  function settings(/** @type {any} */ n) {
    guide.area = n.settings;
    go({ kind: "settings" });
  }
  /** *Show me*: the Settings area, a page of its own (`page:clubs`), Play online on a server (`online:ogs:go`), or the sidebar item of the first Family shown that has it. */
  function showMe(/** @type {any} */ tip) {
    guide.tip = null;
    const [kind, what, family] = tip.open.split(":");
    if (kind === "online") return go({ kind: "online", server: what, family });
    if (kind === "settings") {
      guide.area = what;
      return go({ kind: "settings" });
    }
    if (kind === "page") return go({ kind: what });
    const sb = snap.sidebar;
    const families = sb.kind === "families" ? sb.families : sb.kind === "one" ? [sb.family] : [];
    const f = families.find((/** @type {any} */ f) => f.items.includes(what));
    go(f ? { kind: "item", family: f.id, item: what } : { kind: "settings" });
  }
  function never(/** @type {any} */ tip) {
    guide.tip = null;
    api.tipSilence(tip.id);
  }
  async function copy(/** @type {any} */ n) {
    await navigator.clipboard?.writeText(n.copy).catch(() => {});
    copied = n.id;
  }
</script>

{#if (shown || guide.tip) && !guide.wizard && !guide.cards.length}
  <aside class="notices" aria-live="polite">
    {#if !shown && guide.tip}
      {@const tip = guide.tip}
      <div class="notice panel hintish tip">
        <p><b>{t("tip.label")}:</b> {tip.text}</p>
        <div class="actions">
          <button class="primary" onclick={() => showMe(tip)}>{t("tip.show_me")}</button>
          <button onclick={() => (guide.tip = null)}>{t("tip.not_now")}</button>
          <button class="link" onclick={() => never(tip)}>{t("tip.never")}</button>
        </div>
      </div>
    {/if}
    {#if shown}
      {@const n = shown}
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
    {/if}
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
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 0.4rem;
  }
  .link {
    border: none;
    background: none;
    color: var(--accent);
    text-decoration: underline;
  }
</style>
