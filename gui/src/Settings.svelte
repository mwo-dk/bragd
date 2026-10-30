<script>
  // Settings, built from bragd-core's option descriptors: each option's area, label, hint, value
  // and choices come from Rust, and each change is saved to config.toml there.
  import { t } from "./i18n.svelte.js";

  /** @type {{ snap: any, save: (name: string, value: any) => void }} */
  let { snap, save } = $props();
  let area = $state("overview");

  const opts = $derived(snap.settings.filter((/** @type {any} */ o) => o.area === area && o.area !== "families"));
  const byName = $derived(new Map(snap.settings.map((/** @type {any} */ o) => [o.name, o])));
  const shown = $derived(snap.families.filter((/** @type {any} */ f) => f.shown));
  const variantsOn = $derived(shown.reduce((/** @type {number} */ n, /** @type {any} */ f) => n + f.variants.filter((/** @type {any} */ v) => v.on).length, 0));

  /** Choices under their headings, for <optgroup>. */
  function groups(/** @type {any[]} */ choices) {
    /** @type {[string, any[]][]} */
    const out = [];
    for (const c of choices) {
      const last = out[out.length - 1];
      if (last && last[0] === c.group) last[1].push(c);
      else out.push([c.group, [c]]);
    }
    return out;
  }
</script>

<section class="settings">
  <h1>{t("settings.title")}</h1>
  <div class="frame">
    <nav class="areas">
      {#each snap.areas as [id, label]}
        <button class:current={area === id} onclick={() => (area = id)}>{label}</button>
      {/each}
    </nav>

    <section>
      {#if area === "overview"}
        <p>{t("settings.overview", { version: snap.version, families: shown.length, variants: variantsOn })}</p>
      {/if}

      {#if area === "families"}
        {#each snap.families as f}
          {@const fo = byName.get(`family_${f.id}`)}
          <fieldset class="family">
            <legend>
              <label><input type="checkbox" checked={fo.value} onchange={(e) => save(fo.name, e.currentTarget.checked)} /> {f.name}</label>
            </legend>
            {#each f.variants as v}
              {@const vo = byName.get(`variant_${v.key}`)}
              <label class="variant">
                <input type="checkbox" checked={vo.value} disabled={!fo.value} onchange={(e) => save(vo.name, e.currentTarget.checked)} />
                {v.name}
              </label>
            {/each}
          </fieldset>
        {/each}
      {/if}

      {#each opts as o (o.name)}
        <div class="row">
          {#if typeof o.value === "boolean"}
            <label><input type="checkbox" checked={o.value} onchange={(e) => save(o.name, e.currentTarget.checked)} /> {o.label}</label>
          {:else}
            <label for="opt-{o.name}">{o.label}</label>
            {#if o.choices.length}
              <select id="opt-{o.name}" value={o.value} onchange={(e) => save(o.name, e.currentTarget.value)}>
                {#each groups(o.choices) as [group, list]}
                  {#if group}
                    <optgroup label={group}>
                      {#each list as c}<option value={c.value}>{c.label}</option>{/each}
                    </optgroup>
                  {:else}
                    {#each list as c}<option value={c.value}>{c.label}</option>{/each}
                  {/if}
                {/each}
              </select>
            {:else if Array.isArray(o.value)}
              <input
                id="opt-{o.name}"
                type="text"
                value={o.value.join(", ")}
                onchange={(e) => save(o.name, e.currentTarget.value.split(",").map((s) => s.trim()).filter(Boolean))}
              />
            {:else}
              <input id="opt-{o.name}" type="text" value={o.value} onchange={(e) => save(o.name, e.currentTarget.value)} />
            {/if}
          {/if}
          {#if o.hint}<p class="hint">{o.hint}</p>{/if}
        </div>
      {/each}
    </section>
  </div>
</section>

<style>
  .frame {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }
  .areas {
    display: flex;
    flex-direction: column;
    min-width: 11rem;
  }
  .areas button {
    text-align: start;
    background: none;
    border: none;
    padding: 0.45rem 0.7rem;
    border-radius: var(--r-sm);
    color: inherit;
    cursor: pointer;
  }
  .areas button.current {
    background: var(--sel-bg);
    color: var(--sel-fg);
  }

  .frame > section {
    flex: 1;
    min-width: 0;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 1rem;
    max-width: 32rem;
  }
  .hint {
    margin: 0;
    color: var(--dim);
    font-size: 0.9em;
  }
  .family {
    border: 1px solid var(--border);
    border-radius: var(--r);
    margin: 0 0 0.6rem;
    padding: 0.3rem 0.8rem 0.6rem;
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem 1rem;
  }
  .family legend {
    font-weight: 600;
  }
  @media (max-width: 760px) {
    .frame {
      flex-direction: column;
    }
    .areas {
      flex-direction: row;
      flex-wrap: wrap;
      min-width: 0;
    }
  }
</style>
