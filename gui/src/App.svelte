<script>
  // The shell: the splash, the sidebar and the page it opens. Texts, colours and the sidebar all
  // come in one snapshot from Rust, and come again after every saved setting.
  import * as api from "./api.js";
  import { setLanguage, t } from "./i18n.svelte.js";
  import Sprites from "./Sprites.svelte";
  import Sidebar from "./Sidebar.svelte";
  import Settings from "./Settings.svelte";
  import Play from "./Play.svelte";
  import Rules from "./Rules.svelte";
  import Help from "./Help.svelte";
  import WhatsNew from "./WhatsNew.svelte";
  import Wizard from "./Wizard.svelte";
  import Notices from "./Notices.svelte";
  import { guide } from "./guide.svelte.js";
  import Databases from "./Databases.svelte";
  import Jobs from "./Jobs.svelte";
  import * as knowledge from "./knowledge.js";
  import Engines from "./Engines.svelte";
  import Tablebases from "./Tablebases.svelte";
  import Books from "./Books.svelte";
  import splash from "../../assets/brand/splash.svg";

  /** @type {any} */
  let snap = $state(null);
  /** @type {any} */
  let page = $state({ kind: "settings" });
  let collapsed = $state(window.innerWidth < 760);
  let splashing = $state(true);

  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  // At most about a second, and without the fade when motion is reduced.
  setTimeout(() => (splashing = false), reduced ? 600 : 900);

  function apply(/** @type {any} */ s) {
    snap = s;
    setLanguage(s);
    const root = document.documentElement;
    root.dataset.look = s.appearance.look;
    root.toggleAttribute("data-crt", s.appearance.crt);
    root.toggleAttribute("data-dark", s.appearance.dark);
    root.style.colorScheme = s.appearance.dark ? "dark" : "light";
    for (const [name, value] of s.appearance.vars) root.style.setProperty(name, value);
    // A page whose Family was just turned off goes back to Settings.
    if (page.kind === "item" && !s.families.find((/** @type {any} */ f) => f.id === page.family)?.shown) page = { kind: "settings" };
  }

  api.snapshot().then((s) => {
    apply(s);
    const first = s.families.find((/** @type {any} */ f) => f.shown);
    if (first) page = { kind: "item", family: first.id, item: "play" };
    // The wizard on a first start, the tour cards after an upgrade; then notices and hints,
    // and the update check (which may take a few seconds).
    api.guideDue().then((d) => ([guide.wizard, guide.cards] = [d.wizard, d.cards]));
    api.noticeList(true).then((n) => {
      guide.notices = n;
      api.updateCheck().then((all) => (guide.notices = [...all, ...guide.notices.filter((x) => /^(hint|local):/.test(x.id))]));
    });
  });

  /** Slices by Game family, for the sidebar. @type {Record<string, any[]>} */
  let slices = $state({});
  const loadSlices = () => knowledge.slices().then((s) => (slices = s), () => {});
  loadSlices();

  const save = (/** @type {string} */ name, /** @type {any} */ value) => api.saveSetting(name, value).then(apply);

  function go(/** @type {any} */ p) {
    page = p;
    if (window.innerWidth < 760) collapsed = true;
  }

  const family = $derived(page.kind === "item" ? snap?.families.find((/** @type {any} */ f) => f.id === page.family) : null);
</script>

<svelte:window onkeydown={(e) => e.key === "F1" && (e.preventDefault(), go({ kind: "help" }))} />

<Sprites />

{#if splashing}
  <div class="splash" class:still={reduced}>
    <img src={splash} alt="Bragd ᛒᚱᛅᚴᚦ" />
  </div>
{/if}

{#if snap}
  <div class="shell">
    <Sidebar {snap} {page} {collapsed} {go} {slices} toggle={() => (collapsed = !collapsed)} />
    <main class="page">
      {#if page.kind === "settings"}
        <Settings {snap} {save} />
      {:else if page.kind === "help"}
        <Help />
      {:else if page.kind === "whats_new"}
        <WhatsNew />
      {:else if family && page.item === "play"}
        {#key family.id}
          <Play {family} />
        {/key}
      {:else if page.kind === "jobs"}
        <Jobs />
      {:else if family && page.item === "databases"}
        {#key family.id}
          <Databases {family} slice={page.slice} onslices={loadSlices} />
        {/key}
      {:else if family && page.item === "rules"}
        <Rules {family} />
      {:else if family && page.item === "analyse"}
        {#key family.id}
          <Play {family} mode="analyse" />
        {/key}
      {:else if family && page.item === "engines"}
        {#key family.id}
          <Engines {family} />
        {/key}
      {:else if family && page.item === "tablebases"}
        <Tablebases {snap} {save} />
      {:else if family && page.item === "opening_books"}
        <Books />
      {:else if family}
        <section>
          <h1>{family.name}: {t(`sidebar.${page.item}`)}</h1>
          <p>{t("page.later")}</p>
        </section>
      {/if}
    </main>
  </div>
  {#if guide.wizard || guide.cards.length}
    <Wizard {snap} {save} {go} />
  {/if}
  <Notices {snap} {go} />
{/if}

<style>
  .shell {
    position: relative;
    display: flex;
    height: 100vh;
    overflow: hidden;
  }
  .page {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 1rem 1.2rem;
    box-sizing: border-box;
  }
  .splash {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: #00301b;
    animation: fade 0.9s ease-in forwards;
  }
  .splash.still {
    animation: none;
  }
  .splash img {
    width: min(90vw, 960px);
    height: auto;
  }
  @keyframes fade {
    0%,
    70% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }
</style>
