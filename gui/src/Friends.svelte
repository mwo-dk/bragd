<script>
  // Play with Friends: the account on the Bragd server, Friends, Clubs, and Games played by
  // Bragd's rules (Play online's Game, on the server `bragd`). Rust keeps the account and asks
  // the server; this page only draws what it is handed.
  import { onDestroy } from "svelte";
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";
  import PlayOnline from "./PlayOnline.svelte";

  /** @type {{ save: (name: string, value: any) => Promise<any> }} */
  let { save } = $props();

  /** @type {any} */
  let view = $state(null);
  let error = $state("");
  let busy = $state(false);
  let email = $state("");
  let password = $state("");
  let name = $state("");
  let ask = $state("");
  let code = $state("");
  let clubName = $state("");
  let clubCode = $state("");
  /** The player being reported, and why. */
  let reporting = $state("");
  let reason = $state("");
  /** Deleting the account: the form is open, the name typed, and the user's choice for their Games. */
  let deleting = $state(false);
  let confirmName = $state("");
  let keepGames = $state(false);

  const refresh = () =>
    api.friendsView().then(
      (v) => (view = v),
      (e) => (error = String(e)),
    );
  refresh();
  const timer = setInterval(refresh, 1000);
  onDestroy(() => clearInterval(timer));

  /** Runs a command, showing its refusal. */
  async function run(/** @type {() => Promise<any>} */ f) {
    busy = true;
    try {
      const r = await f();
      if (r) view = r;
      error = "";
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }
  const act = (/** @type {string} */ what, /** @type {any} */ body = {}) => run(() => api.friendsAct(what, body));
  const signOut = () => run(() => api.onlineSignOut("bragd").then(() => api.friendsView()));
  const deleteAccount = () => run(() => api.friendsDeleteAccount(confirmName, keepGames)).then(() => (deleting = view?.signedIn ?? false));
  const report = () => act("reports", { name: reporting, reason }).then(() => ((reporting = ""), (reason = "")));
</script>

<section class="friends">
  <header class="bar">
    <h1>{t("friends.title")}</h1>
    {#if view?.signedIn && view.name}
      <span class="who">{t("friends.signed_in_as", { name: view.name })}</span>
      <button onclick={signOut} disabled={busy}>{t("online.sign_out")}</button>
    {/if}
  </header>

  {#if error || view?.error}
    <p class="error" role="alert">{error || view.error}</p>
  {/if}
  {#if view?.notice}
    <p class="notice" role="status">{view.notice}</p>
  {/if}

  {#if view && !view.signedIn}
    <div class="panel">
      <p>{t("friends.intro")}</p>
      {#if view.signingIn}
        <p class="hint" role="status">{t("friends.signing_in")}</p>
        <button onclick={() => api.onlineCancelSignIn("bragd")}>{t("common.cancel")}</button>
      {:else}
        <form class="form" onsubmit={(e) => (e.preventDefault(), run(() => api.friendsSignIn(email, password)))}>
          <label class="field">{t("friends.email")} <input type="email" bind:value={email} autocomplete="username" /></label>
          <label class="field">{t("friends.password")} <input type="password" bind:value={password} autocomplete="current-password" /></label>
          <button class="primary" type="submit" disabled={busy || !email.trim() || !password}>{t("friends.sign_in")}</button>
          <button type="button" onclick={() => run(() => api.friendsSignUp(email, password))} disabled={busy || !email.trim() || !password}>{t("friends.sign_up")}</button>
        </form>
        {#if view.providers.length}
          <div class="form">
            {#each view.providers as [id, label]}
              <button onclick={() => run(() => api.friendsSignInWith(id))} disabled={busy}>{t("friends.sign_in_with", { provider: label })}</button>
            {/each}
          </div>
        {/if}
      {/if}
      <p class="hint">{t("friends.sign_in_hint")}</p>
    </div>
  {:else if view && !view.name}
    <form class="panel form" onsubmit={(e) => (e.preventDefault(), run(() => api.friendsName(name)))}>
      <label class="field">{t("friends.choose_name")} <input bind:value={name} maxlength="30" autocomplete="nickname" /></label>
      <button class="primary" type="submit" disabled={busy || name.trim().length < 2}>{t("friends.save_name")}</button>
    </form>
  {:else if view}
    <div class="columns">
      <div class="panel">
        <h2>{t("friends.friends")}</h2>
        {#if !view.friends.length}<p class="hint">{t("friends.no_friends")}</p>{/if}
        <ul class="list">
          {#each view.friends as f (f.name)}
            <li>
              <span><strong>{f.name}</strong>{#if f.club} <small class="hint">· {t("friends.from_club")}</small>{/if}</span>
              {#if !f.club}<button onclick={() => act("friends/remove", { name: f.name })} disabled={busy}>{t("friends.remove")}</button>{/if}
              <button onclick={() => act("blocks", { name: f.name })} disabled={busy}>{t("friends.block")}</button>
              <button onclick={() => (reporting = f.name)} disabled={busy}>{t("friends.report")}</button>
            </li>
          {/each}
        </ul>
        {#if view.asked.length}
          <h3>{t("friends.asked")}</h3>
          <ul class="list">
            {#each view.asked as n (n)}
              <li>
                <strong>{n}</strong>
                <button class="primary" onclick={() => act("friends", { name: n })} disabled={busy}>{t("friends.accept")}</button>
                <button onclick={() => act("friends/remove", { name: n })} disabled={busy}>{t("friends.decline")}</button>
                <button onclick={() => act("blocks", { name: n })} disabled={busy}>{t("friends.block")}</button>
                <button onclick={() => (reporting = n)} disabled={busy}>{t("friends.report")}</button>
              </li>
            {/each}
          </ul>
        {/if}
        {#if view.asking.length}
          <h3>{t("friends.asking")}</h3>
          <ul class="list">
            {#each view.asking as n (n)}
              <li><strong>{n}</strong> <button onclick={() => act("friends/remove", { name: n })} disabled={busy}>{t("friends.take_back")}</button></li>
            {/each}
          </ul>
        {/if}
        {#if reporting}
          <form class="report" onsubmit={(e) => (e.preventDefault(), report())}>
            <label class="field">{t("friends.report_reason")} <textarea bind:value={reason} maxlength="2000" rows="3"></textarea></label>
            <button class="primary" type="submit" disabled={busy || !reason.trim()}>{t("friends.send_report")}</button>
            <button type="button" onclick={() => (reporting = "")}>{t("common.cancel")}</button>
          </form>
        {/if}
        <form class="form" onsubmit={(e) => (e.preventDefault(), act("friends", { name: ask }).then(() => (ask = "")))}>
          <label class="field">{t("friends.add")} <input bind:value={ask} autocomplete="off" spellcheck="false" /></label>
          <button type="submit" disabled={busy || !ask.trim()}>{t("friends.ask")}</button>
        </form>
        <div class="form">
          <button onclick={() => act("invites")} disabled={busy}>{t("friends.invite")}</button>
        </div>
        <form class="form" onsubmit={(e) => (e.preventDefault(), act("invites/redeem", { code }).then(() => (code = "")))}>
          <label class="field">{t("friends.redeem")} <input bind:value={code} autocomplete="off" spellcheck="false" /></label>
          <button type="submit" disabled={busy || !code.trim()}>{t("friends.redeem_button")}</button>
        </form>
        {#if view.blocked.length}
          <h3>{t("friends.blocked")}</h3>
          <ul class="list">
            {#each view.blocked as n (n)}
              <li>
                <strong>{n}</strong>
                <button onclick={() => act("blocks/remove", { name: n })} disabled={busy}>{t("friends.unblock")}</button>
                <button onclick={() => (reporting = n)} disabled={busy}>{t("friends.report")}</button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="panel">
        <h2>{t("friends.clubs")}</h2>
        <label class="check"><input type="checkbox" checked={view.clubFriends} onchange={(e) => run(() => api.friendsClubFriends(e.currentTarget.checked))} disabled={busy} /> {t("friends.club_friends")}</label>
        <p class="hint">{t("friends.club_friends.hint")}</p>
        <ul class="list">
          {#each view.clubs as c (c.id)}
            <li>
              <span><strong>{c.name}</strong> <small class="hint">· {t("friends.members", { n: c.members })}</small>{#if c.code} <code>{c.code}</code>{/if}</span>
              <button onclick={() => act("clubs/leave", { id: c.id })} disabled={busy}>{t("friends.leave")}</button>
            </li>
          {/each}
        </ul>
        <form class="form" onsubmit={(e) => (e.preventDefault(), act("clubs", { name: clubName }).then(() => (clubName = "")))}>
          <label class="field">{t("friends.new_club")} <input bind:value={clubName} maxlength="80" /></label>
          <button type="submit" disabled={busy || !clubName.trim()}>{t("friends.make_club")}</button>
        </form>
        <form class="form" onsubmit={(e) => (e.preventDefault(), act("clubs/join", { code: clubCode }).then(() => (clubCode = "")))}>
          <label class="field">{t("friends.join_code")} <input bind:value={clubCode} autocomplete="off" spellcheck="false" /></label>
          <button type="submit" disabled={busy || !clubCode.trim()}>{t("friends.join")}</button>
        </form>
      </div>
    </div>

    <h2>{t("friends.games")}</h2>
    <PlayOnline server="bragd" family={null} {save} embedded />

    {#if !deleting}
      <div class="form"><button onclick={() => ((deleting = true), (confirmName = ""))} disabled={busy}>{t("friends.delete_account")}</button></div>
    {:else}
      <form class="panel delete" onsubmit={(e) => (e.preventDefault(), deleteAccount())}>
        <h2>{t("friends.delete_account")}</h2>
        <p>{t("friends.delete_account.what")}</p>
        <label class="check"><input type="checkbox" bind:checked={keepGames} /> {t("friends.delete_account.keep_games")}</label>
        <label class="field">{t("friends.delete_account.confirm", { name: view.name })} <input bind:value={confirmName} autocomplete="off" spellcheck="false" /></label>
        <div class="form">
          <button class="primary" type="submit" disabled={busy || confirmName.trim() !== view.name}>{t("friends.delete_account.button")}</button>
          <button type="button" onclick={() => (deleting = false)}>{t("common.cancel")}</button>
        </div>
      </form>
    {/if}
  {/if}
</section>

<style>
  .friends {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
    align-items: baseline;
  }
  h1 {
    margin: 0;
    font-size: 1.4rem;
  }
  h2 {
    font-size: 1.1rem;
    margin: 0.4rem 0 0.3rem;
  }
  h3 {
    font-size: 0.95rem;
    margin: 0.8rem 0 0.3rem;
  }
  .who,
  .hint {
    color: var(--dim);
  }
  .error {
    color: var(--red);
  }
  .columns {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
    gap: 1rem;
    align-items: start;
  }
  .form,
  .report {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
    margin: 0.4rem 0;
  }
  .field,
  .check {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    align-items: center;
  }
  .field input {
    width: 12rem;
    max-width: 100%;
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .list li {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
    padding: 0.3rem 0;
    border-bottom: 1px solid var(--border);
  }
  .list li > span:first-child,
  .list li > strong:first-child {
    flex: 1 1 8rem;
  }
  code {
    overflow-wrap: anywhere;
  }
  .delete {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
    border-left: 3px solid var(--red);
    padding-left: 0.6rem;
  }
  .delete p,
  .delete h2,
  .delete .form {
    margin: 0;
  }
  .delete .check {
    flex-wrap: nowrap;
    align-items: flex-start;
  }
</style>
