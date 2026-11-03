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
  import { here, go as visit, replace, back } from "./nav.svelte.js";
  import { wantsBack } from "./back.js";
  import Databases from "./Databases.svelte";
  import Jobs from "./Jobs.svelte";
  import * as knowledge from "./knowledge.js";
  import Engines from "./Engines.svelte";
  import Matches from "./Matches.svelte";
  import Tablebases from "./Tablebases.svelte";
  import Books from "./Books.svelte";
  import Clubs from "./Clubs.svelte";
  import PlayOnline from "./PlayOnline.svelte";
  import Friends from "./Friends.svelte";
  import splash from "../../assets/brand/splash.svg";

  /** @type {any} */
  let snap = $state(null);
  /** Where the window is: the page, and the tab and item open on it (nav.svelte.js). */
  const page = $derived(here.view);
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
  }

  // A page whose Family is off (just turned off, or reached by Back) gives way to Settings.
  $effect(() => {
    if (snap && (page.kind === "item" || page.kind === "online") && !snap.families.find((/** @type {any} */ f) => f.id === page.family)?.shown) replace({ kind: "settings" });
  });
  // Play with Friends without a Bragd server address set gives way too.
  $effect(() => {
    if (snap && page.kind === "friends" && !snap.friends) replace({ kind: "settings" });
  });

  api.snapshot().then((s) => {
    apply(s);
    const first = s.families.find((/** @type {any} */ f) => f.shown);
    if (first) replace({ kind: "item", family: first.id, item: "play" });
    // The wizard on a first start, the tour cards after an upgrade; then notices and hints,
    // and the update check (which may take a few seconds).
    api.guideDue().then((d) => ([guide.wizard, guide.first, guide.cards] = [d.wizard, d.wizard, d.cards]));
    api.noticeList(true).then((n) => {
      guide.notices = n;
      api.updateCheck().then((all) => (guide.notices = [...all, ...guide.notices.filter((x) => /^(hint|local):/.test(x.id))]));
      askTip();
    });
  });

  // A Tip at start and after a while without input; Rust decides whether one is due.
  const IDLE = 2 * 60 * 1000;
  const askTip = () => {
    // A Tip waits while a notice or hint is on screen: it is counted as shown when asked for.
    if (!guide.tip && !guide.notices.length) api.tipNext(guide.wizard || guide.cards.length > 0 || guide.live).then((tip) => (guide.tip = tip));
  };
  let idle = setTimeout(askTip, IDLE);
  const active = () => {
    clearTimeout(idle);
    idle = setTimeout(askTip, IDLE);
  };

  /** Slices by Game family, for the sidebar. @type {Record<string, any[]>} */
  let slices = $state({});
  const loadSlices = () => knowledge.slices().then((s) => (slices = s), () => {});
  loadSlices();

  const save = (/** @type {string} */ name, /** @type {any} */ value) => api.saveSetting(name, value).then(apply);

  function go(/** @type {any} */ p) {
    visit(p);
    if (window.innerWidth < 760) collapsed = true;
  }

  /** The wizard, tour cards or a dialog is open: they keep their own keys. */
  const overlay = () => guide.wizard || guide.cards.length > 0 || !!document.querySelector("dialog[open], .veil");
  /** Typing in a field (not a search) is input Back must not drop without asking. */
  const typed = (/** @type {Event} */ e) => {
    const el = /** @type {HTMLInputElement} */ (e.target);
    if (el.type !== "search" && !el.closest("[role=search]")) here.dirty = true;
  };

  /** Online play is on a server, for one of its Families: the one whose Play opened it. */
  const onlineFamily = $derived(page.kind === "online" ? snap?.families.find((/** @type {any} */ f) => f.id === page.family) : null);
  const family = $derived(page.kind === "item" ? snap?.families.find((/** @type {any} */ f) => f.id === page.family) : null);
</script>

<svelte:window
  onkeydown={(e) => {
    active();
    if (e.key === "F1") e.preventDefault(), page.kind !== "help" && go({ kind: "help" });
    else if (wantsBack(e, page.kind, overlay()) && here.back) e.preventDefault(), back();
  }}
  onmouseup={(e) => wantsBack(e, page.kind, overlay()) && here.back && (e.preventDefault(), back())}
  onpointerdown={active}
/>

<Sprites />

{#if splashing}
  <div class="splash" class:still={reduced}>
    <img src={splash} alt="Bragd ᛒᚱᛅᚴᚦ" />
  </div>
{/if}

{#if snap}
  <!-- Under the splash nothing scrolls: WebKitGTK draws a scrollbar above a fixed layer. -->
  <div class="shell" class:behind={splashing}>
    <Sidebar {snap} {page} {collapsed} {go} {slices} toggle={() => (collapsed = !collapsed)} />
    <div class="column">
      <!-- Back shows only when there is somewhere to go back to (nav.svelte.js). -->
      {#if here.back}
        <header class="top">
          <button class="back" onclick={back} title={t("nav.back_hint")}>← {t("nav.back")} <kbd>{page.kind === "help" ? "Esc" : "Alt+←"}</kbd></button>
        </header>
      {/if}
      <!-- Focusable, so the keyboard can scroll a page that has nothing else to focus. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <main class="page" tabindex="0" oninput={typed} onsubmit={() => (here.dirty = false)}>
      {#if page.kind === "settings"}
        <Settings {snap} {save} />
      {:else if page.kind === "help"}
        <Help />
      {:else if page.kind === "whats_new"}
        <WhatsNew />
      {:else if page.kind === "clubs"}
        <Clubs {snap} {go} />
      {:else if page.kind === "friends" && snap.friends}
        <Friends {save} />
      {:else if page.kind === "online" && onlineFamily?.shown}
        {#key page.server}
          <PlayOnline server={page.server} family={onlineFamily} {save} />
        {/key}
      {:else if family && page.item === "play"}
        {#key family.id}
          <Play {family} {go} />
        {/key}
      {:else if page.kind === "jobs"}
        <Jobs />
      {:else if family && page.item === "databases"}
        {#key family.id}
          <Databases {family} slice={page.slice} onslices={loadSlices} onsnap={apply} />
        {/key}
      {:else if family && page.item === "rules"}
        <Rules {family} />
      {:else if family && page.item === "analyse"}
        {#key family.id}
          <Play {family} mode="analyse" {go} />
        {/key}
      {:else if family && page.item === "engines"}
        {#key family.id}
          <Engines {family} {go} />
        {/key}
      {:else if family && page.item === "matches"}
        {#key family.id}
          <Matches {family} {go} />
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
  </div>
  {#if guide.wizard || guide.cards.length}
    <Wizard {snap} {save} only={(id) => api.familiesOnly(id).then(apply)} {go} />
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
  .behind :global(nav),
  .behind .page {
    overflow: hidden;
  }
  .column {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .top {
    display: flex;
    padding: 0.3rem 0.8rem;
    border-bottom: 1px solid var(--border);
  }
  .back {
    font: inherit;
    color: inherit;
    background: none;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    padding: 0.2rem 0.6rem;
    cursor: pointer;
  }
  .back:hover {
    background: color-mix(in srgb, var(--sel-bg) 35%, transparent);
  }
  kbd {
    font-size: 0.75em;
    color: var(--dim);
    margin-inline-start: 0.3rem;
  }
  .page {
    flex: 1;
    min-width: 0;
    min-height: 0;
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
