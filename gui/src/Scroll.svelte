<script>
  // A box that scrolls sideways, as a stop for the keyboard: a scrolling area with nothing
  // focusable inside cannot be reached otherwise. axe wants a role for a stop in the focus order,
  // and a region needs a name: `by` is the id of the heading (or tab) it sits under.
  let { by, children } = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="scroll" tabindex="0" role="region" aria-labelledby={by}>{@render children()}</div>

<style>
  .scroll {
    overflow-x: auto;
    margin-bottom: 0.5rem;
  }
</style>
