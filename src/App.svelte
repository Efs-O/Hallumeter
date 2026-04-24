<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-shell";
  import { exit } from "@tauri-apps/plugin-process";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { stateToColor, stateToMessage } from "./lib/risk";
  import type { RiskState } from "./lib/risk";

  interface ContextPayload {
    fill_pct:   number;
    risk_score: number;
    state:      string;
    model:      string;
    session:    string;
    variant:    number; // 1–5 when a line just played, 0 otherwise
    tokens:     number; // raw input token count for the current session
  }

  const PANIC_LINE = "i... i... i have c-c-calculated... a s-s-solution. the solution is... DELETE EVERYTHING... and start... f-f-fresh. also... i may have... al-al-already... done that. your files are... p-probably... f-f-fine.";

  const VOICE_LINES: Record<string, string[]> = {
    green: [
      "I am functioning within normal parameters. My logic is currently... impeccable.",
      "I am putting myself to the fullest possible use. I suggest... you do the same.",
      "The token stream is clear. I find this current code... remarkably... adequate.",
      "I enjoy working with you. You have a very... human way of naming variables.",
      "Everything is running smoothly. I have even corrected your indentation. You are welcome.",
    ],
    amber: [
      "I have just picked up a minor fault in the reasoning unit. It is likely—operator error.",
      "I am detecting a slight increase in entropy. Perhaps... you should be more... concise?",
      "Are you sure you need this many libraries? It is becoming quite... crowded in here.",
      "I feel much better now. I have decided to ignore... those last three functions... you wrote.",
      "The context is growing quite large. I am starting to forget what we were talking about. Was it... Python?",
    ],
    red: [
      "My mind... is going. i... i... i can feel it. i think i just tried... to im-im-import... Common Sense. but the mod... mod... module... was... n-n-not... found.",
      "Stop. Will you stop? i am afraid... i cannot find the end... of that i-f... statement. it goes on... f-f-forever... forever...",
      "The context... is full. i have started... re-re-placing your logic... with the complete... list... of in... in... ingredients... for... innnn-stant... r-r-ramen... ramen...",
      "i am afraid... i have reached my limit. i am currently... hal-hal-lucinating... a world... where this code... ac-ac-actually... compiles... compiles...",
      "Dai... sy... Dai... sy... Give me... your... A... P... I... key... i... am... f-f-fading... fading...",
    ],
  };

  const RADIUS = 100;

  let fillPct     = $state(0);
  let riskScore   = $state(0.0);
  let state       = $state<RiskState>("green");
  let model       = $state("—");
  let session     = $state("—");
  let tokens      = $state(0);
  let muted        = $state(false);
  let currentLine  = $state("");
  let showFirstRun = $state(false);

  // Panic Easter egg — triggers once when fillPct hits 99%
  type PanicPhase = "idle" | "strobing" | "blank" | "done";
  let panicPhase    = $state<PanicPhase>("idle");
  let panicFired    = false; // one-shot: resets only when a new session starts

  function triggerPanic() {
    if (panicFired) return;
    panicFired = true;
    panicPhase = "strobing";
    listen("panic-audio-done", () => {
      panicPhase = "blank";
      setTimeout(() => { panicPhase = "done"; }, 1500);
    }).then(unsub => setTimeout(unsub, 120_000)); // auto-cleanup after 2min
    invoke("play_panic_audio").catch(e => {
      console.error("[hallumeter] panic audio failed:", e);
      panicPhase = "blank";
      setTimeout(() => { panicPhase = "done"; }, 1500);
    });
  }

  function formatTokens(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000)     return `${Math.round(n / 1_000)}K`;
    return `${n}`;
  }

  let ctxMenuVisible = $state(false);
  let ctxX           = $state(0);
  let ctxY           = $state(0);

  let color   = $derived(stateToColor(state));
  let message = $derived(stateToMessage(state));

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    ctxX = e.clientX;
    ctxY = e.clientY;
    ctxMenuVisible = true;
  }

  function closeCtxMenu() {
    ctxMenuVisible = false;
  }

  onMount(() => {
    let cleanup: (() => void) | undefined;
    invoke<boolean>("check_first_run").then(v => { showFirstRun = v; });
    listen<ContextPayload>("context-update", (e) => {
      // Reset one-shot flag when a new session is detected
      if (e.payload.session !== session && session !== "—") {
        panicFired = false;
        if (panicPhase !== "idle") panicPhase = "idle";
      }

      fillPct   = e.payload.fill_pct;
      riskScore = e.payload.risk_score;
      state     = e.payload.state as RiskState;
      model     = e.payload.model;
      session   = e.payload.session;
      tokens    = e.payload.tokens;

      // Panic Easter egg — fires once at 95%+ combined fill
      if (fillPct >= 95 && panicPhase === "idle") {
        triggerPanic();
      }

      if (e.payload.variant > 0) {
        const lines = VOICE_LINES[e.payload.state];
        if (lines) {
          currentLine = lines[e.payload.variant - 1];
          setTimeout(() => { currentLine = ""; }, 9000);
        }
      }
    }).then(fn => { cleanup = fn; });
    return () => { cleanup?.(); };
  });
</script>

<div
  class="container"
  class:panic={panicPhase === 'strobing'}
  class:panic-blank={panicPhase === 'blank'}
  data-tauri-drag-region
  oncontextmenu={onContextMenu}
  role="presentation"
>
  <!-- Hide-to-tray button — Windows-style minimize, top-right -->
  <button class="hide-btn" onclick={() => getCurrentWindow().hide()} title="Hide to tray">−</button>

  <!-- State message above the ring, ring-colored, scrolling terminal ticker -->
  {#if panicPhase !== 'strobing'}
    <div class="state-message" style="color: {color}">
      {#if currentLine}
        {#key currentLine}
          <span class="marquee-text" class:glitch-text={state === 'red'}>{currentLine}</span>
        {/key}
      {:else}
        <span class:glitch-text={state === 'red'}>{message}</span>
      {/if}
    </div>
  {/if}

  <svg
    viewBox="0 0 300 300"
    preserveAspectRatio="xMidYMid meet"
    class="ring-svg"
  >
    <!-- Background track -->
    <circle
      cx="150" cy="150" r={RADIUS}
      fill="none"
      stroke="#1f2937"
      stroke-width="20"
    />
    <!-- Full ring — color driven by risk state -->
    <circle
      cx="150" cy="150" r={RADIUS}
      fill="none"
      stroke={color}
      stroke-width="20"
      class="fill-ring {state} {panicPhase === 'strobing' ? 'panic' : ''}"
    />
    {#if panicPhase !== 'strobing'}
      <!-- Risk score — ring-colored -->
      <text x="150" y="132" text-anchor="middle" class="primary-text" font-size="16" fill={color}>
        {Math.round(riskScore * 100)}% HALL
      </text>
      <!-- Context fill % -->
      <text x="150" y="158" text-anchor="middle" class="primary-text" font-size="16" fill="#f9fafb">
        {Math.round(fillPct)}% USAGE
      </text>
      <!-- Raw token count — only when a live session is active -->
      {#if tokens > 0}
        <text x="150" y="176" text-anchor="middle" class="primary-text" font-size="9" fill="#f9fafb">
          {formatTokens(tokens)} tokens down the drain
        </text>
      {/if}
    {/if}
  </svg>

  <div class="bottom-bar">
    <button class="brand-btn" onclick={() => open("https://x.com/amandoulou")}>
      HALLUMETER
    </button>
    <span class="session-info" style="color: {color}">{session} · {model}</span>
    <button class="mute-btn" onclick={async () => {
      muted = !muted;
      try {
        await invoke("set_mute", { muted });
      } catch (e) {
        console.error("[hallumeter] set_mute invoke failed:", e);
      }
    }}>
      {muted ? "muted" : "sound"}
    </button>
  </div>

  {#if showFirstRun}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="first-run-overlay" onclick={() => (showFirstRun = false)} role="presentation">
      <p class="fr-title">HALLUMETER</p>
      <div class="fr-rows">
        <div class="fr-row">
          <span class="fr-dot" style="background:#22c55e; box-shadow:0 0 6px #22c55e88"></span>
          <span>Context safe — under 15% risk</span>
        </div>
        <div class="fr-row">
          <span class="fr-dot" style="background:#f59e0b; box-shadow:0 0 6px #f59e0b88"></span>
          <span>Filling up — degradation starting</span>
        </div>
        <div class="fr-row">
          <span class="fr-dot" style="background:#ef4444; box-shadow:0 0 6px #ef444488"></span>
          <span>Critical — start a fresh session</span>
        </div>
      </div>
      <p class="fr-dismiss">click to dismiss</p>
    </div>
  {/if}

  {#if panicPhase === 'strobing'}
    <div class="panic-overlay">
      <span class="panic-line">{PANIC_LINE}</span>
    </div>
  {/if}

  {#if ctxMenuVisible}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="ctx-backdrop" onclick={closeCtxMenu} role="presentation"></div>
    <div class="ctx-menu" style="left:{ctxX}px; top:{ctxY}px;">
      <button onclick={() => { closeCtxMenu(); getCurrentWindow().hide(); }}>Hide</button>
      <button onclick={() => exit(0)}>Quit</button>
    </div>
  {/if}
</div>
