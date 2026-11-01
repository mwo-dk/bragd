<script>
  // Settings → Language model: the servers found running on this machine, the address and model
  // (saved as options), the key (kept in the keychain by Rust) and, for an address off this
  // machine, the consent asked once. Rust checks the address and lists the models.
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";

  /** @type {{ snap: any, save: (name: string, value: any) => Promise<any> }} */
  let { snap, save } = $props();
  /** @type {any} */
  let st = $state(null);
  /** @type {any[] | null} */
  let found = $state(null);
  /** @type {string[]} */
  let models = $state([]);
  let key = $state("");
  /** The model picked for each server found, by its address. @type {Record<string, string>} */
  let pick = $state({});
  let error = $state("");

  const fail = (/** @type {any} */ e) => (error = String(e));
  $effect(() => {
    void snap;
    api.modelStatus().then((s) => (st = s), fail);
  });
  const look = () =>
    ((found = null),
    api.modelFind().then((f) => {
      found = f;
      // Each server's model in use, else its first.
      for (const x of f) pick[x.url] = st?.url === x.url && x.models.includes(st.model) ? st.model : x.models[0];
    }, fail));
  look();

  const use = (/** @type {string} */ url, /** @type {string} */ model) => ((error = ""), save("model_url", url).then(() => save("model_name", model)).catch(fail));
  const list = () => ((error = ""), api.modelModels(st.url).then((m) => (models = m), fail));
  const saveKey = (/** @type {string} */ k) => ((error = ""), api.modelKey(k).then((s) => ((st = s), (key = "")), fail));
</script>

<div class="model">
  <p>{t("model.intro")}</p>

  <h3>{t("model.found")}</h3>
  {#if found === null}
    <p class="hint">{t("model.looking")}</p>
  {:else if !found.length}
    <p class="hint">{t("model.none_found")}</p>
  {/if}
  {#each found ?? [] as f (f.url)}
    <div class="row">
      <label for="found-{f.url}">{f.name} <small>{f.url}</small></label>
      <div class="line">
        <select id="found-{f.url}" bind:value={pick[f.url]}>
          {#each f.models as m}<option value={m}>{m}</option>{/each}
        </select>
        <button onclick={() => use(f.url, pick[f.url])} disabled={!pick[f.url]}>{t("model.use")}</button>
      </div>
    </div>
  {/each}
  <button onclick={look}>{t("model.look_again")}</button>

  {#if st}
    <h3>{t("model.address")}</h3>
    <div class="row">
      <label for="model-url">{t("setting.model_url")}</label>
      <input id="model-url" type="url" value={st.url} placeholder="https://api.example.com/v1" onchange={(e) => save("model_url", e.currentTarget.value.trim()).catch(fail)} />
      <p class="hint">{t("setting.model_url.hint")}</p>
    </div>
    {#if st.url}
      <div class="row">
        <label for="model-name">{t("setting.model_name")}</label>
        <div class="line">
          <input id="model-name" type="text" list="model-list" value={st.model} onchange={(e) => save("model_name", e.currentTarget.value.trim()).catch(fail)} />
          <datalist id="model-list">{#each models as m}<option value={m}></option>{/each}</datalist>
          <button onclick={list}>{t("model.list")}</button>
        </div>
        <p class="hint">{t("setting.model_name.hint")}</p>
      </div>
      <div class="row">
        <label for="model-key">{t("model.key")}</label>
        <div class="line">
          <input id="model-key" type="password" autocomplete="off" bind:value={key} placeholder={st.keySaved ? t("model.key_saved") : ""} />
          <button onclick={() => saveKey(key)} disabled={!key.trim()}>{t("model.key_save")}</button>
          {#if st.keySaved}<button onclick={() => saveKey("")}>{t("model.key_remove")}</button>{/if}
        </div>
        <p class="hint">{t("model.key_hint")}</p>
      </div>
      {#if st.consentNeeded}
        <div class="consent" role="alert">
          <p>{t("model.consent", { url: st.url })}</p>
          <button onclick={() => save("model_consent", st.url).catch(fail)}>{t("model.agree")}</button>
        </div>
      {:else if !st.onThisMachine}
        <p class="hint">{t("model.leaves", { url: st.url })}</p>
      {/if}
      <p class="state">{st.ready ? t("model.ready", { model: st.model }) : t("model.not_ready")}</p>
      <button onclick={() => save("model_url", "").catch(fail)}>{t("model.turn_off")}</button>
    {:else}
      <p class="state">{t("model.off")}</p>
    {/if}
  {/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
</div>

<style>
  .model {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    max-width: 32rem;
    min-width: 0;
    align-items: flex-start;
    overflow-wrap: anywhere;
  }
  .model > * {
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
  }
  .line input,
  .line select {
    flex: 1 1 10rem;
    min-width: 0;
    /* A long model name must not widen the page: WebKit sizes a select by its longest option. */
    width: 100%;
  }
  small,
  .hint {
    color: var(--dim);
  }
  .hint {
    margin: 0;
    font-size: 0.9em;
  }
  .consent {
    border: 1px solid var(--accent, currentColor);
    border-radius: var(--r-sm);
    padding: 0.6rem 0.8rem;
    align-self: stretch;
  }
  .consent p {
    margin: 0 0 0.5rem;
  }
  .state {
    font-weight: 600;
  }
  .err {
    color: var(--red);
  }
</style>
