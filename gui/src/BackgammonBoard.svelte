<script>
  // The backgammon board from White's side: points 13–24 along the top, 12–1 along the bottom,
  // the bar in the middle and the trays of borne-off checkers on the right. The checkers,
  // dice and cube come from bragd-games; a point is a cell named by its number.
  import { point } from "./board.js";

  /** @type {{ state: any, flipped?: boolean, selected?: string | null, targets?: string[], oncell?: (square: string) => void, onpress?: (square: string, e: PointerEvent) => void }} */
  let { state, flipped = false, selected = null, targets = [], oncell, onpress } = $props();

  const b = $derived(state.board);
  const H = 11; // two rows of points five checkers deep, and a gap
  const at = (/** @type {number} */ n) => point(n, flipped);
  const byPoint = $derived(new Map(b.pieces.filter((/** @type {any} */ p) => p.square !== "bar").map((/** @type {any} */ p) => [p.x, p])));
  const onBar = $derived(b.pieces.filter((/** @type {any} */ p) => p.square === "bar"));
  const numbers = Array.from({ length: 24 }, (_, i) => i + 1);
  // White's bar checkers wait at the bottom of the bar, Black's at the top (from White's side).
  const barTop = (/** @type {any} */ p) => (p.x === 25) !== flipped;
  const offFor = (/** @type {number} */ seat) => b.off.find((/** @type {any} */ h) => h.seat === seat)?.pieces?.[0];
  const offTarget = $derived(targets.includes("off"));
</script>

<svg class="board points" viewBox="-0.1 -0.45 14.2 {H + 0.9}" role="grid" tabindex="-1">
  <rect class="frame" x="-0.1" y="-0.45" width="14.2" height={H + 0.9} />
  <rect class="field" x="0" y="0" width="6" height={H} />
  <rect class="field" x="7" y="0" width="6" height={H} />

  {#each numbers as n}
    {@const p = at(n)}
    {@const tip = p.top ? 5 : H - 5}
    {@const base = p.top ? 0 : H}
    <path
      class="spike"
      class:alt={n % 2 === 0}
      d="M{p.col} {base} L{p.col + 0.5} {tip} L{p.col + 1} {base} Z"
    />
    {#if selected === String(n)}
      <path class="selected" d="M{p.col} {base} L{p.col + 0.5} {tip} L{p.col + 1} {base} Z" />
    {/if}
    {@const pc = byPoint.get(n)}
    {#if pc}
      {#each Array(Math.min(pc.count, 5)) as _, i}
        <use href="#{pc.sprite}" x={p.col} y={p.top ? i : H - 1 - i} width="1" height="1" />
      {/each}
      {#if pc.count > 5}
        <text class="count" x={p.col + 0.5} y={p.top ? 4.7 : H - 4.3}>{pc.count}</text>
      {/if}
    {/if}
    {#if targets.includes(String(n))}
      <circle class="target" cx={p.col + 0.5} cy={p.top ? 5.5 : H - 5.5} r="0.2" />
    {/if}
    <text class="number" x={p.col + 0.5} y={p.top ? -0.12 : H + 0.34}>{n}</text>
  {/each}

  {#each onBar as pc}
    {#each Array(Math.min(pc.count, 4)) as _, i}
      <use href="#{pc.sprite}" x="6" y={barTop(pc) ? 1 + i : H - 2 - i} width="1" height="1" />
    {/each}
    {#if pc.count > 4}
      <text class="count" x="6.5" y={barTop(pc) ? 5.2 : H - 4.8}>{pc.count}</text>
    {/if}
  {/each}
  {#if selected === "bar"}
    <rect class="selected" x="6" y="0" width="1" height={H} />
  {/if}

  <!-- The trays: seat 0 (White) bears off at the bottom, seat 1 at the top. -->
  {#each [0, 1] as seat}
    {@const off = offFor(seat)}
    {@const top = (seat === 1) !== flipped}
    <rect class="tray" class:target={offTarget} x="13.2" y={top ? 0 : H - 5} width="0.8" height="5" />
    {#if off && off.count > 0}
      {#each Array(Math.min(off.count, 15)) as _, i}
        <use href="#{off.sprite}" x="13.2" y={top ? i * 0.3 : H - 0.8 - i * 0.3} width="0.8" height="0.8" />
      {/each}
      <text class="count" x="13.6" y={top ? 5.5 : H - 5.2}>{off.count}</text>
    {/if}
  {/each}

  <!-- The dice of the roll being played, and the cube. -->
  {#each b.dice as d, i}
    <g transform="translate({flipped ? 1.5 + i * 1.2 : 8.5 + i * 1.2} {H / 2 - 0.5})">
      <rect class="die" width="1" height="1" rx="0.15" />
      {#each pips(d) as [x, y]}
        <circle class="pip" cx={x} cy={y} r="0.09" />
      {/each}
    </g>
  {/each}
  {#if b.cube}
    {@const y = b.cube.owner === null ? H / 2 - 0.45 : (b.cube.owner === 1) !== flipped ? 0.1 : H - 1}
    <g transform="translate(6.1 {y})">
      <rect class="cube" width="0.8" height="0.8" rx="0.08" />
      <text class="cube-value" x="0.4" y="0.56">{b.cube.value}</text>
    </g>
  {/if}

  {#if oncell}
    {#each numbers as n}
      {@const p = at(n)}
      <rect
        class="hit"
        data-square={String(n)}
        x={p.col}
        y={p.top ? 0 : H - 5.5}
        width="1"
        height="5.5"
        role="gridcell"
        tabindex="-1"
        aria-label={String(n)}
        onclick={() => oncell(String(n))}
        onkeydown={(e) => e.key === "Enter" && oncell(String(n))}
        onpointerdown={(e) => onpress?.(String(n), e)}
      />
    {/each}
    <rect class="hit" data-square="bar" x="6" y="0" width="1" height={H} role="gridcell" tabindex="-1" aria-label="bar"
      onclick={() => oncell("bar")} onkeydown={(e) => e.key === "Enter" && oncell("bar")} onpointerdown={(e) => onpress?.("bar", e)} />
    <rect class="hit" data-square="off" x="13.2" y="0" width="0.8" height={H} role="gridcell" tabindex="-1" aria-label="off"
      onclick={() => oncell("off")} onkeydown={(e) => e.key === "Enter" && oncell("off")} />
  {/if}
</svg>

<script module>
  /** Where a die's pips go for each face, in a one-by-one box. */
  const P = { l: 0.27, m: 0.5, r: 0.73 };
  /** @param {number} n @returns {number[][]} */
  export function pips(n) {
    const { l, m, r } = P;
    return (
      {
        1: [[m, m]],
        2: [[l, l], [r, r]],
        3: [[l, l], [m, m], [r, r]],
        4: [[l, l], [r, l], [l, r], [r, r]],
        5: [[l, l], [r, l], [m, m], [l, r], [r, r]],
        6: [[l, l], [r, l], [l, m], [r, m], [l, r], [r, r]],
      }[n] ?? []
    );
  }
</script>

<style>
  .board {
    display: block;
    width: 100%;
    height: auto;
    user-select: none;
    touch-action: none;
  }
  .frame {
    fill: var(--board-dark);
  }
  .field {
    fill: var(--board-light);
  }
  .spike {
    fill: var(--board-dark);
    opacity: 0.85;
  }
  .spike.alt {
    fill: var(--board-highlight);
  }
  .selected {
    fill: var(--board-highlight);
    opacity: 0.6;
  }
  .tray {
    fill: var(--board-light);
    opacity: 0.25;
  }
  .tray.target {
    opacity: 0.6;
    fill: var(--board-highlight);
  }
  .target {
    fill: var(--board-marker);
    stroke: var(--board-highlight);
    stroke-width: 0.05;
    pointer-events: none;
  }
  .number {
    font-size: 0.3px;
    text-anchor: middle;
    fill: var(--board-light);
    font-family: system-ui, sans-serif;
  }
  .count {
    font-size: 0.4px;
    font-weight: 700;
    text-anchor: middle;
    fill: var(--board-highlight);
    stroke: var(--board-dark);
    stroke-width: 0.03;
    paint-order: stroke;
    font-family: system-ui, sans-serif;
  }
  .die,
  .cube {
    fill: #faf7f1;
    stroke: #222;
    stroke-width: 0.04;
  }
  .pip {
    fill: #222;
  }
  .cube-value {
    font-size: 0.45px;
    text-anchor: middle;
    font-family: system-ui, sans-serif;
    fill: #222;
  }
  .hit {
    fill: transparent;
    cursor: pointer;
    outline: none;
  }
</style>
