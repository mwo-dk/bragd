<script>
  // Settings → Peers: the switch and the interface (saved as options, then the Helper is told),
  // the code this machine shows, pairing with another machine, and the machines paired. The
  // Helper does the pairing and the talking (bragd-peers); this page only draws its view.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";

  /** @type {{ snap: any, save: (name: string, value: any) => Promise<any> }} */
  let { snap, save } = $props();
  /** @type {any} */
  let v = $state(null);
  /** @type {any[] | null} */
  let found = $state(null);
  let looking = $state(false);
  let addr = $state("");
  let code = $state("");
  let busy = $state(false);
  let error = $state("");
  let done = $state("");

  const fail = (/** @type {any} */ e) => ((error = String(e)), (busy = false));
  // A view not asking the Peers keeps what the last one that did heard.
  const got = (/** @type {any} */ x) => {
    for (const p of x.peers) p.online ??= v?.peers.find((/** @type {any} */ q) => q.key === p.key)?.online ?? null;
    [v, busy] = [x, false];
  };
  $effect(() => {
    void snap;
    api.peersView(true).then(got, fail);
  });
  // While a code shows, its time and tries count down, and a pairing shows up.
  $effect(() => {
    if (!v?.code) return;
    const tick = setInterval(() => api.peersView(false).then(got, fail), 2000);
    return () => clearInterval(tick);
  });

  const setting = (/** @type {string} */ name, /** @type {any} */ value) => (
    (error = ""), save(name, value).then(() => api.peersApply()).then(got, fail)
  );
  const show = () => ((error = ""), api.pairShow().then(got, fail));
  const hide = () => api.pairHide().then(got, fail);
  const look = () => (
    (looking = true),
    api.peersLook().then(
      (f) => ((found = f), (looking = false)),
      (e) => ((looking = false), fail(e)),
    )
  );
  const pair = () => {
    const before = new Set(v.peers.map((/** @type {any} */ p) => p.key));
    [error, done, busy] = ["", "", true];
    api.pairWith(addr.trim(), code).then((x) => {
      got(x);
      const p = x.peers.find((/** @type {any} */ p) => !before.has(p.key));
      done = t("peers.done", { name: p?.name ?? addr });
      code = "";
    }, fail);
  };
  const unpair = (/** @type {string} */ id) => ((error = ""), api.peerUnpair(id).then(got, fail));
  const check = () => ((busy = true), api.peersView(true).then(got, fail));
  const when = (/** @type {number} */ s) => new Date(s * 1000).toLocaleString();
  const paired = (/** @type {string} */ key) => v?.peers.some((/** @type {any} */ p) => p.key === key);
</script>

<div class="peers">
  <p>{t("peers.intro")}</p>
  {#if v}
    <div class="row">
      <label><input type="checkbox" checked={v.on} onchange={(e) => setting("peers_on", e.currentTarget.checked)} /> {t("setting.peers_on")}</label>
      <p class="hint">{t("setting.peers_on.hint")}</p>
    </div>
    {#if v.on}
      <div class="row">
        <label for="peers-interface">{t("setting.peers_interface")}</label>
        <Picker
          id="peers-interface"
          label={t("setting.peers_interface")}
          value={snap.settings.find((/** @type {any} */ o) => o.name === "peers_interface")?.value ?? ""}
          choices={[{ value: "", label: t("peers.lan") }, ...v.interfaces.map((/** @type {any} */ i) => ({ value: i.name, label: i.name, hint: i.addrs.join(", ") }))]}
          onchange={(x) => setting("peers_interface", x)}
        />
        <p class="hint">{t("setting.peers_interface.hint")}</p>
      </div>
    {/if}
    <p class="state">
      {#if !v.on}{t("peers.off")}
      {:else if v.error}{v.error}
      {:else}{t("peers.listening", { name: v.name, addrs: v.listening.join(", ") })}{/if}
    </p>

    {#if v.listening.length}
      <h3>{t("peers.show_title")}</h3>
      {#if v.code}
        <p class="code" aria-live="polite">{v.code[0].slice(0, 3)} {v.code[0].slice(3)}</p>
        <p class="hint">{t("peers.code_left", { secs: v.code[1], tries: v.code[2] })}</p>
        <button onclick={hide}>{t("peers.hide_code")}</button>
      {:else}
        <p class="hint">{t("peers.show_hint")}</p>
        <button onclick={show}>{t("peers.show_code")}</button>
      {/if}

      <h3>{t("peers.pair_title")}</h3>
      <button onclick={look} disabled={looking}>{looking ? t("peers.looking") : t("peers.look")}</button>
      {#if found && !found.length}<p class="hint">{t("peers.none_found")}</p>{/if}
      {#each found ?? [] as f (f.key)}
        <div class="line found">
          <span>{f.name} <small>{f.addr}</small></span>
          {#if paired(f.key)}<small>{t("peers.already")}</small>
          {:else}<button onclick={() => (addr = f.addr)}>{t("peers.use")}</button>{/if}
        </div>
      {/each}
      <div class="row">
        <label for="peer-addr">{t("peers.address")}</label>
        <input id="peer-addr" type="text" bind:value={addr} placeholder="laptop.tailnet.ts.net" autocomplete="off" />
        <p class="hint">{t("peers.address_hint")}</p>
      </div>
      <div class="row">
        <label for="peer-code">{t("peers.code")}</label>
        <div class="line">
          <input id="peer-code" type="text" inputmode="numeric" maxlength="7" bind:value={code} autocomplete="off" />
          <button onclick={pair} disabled={busy || !addr.trim() || code.replace(/\s/g, "").length !== 6}>{t("peers.pair")}</button>
        </div>
      </div>
      {#if done}<p class="ok" role="status">{done}</p>{/if}
    {/if}

    <h3>{t("peers.paired")}</h3>
    {#if !v.peers.length}<p class="hint">{t("peers.none")}</p>{/if}
    {#each v.peers as p (p.key)}
      <div class="peer">
        <div>
          <strong>{p.name}</strong> <small>{p.addr}</small>
          <p class="hint">
            {p.online === true ? t("peers.online") : p.online === false ? t("peers.offline") : ""}
            {#if p.version}{t("peers.version", { version: p.version })}{/if}
            {#if p.seen}{t("peers.seen", { when: when(p.seen) })}{/if}
          </p>
        </div>
        <button onclick={() => unpair(p.key)}>{t("peers.unpair")}</button>
      </div>
    {/each}
    {#if v.peers.length}<button onclick={check} disabled={busy}>{t("peers.check")}</button>{/if}
  {/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
</div>

<style>
  .peers {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    max-width: 32rem;
    min-width: 0;
    align-items: flex-start;
    overflow-wrap: anywhere;
  }
  .peers > * {
    margin: 0;
  }
  h3 {
    font-size: 1rem;
    margin-top: 0.6rem;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    align-self: stretch;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }
  .line input {
    flex: 1 1 10rem;
    min-width: 0;
    width: 100%;
  }
  .found,
  .peer {
    align-self: stretch;
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  .peer {
    border-top: 1px solid var(--border);
    padding-top: 0.4rem;
  }
  .peer p {
    margin: 0;
  }
  small,
  .hint {
    color: var(--dim);
  }
  .hint {
    margin: 0;
    font-size: 0.9em;
  }
  .code {
    font-size: 2rem;
    font-weight: 700;
    letter-spacing: 0.15em;
    font-variant-numeric: tabular-nums;
  }
  .state {
    font-weight: 600;
  }
  .ok {
    color: var(--green);
  }
  .err {
    color: var(--red);
  }
</style>
