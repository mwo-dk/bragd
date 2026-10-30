<script>
  // The Game family and Variant switches, as Settings → Game families and the install wizard
  // show them: each saved to config.toml by Rust.
  /** @type {{ snap: any, save: (name: string, value: any) => void }} */
  let { snap, save } = $props();
  const byName = $derived(new Map(snap.settings.map((/** @type {any} */ o) => [o.name, o])));
</script>

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

<style>
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
</style>
