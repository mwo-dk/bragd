<script>
  // One Helper job as a line: what it does, how far, and what it did. It asks again every second
  // until the job has ended. Used wherever a button starts a job.
  import * as k from "./knowledge.js";
  import { t } from "./i18n.svelte.js";

  /** @type {{ id: number, ondone?: (job: any) => void }} */
  let { id, ondone } = $props();
  /** @type {any} */
  let job = $state(null);
  let error = $state("");

  $effect(() => {
    const want = id;
    let stop = false;
    const tick = async () => {
      try {
        const s = await k.helperStatus();
        job = s.jobs.find((/** @type {any} */ j) => j.id === want) ?? null;
        error = s.error;
        if (job && !job.live) return ondone?.(job);
      } catch (e) {
        error = k.why(e);
      }
      if (!stop) setTimeout(tick, 1000);
    };
    tick();
    return () => (stop = true);
  });
</script>

<div class="job" role="status">
  {#if job}
    <div class="head">
      <span>{job.what}: {job.statusText}</span>
      {#if job.live}<button onclick={() => k.jobCancel(job.id)}>{t("common.cancel")}</button>{/if}
    </div>
    {#if job.live}
      <progress max="1" value={job.progress ?? undefined}></progress>
      <small>{job.count}</small>
    {/if}
    {#if job.report}<p>{job.report}</p>{/if}
    {#if job.error}<p class="error">{job.error}</p>{/if}
  {:else if error}
    <p class="error">{error}</p>
  {:else}
    <small>{t("job.waiting")}</small>
  {/if}
</div>

<style>
  .job {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.4rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    margin: 0.4rem 0;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  progress {
    width: 100%;
  }
  p {
    margin: 0;
  }
  small {
    color: var(--dim);
  }
  .error {
    color: var(--red);
  }
</style>
