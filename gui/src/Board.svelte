<script>
  // A board of squares, intersections or a race track, drawn from bragd-games' render model:
  // the cells, the pieces by sprite id, the highlights and the printed labels. The Family decides
  // the look (chequered, ruled, lines with a river and palaces, a track); the rules are not here.
  import { place } from "./board.js";

  /** @type {{ state: any, family: string, flipped?: boolean, selected?: string | null, targets?: string[], dead?: string[], oncell?: (square: string) => void, onpress?: (square: string, e: PointerEvent) => void }} */
  let { state, family, flipped = false, selected = null, targets = [], dead = [], oncell, onpress } = $props();

  const b = $derived(state.board);
  const at = (/** @type {number} */ x, /** @type {number} */ y) => place(x, y, b.files, b.ranks, flipped);
  const grid = $derived(
    Array.from({ length: b.files * b.ranks }, (_, i) => ({ x: i % b.files, y: Math.floor(i / b.files) })),
  );
  const occupied = $derived(new Set(b.pieces.map((/** @type {any} */ p) => p.square)));

  /** xiangqi: the river between ranks 4 and 5, and the two palaces. */
  const river = $derived(b.geometry === "intersections" && b.ranks === 10);
  const lines = $derived(b.geometry === "intersections");
  // Shogi pieces face their owner: turned with the board.
  const turn = $derived(flipped && family === "shogi");
</script>

<svg
  class="board {b.geometry} {family}"
  class:chequered={b.chequered}
  viewBox="-0.08 -0.08 {b.files + 0.16} {b.ranks + 0.16}"
  role="grid"
  tabindex="-1"
>
  <rect class="ground" x="-0.08" y="-0.08" width={b.files + 0.16} height={b.ranks + 0.16} />

  <!-- A track has only its cells; any other board is the whole grid (draughts names only the
       dark squares). -->
  {#each b.geometry === "track" ? b.cells : grid as c}
    {@const p = at(c.x, c.y)}
    <rect
      class="cell"
      class:dark={b.chequered && (c.x + c.y) % 2 === 0}
      class:lined={!b.chequered && !lines}
      x={p.col}
      y={p.row}
      width="1"
      height="1"
    />
  {/each}

  {#if lines}
    <g class="lines">
      {#each Array(b.ranks) as _, r}
        <line x1="0.5" x2={b.files - 0.5} y1={r + 0.5} y2={r + 0.5} />
      {/each}
      {#each Array(b.files) as _, f}
        {#if river && f > 0 && f < b.files - 1}
          <line x1={f + 0.5} x2={f + 0.5} y1="0.5" y2="4.5" />
          <line x1={f + 0.5} x2={f + 0.5} y1="5.5" y2={b.ranks - 0.5} />
        {:else}
          <line x1={f + 0.5} x2={f + 0.5} y1="0.5" y2={b.ranks - 0.5} />
        {/if}
      {/each}
      {#if river}
        {#each [0.5, 7.5] as top}
          <line x1="3.5" y1={top} x2="5.5" y2={top + 2} />
          <line x1="5.5" y1={top} x2="3.5" y2={top + 2} />
        {/each}
      {/if}
    </g>
  {/if}

  {#if family === "shogi"}
    {#each [[3, 3], [3, 6], [6, 3], [6, 6]] as [x, y]}
      <circle class="star" cx={x} cy={y} r="0.06" />
    {/each}
  {/if}

  {#each b.highlights as h}
    {@const p = at(h.x, h.y)}
    {#if h.kind === "marked" && lines}
      <circle class="star" cx={p.col + 0.5} cy={p.row + 0.5} r="0.1" />
    {:else if h.kind === "marked" && family === "ur"}
      <g class="rosette" transform="translate({p.col + 0.5} {p.row + 0.5})">
        {#each [0, 45, 90, 135] as a}
          <ellipse rx="0.36" ry="0.1" transform="rotate({a})" />
        {/each}
        <circle r="0.1" />
      </g>
    {:else if h.kind === "marked"}
      <rect class="marked" x={p.col} y={p.row} width="1" height="1" />
      {#if family === "tafl"}
        <path class="marked-x" d="M{p.col + 0.2} {p.row + 0.2}l0.6 0.6m0 -0.6l-0.6 0.6" />
      {/if}
    {:else}
      <rect class={h.kind} x={p.col} y={p.row} width="1" height="1" />
    {/if}
  {/each}

  {#each b.labels as l}
    {@const p = at(l.x, l.y)}
    <text class="label" x={p.col + 0.06} y={p.row + 0.2}>{l.text}</text>
  {/each}

  {#if selected}
    {#each b.cells.filter((/** @type {any} */ c) => c.square === selected) as c}
      {@const p = at(c.x, c.y)}
      <rect class="selected" x={p.col} y={p.row} width="1" height="1" />
    {/each}
  {/if}

  {#each b.pieces as pc (pc.square + pc.sprite + pc.x)}
    {@const p = at(pc.x, pc.y)}
    <use
      href="#{pc.sprite}"
      x={p.col}
      y={p.row}
      width="1"
      height="1"
      transform={turn ? `rotate(180 ${p.col + 0.5} ${p.row + 0.5})` : undefined}
    />
    {#if pc.count > 1}
      <text class="count" x={p.col + 0.94} y={p.row + 0.94}>{pc.count}</text>
    {/if}
    {#if dead.includes(pc.square)}
      <path class="dead" d="M{p.col + 0.25} {p.row + 0.25}l0.5 0.5m0 -0.5l-0.5 0.5" />
    {/if}
  {/each}

  {#each b.cells.filter((/** @type {any} */ c) => targets.includes(c.square)) as c}
    {@const p = at(c.x, c.y)}
    {#if occupied.has(c.square)}
      <circle class="target ring" cx={p.col + 0.5} cy={p.row + 0.5} r="0.45" />
    {:else}
      <circle class="target" cx={p.col + 0.5} cy={p.row + 0.5} r="0.16" />
    {/if}
  {/each}

  {#if oncell}
    {#each b.cells as c (c.square)}
      {@const p = at(c.x, c.y)}
      <rect
        class="hit"
        data-square={c.square}
        x={p.col}
        y={p.row}
        width="1"
        height="1"
        role="gridcell"
        tabindex="-1"
        aria-label={c.square}
        onclick={() => oncell(c.square)}
        onkeydown={(e) => e.key === "Enter" && oncell(c.square)}
        onpointerdown={(e) => onpress?.(c.square, e)}
      />
    {/each}
  {/if}
</svg>

<style>
  .board {
    display: block;
    width: 100%;
    height: auto;
    max-height: 100%;
    user-select: none;
    touch-action: none;
  }
  .ground {
    fill: var(--board-dark);
  }
  .cell {
    fill: var(--board-light);
  }
  .cell.dark {
    fill: var(--board-dark);
  }
  .cell.lined {
    stroke: var(--board-dark);
    stroke-width: 0.02;
  }
  .intersections .ground,
  .intersections .cell {
    fill: var(--board-light);
  }
  .intersections .ground {
    stroke: var(--board-dark);
    stroke-width: 0.04;
  }
  .track .ground {
    fill: transparent;
  }
  .track .cell {
    stroke: var(--board-dark);
    stroke-width: 0.04;
  }
  .lines line {
    stroke: var(--board-dark);
    stroke-width: 0.03;
  }
  .xiangqi .lines line {
    stroke-width: 0.025;
  }
  .star {
    fill: var(--board-dark);
  }
  .marked {
    fill: var(--board-marker);
  }
  .marked-x {
    stroke: var(--board-dark);
    stroke-width: 0.04;
    opacity: 0.5;
  }
  .rosette {
    fill: var(--board-marker);
    stroke: var(--board-dark);
    stroke-width: 0.02;
  }
  .lastMove {
    fill: var(--board-highlight);
    opacity: 0.5;
  }
  .check {
    fill: var(--red, #c33);
    opacity: 0.55;
  }
  .selected {
    fill: var(--board-highlight);
    opacity: 0.8;
  }
  .target {
    fill: var(--board-highlight);
    stroke: var(--board-dark);
    stroke-width: 0.03;
    opacity: 0.85;
    pointer-events: none;
  }
  .target.ring {
    fill: none;
    stroke: var(--board-highlight);
    stroke-width: 0.07;
  }
  .label {
    font-size: 0.2px;
    fill: var(--board-light);
    font-family: system-ui, sans-serif;
  }
  .track .label {
    fill: var(--board-dark);
  }
  .count {
    font-size: 0.32px;
    font-weight: 700;
    text-anchor: end;
    fill: var(--board-highlight);
    stroke: var(--board-dark);
    stroke-width: 0.02;
    paint-order: stroke;
    font-family: system-ui, sans-serif;
  }
  .dead {
    stroke: var(--red, #c33);
    stroke-width: 0.08;
    pointer-events: none;
  }
  .hit {
    fill: transparent;
    cursor: pointer;
    outline: none;
  }
</style>
