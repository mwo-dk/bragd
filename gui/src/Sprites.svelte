<script>
  // Every sprite sheet, inlined once into the page: a board draws a piece as <use href="#id">.
  // The ids carry their sheet's name, so none clash (assets/pieces/README.md).
  // A gradient inside a display:none <svg> does not paint, so the sheets are hidden by size.
  const sheets = Object.values(
    import.meta.glob("./lib/sprites/*.svg", { query: "?raw", import: "default", eager: true }),
  ).map((s) => String(s).replace('style="display:none"', 'width="0" height="0"'));
</script>

<div class="sprites" aria-hidden="true">
  {#each sheets as sheet}
    {@html sheet}
  {/each}
</div>

<style>
  .sprites {
    position: absolute;
    width: 0;
    height: 0;
    overflow: hidden;
  }
</style>
