<script>
  // The Helper's jobs: every job it keeps, newest first, with its progress, and Cancel for those
  // queued or running. Also the disk limits crossed, and why the Helper is out of reach.
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";

  /** @type {any} */
  let s = $state(null);

  async function load() {
    try {
      s = await k.helperStatus();
    } catch (e) {
      s = { on: false, error: k.why(e), jobs: [], notices: [] };
    }
  }

  $effect(() => {
    let stop = false;
    const tick = async () => {
      await load();
      if (!stop) setTimeout(tick, 1500);
    };
    tick();
    return () => (stop = true);
  });

  async function retry() {
    s = await k.helperRetry();
  }
</script>

<section class="jobs">
  <h1>{t("jobs.title")}</h1>
  {#if s}
    <p>{s.line}</p>
    {#if !s.on}
      <p class="error" role="alert">{s.error}</p>
      <button onclick={retry}>{t("helper.retry")}</button>
    {/if}
    {#each s.notices as n}
      <p class="notice" role="alert">{n}</p>
    {/each}
    {#if s.on && !s.jobs.length}
      <p class="dim">{t("jobs.none")}</p>
    {/if}
    {#each s.jobs as j (j.id)}
      <article class="panel job">
        <div class="head">
          <strong>{j.what}</strong>
          <span class="status {j.status}">{j.statusText}</span>
          {#if j.live}<button onclick={() => k.jobCancel(j.id).then(load)}>{t("common.cancel")}</button>{/if}
        </div>
        {#if j.live}
          <progress max="1" value={j.progress ?? undefined}></progress>
        {/if}
        <small>{j.count}</small>
        {#if j.report}<p>{j.report}</p>{/if}
        {#if j.error}<p class="error">{j.error}</p>{/if}
      </article>
    {/each}
  {:else}
    <p class="dim">{t("helper.connecting")}</p>
  {/if}
</section>

<style>
  .jobs {
    max-width: 48rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .job {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
    align-items: center;
  }
  .head strong {
    flex: 1;
  }
  .status.failed,
  .error {
    color: var(--red);
  }
  progress {
    width: 100%;
  }
  p {
    margin: 0;
  }
  small,
  .dim {
    color: var(--dim);
  }
  .notice {
    padding: 0.4rem 0.6rem;
    border-inline-start: 3px solid var(--red);
    background: var(--alt);
  }
</style>
