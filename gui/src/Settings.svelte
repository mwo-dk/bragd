<script>
  // Settings, built from bragd-core's option descriptors: each option's area, label, hint, value
  // and choices come from Rust, and each change is saved to config.toml there.
  import { t, tn } from "./i18n.svelte.js";
  import Picker from "./Picker.svelte";
  import { guide } from "./guide.svelte.js";
  import FamilyToggles from "./FamilyToggles.svelte";
  import SettingsExtras from "./SettingsExtras.svelte";
  import DiskUse from "./DiskUse.svelte";
  import LanguageModel from "./LanguageModel.svelte";

  /** @type {{ snap: any, save: (name: string, value: any) => Promise<any> }} */
  let { snap, save } = $props();
  let area = $state("overview");
  // A link elsewhere (the wizard, a notice) names the area to open.
  $effect(() => {
    if (guide.area) [area, guide.area] = [guide.area, null];
  });
  // An area Rust no longer lists (its last Family turned off) gives way to Overview.
  $effect(() => {
    if (!snap.areas.some((/** @type {any} */ a) => a[0] === area)) area = "overview";
  });

  const opts = $derived(snap.settings.filter((/** @type {any} */ o) => o.listed && o.area === area));
  const byName = $derived(new Map(snap.settings.map((/** @type {any} */ o) => [o.name, o])));
  const shown = $derived(snap.families.filter((/** @type {any} */ f) => f.shown));
  const variantsOn = $derived(shown.reduce((/** @type {number} */ n, /** @type {any} */ f) => n + f.variants.filter((/** @type {any} */ v) => v.on).length, 0));
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
        <p>{t("settings.overview", { version: snap.version, families: tn("settings.families_on", shown.length), variants: tn("settings.variants_on", variantsOn) })}</p>
        <SettingsExtras />
      {/if}

      {#if area === "model"}
        <LanguageModel {snap} {save} />
      {/if}

      {#if area === "families"}
        <FamilyToggles {snap} {save} />
      {/if}

      {#each opts as o (o.name)}
        <div class="row">
          {#if typeof o.value === "boolean"}
            <label><input type="checkbox" checked={o.value} onchange={(e) => save(o.name, e.currentTarget.checked)} /> {o.label}</label>
          {:else}
            <label for="opt-{o.name}">{o.label}</label>
            {#if o.choices.length}
              <Picker id="opt-{o.name}" value={o.value} choices={o.choices} label={o.label} onchange={(v) => save(o.name, v)} />
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
      <!-- Disk watch's limits (disk_total, disk_free_below and each item's) are set here. -->
      {#if area === "knowledge"}<DiskUse />{/if}
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
