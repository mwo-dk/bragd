<script>
  // A Call with a Friend during a Game on the Bragd server. Rust decides who may call and holds
  // each channel's consent and the signalling; this page only carries the media with the
  // webview's own WebRTC, while Rust says a channel is open for both. Nothing is recorded.
  import { onDestroy } from "svelte";
  import * as api from "./api.js";
  import { t } from "./i18n.svelte.js";

  /** @type {{ call: any }} */
  let { call } = $props();

  /** @type {RTCPeerConnection | null} */
  let pc = null;
  /** @type {ReturnType<typeof setInterval> | undefined} */
  let timer;
  let makingOffer = false;
  let mediaError = $state("");
  /** @type {HTMLVideoElement | undefined} */
  let remoteVideo = $state();
  /** @type {HTMLVideoElement | undefined} */
  let localVideo = $state();
  const local = new MediaStream();

  const send = (/** @type {any} */ signal) => api.friendsCallSignal(signal).catch(() => {});

  function setup() {
    pc = new RTCPeerConnection({ iceServers: call.iceServers ?? [] });
    const remote = new MediaStream();
    pc.ontrack = (e) => {
      remote.addTrack(e.track);
      if (remoteVideo) remoteVideo.srcObject = remote;
    };
    pc.onicecandidate = (e) => e.candidate && send({ kind: "ice", candidate: e.candidate.toJSON() });
    pc.onnegotiationneeded = async () => {
      if (!pc) return;
      try {
        makingOffer = true;
        await pc.setLocalDescription();
        if (pc.localDescription) send({ kind: "sdp", description: pc.localDescription.toJSON() });
      } finally {
        makingOffer = false;
      }
    };
    timer = setInterval(pump, 300);
  }

  /** The other machine's offers, answers and candidates, by WebRTC's "perfect negotiation". */
  async function pump() {
    if (!pc) return;
    const polite = !call.offerer;
    for (const s of await api.friendsCallTake()) {
      if (!pc) return;
      try {
        if (s.kind === "sdp") {
          const d = s.description;
          const collision = d.type === "offer" && (makingOffer || pc.signalingState !== "stable");
          if (!polite && collision) continue;
          await pc.setRemoteDescription(d);
          if (d.type === "offer") {
            await pc.setLocalDescription();
            if (pc.localDescription) send({ kind: "sdp", description: pc.localDescription.toJSON() });
          }
        } else if (s.kind === "ice") {
          await pc.addIceCandidate(s.candidate);
        }
      } catch {
        // A candidate for an offer that was ignored; the next negotiation carries on.
      }
    }
  }

  /** Opens or stops the microphone and the camera to match what is open for both. */
  async function tracks(/** @type {boolean} */ voice, /** @type {boolean} */ camera) {
    if (!pc) return;
    for (const [kind, want] of /** @type {[string, boolean][]} */ ([["audio", voice], ["video", camera]])) {
      const sender = pc.getSenders().find((x) => x.track?.kind === kind);
      if (want && !sender) {
        try {
          const media = await navigator.mediaDevices.getUserMedia({ [kind]: true });
          for (const track of media.getTracks()) {
            local.addTrack(track);
            pc?.addTrack(track, local);
          }
          if (localVideo) localVideo.srcObject = local;
          mediaError = "";
        } catch (e) {
          mediaError = t("talk.media_error", { why: String(e) });
        }
      } else if (!want && sender?.track) {
        sender.track.stop();
        local.removeTrack(sender.track);
        pc.removeTrack(sender);
      }
    }
  }

  function teardown() {
    clearInterval(timer);
    for (const track of local.getTracks()) {
      track.stop();
      local.removeTrack(track);
    }
    pc?.close();
    pc = null;
  }

  $effect(() => {
    const voice = call.voice.open;
    const camera = call.camera.open;
    if ((voice || camera) && !pc) setup();
    if (!voice && !camera && pc) teardown();
    tracks(voice, camera);
  });
  onDestroy(teardown);

  const want = (/** @type {string} */ channel, /** @type {boolean} */ on) => api.friendsCallWant(channel, on).catch((e) => (mediaError = String(e)));
</script>

<h2>{t("talk.call_title", { name: call.opponent })}</h2>
{#if call.error}<p class="error" role="alert">{call.error}</p>{/if}
{#if mediaError}<p class="error" role="alert">{mediaError}</p>{/if}
{#each [["voice", call.voice], ["camera", call.camera]] as [channel, c] (channel)}
  <div class="offer">
    {#if c.open}
      <span class="live" role="status">● {t(channel === "voice" ? "talk.mic_live" : "talk.camera_live")}</span>
      <button onclick={() => want(channel, false)}>{t(`talk.stop_${channel}`)}</button>
    {:else if c.asked}
      <span role="status">{t(`talk.asks_${channel}`, { name: call.opponent })}</span>
      <button class="primary" onclick={() => want(channel, true)}>{t("talk.allow")}</button>
      <button onclick={() => want(channel, false)}>{t("talk.not_now")}</button>
    {:else if c.waiting}
      <span class="hint" role="status">{t(`talk.waiting_${channel}`, { name: call.opponent })}</span>
      <button class="link" onclick={() => want(channel, false)}>{t("common.cancel")}</button>
    {:else}
      <button onclick={() => want(channel, true)} disabled={!call.connected}>{t(`talk.ask_${channel}`)}</button>
    {/if}
  </div>
{/each}
<div class="tiles" class:shown={call.voice.open || call.camera.open}>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={remoteVideo} autoplay playsinline class:hidden={!call.camera.open}></video>
  <!-- The user's own picture, without their own sound. -->
  <video bind:this={localVideo} autoplay playsinline muted class="self" class:hidden={!call.camera.open}></video>
</div>
<p class="hint">{t("talk.no_recording")}</p>

<style>
  h2 {
    font-size: 1rem;
    margin: 0.6rem 0 0.3rem;
  }
  .offer {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
    margin: 0.4rem 0;
  }
  .live {
    color: var(--red);
    font-weight: 600;
  }
  .hint {
    color: var(--dim);
  }
  .error {
    color: var(--red);
  }
  .tiles {
    display: none;
    gap: 0.4rem;
  }
  .tiles.shown {
    display: flex;
    flex-wrap: wrap;
  }
  video {
    width: 100%;
    max-width: 20rem;
    border-radius: 6px;
    background: #000;
  }
  video.self {
    max-width: 8rem;
  }
  video.hidden {
    display: none;
  }
</style>
