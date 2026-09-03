<script lang="ts">
  import { page } from "$app/state";
  import { SvelteSet } from "svelte/reactivity";
  import { untrack } from "svelte";
  import { post } from "./api";
  import Card from "./Card.svelte";
  import { drop, seeded } from "./drag";
  import { feed } from "./feed.svelte";
  import { DEFAULT_SHOW } from "./query";
  import { query } from "./query.svelte";
  import {
    buildRefs,
    closedRow,
    foldRows,
    groupTitle,
    refusalLines,
    type FlightView,
    type Group,
    type TowerError,
  } from "./tower";

  let b = $derived(feed.board);
  let q = $derived(query.parsed);
  let field = $derived(q?.group ?? null);
  let sub = $derived(q?.subgroup ?? null);
  let show = $derived(q?.show ?? DEFAULT_SHOW);
  let refs = $derived(b ? buildRefs(b).refs : new Map<string, string>());
  let open = $derived(page.params.flight ?? null);

  // The columns: the grouping's own vocabulary first, each the wire's
  // group of that key or an empty one so a drop target stands with
  // nothing in it, then whatever else the wire named in its order.
  let columns = $derived.by(() => {
    if (!b) return [] as Group[];
    const wire = b.groups;
    const out: Group[] = seeded(field).map(
      (key) => wire.find((g) => g.key === key) ?? { key, count: 0, rows: [], subgroups: [] },
    );
    for (const g of wire) if (!out.some((c) => c.key === g.key)) out.push(g);
    return out;
  });

  // The lanes: the union of subgroup keys across the columns, in the
  // order first seen. Each column stacks them, so a lane's count is
  // the column's own, read off its subgroup where it is drawn.
  let lanes = $derived.by(() => {
    if (sub === null) return null;
    const keys: (string | null)[] = [];
    for (const c of columns)
      for (const s of c.subgroups) if (!keys.includes(s.key)) keys.push(s.key);
    return keys.map((key) => ({ key }));
  });

  function cellRows(column: Group, lane: string | null): FlightView[] {
    return column.subgroups.find((s) => s.key === lane)?.rows ?? [];
  }

  // The drag in the air: the card, the column it left, and the lane it
  // left, since a cell takes a drop only in the card's own lane.
  let dragging = $state<{ view: FlightView; from: string | null; lane: string | null } | null>(
    null,
  );
  let over = $state<{ column: string | null; lane: string | null } | null>(null);
  // Cards with a verb in the air, dimmed until the next frame lands.
  let pending = new SvelteSet<string>();
  let refusal = $state<TowerError | null>(null);
  // A `ready` drop whose landing the next frame will tell: the fold
  // derives waiting for a flight with open dependencies, and the echo
  // does not say so.
  let expecting = $state<string | null>(null);
  let landed = $state<string | null>(null);

  function takes(column: string | null, lane: string | null): boolean {
    if (dragging === null) return false;
    if (lanes !== null && lane !== dragging.lane) return false;
    return drop(field, dragging.view, dragging.from, column) !== null;
  }

  function isOver(column: string | null, lane: string | null): boolean {
    return over !== null && over.column === column && over.lane === lane;
  }

  function start(view: FlightView, from: string | null, lane: string | null) {
    dragging = { view, from, lane };
  }

  function end() {
    dragging = null;
    over = null;
  }

  function dragover(event: DragEvent, column: string | null, lane: string | null) {
    if (!takes(column, lane)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    over = { column, lane };
  }

  function dragleave(event: DragEvent) {
    const here = event.currentTarget as HTMLElement;
    if (event.relatedTarget instanceof Node && here.contains(event.relatedTarget)) return;
    over = null;
  }

  async function dropped(event: DragEvent, column: string | null, lane: string | null) {
    event.preventDefault();
    const drag = dragging;
    end();
    if (drag === null || (lanes !== null && lane !== drag.lane)) return;
    const write = drop(field, drag.view, drag.from, column);
    if (write === null) return;
    refusal = null;
    landed = null;
    expecting = write.verb === "status" && write.body.status === "ready" ? drag.view.id : null;
    pending.add(drag.view.id);
    const answer = await post(`/api/${write.verb}`, write.body);
    if (answer.error) {
      refusal = answer.error;
      pending.delete(drag.view.id);
      expecting = null;
    }
  }

  // The frame is the write's answer: it clears what was pending, and
  // it says where a `ready` drop landed.
  $effect(() => {
    void feed.updatedAt;
    const board = b;
    untrack(() => {
      pending.clear();
      if (expecting === null || board === null) return;
      const id = expecting;
      expecting = null;
      const view = foldRows(board).find((row) => row.id === id);
      if (view === undefined || view.status !== "waiting") return;
      const rows = foldRows(board);
      const deps = view.depends_on
        .filter((dep) => {
          const known = rows.find((row) => row.id === dep);
          return known === undefined || !closedRow(known);
        })
        .map((dep) => refs.get(dep) ?? dep);
      landed = `${refs.get(id) ?? id} landed in waiting — depends on ${deps.join(", ")}`;
    });
  });

  const heading = "font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60";
</script>

<!--
	The kanban: the same fold drawn as columns. The columns are the
	grouping, the lanes the sub-grouping, and the card is the row's
	anatomy folded into a tile under the same `show`. A drag is a verb
	or it is not offered: status is `status`, with `done` and `cancel` on
	their own routes; assignee is `assign`; priority, label, skill and
	bay are `edit`; and a grouping with no write behind it draws its
	columns and offers no drag. The write needs no refetch — the feed
	refolds on the log's motion — and the board says why when a drop is
	refused or a `ready` lands in waiting. No inbox: the inbox is the
	For Me view. The headings are the grid's first row and the bodies
	its second, each body its own scroll, and under a sub-grouping a
	column stacks its lanes inside one scroll, so the lane heading
	repeats per column.
-->

{#snippet body(column: Group, rows: FlightView[], lane: string | null)}
  <div
    role="list"
    class="rounded-box bg-base-200/50 flex min-h-24 flex-col gap-2 overflow-y-auto p-2 {isOver(
      column.key,
      lane,
    )
      ? 'ring-primary ring-1'
      : ''}"
    ondragover={(event) => dragover(event, column.key, lane)}
    ondragleave={dragleave}
    ondrop={(event) => dropped(event, column.key, lane)}
  >
    {#each rows as view (view.id)}
      <Card
        {view}
        {refs}
        {show}
        {field}
        now={feed.now}
        open={view.id === open}
        pending={pending.has(view.id)}
        ondragstart={() => start(view, column.key, lane)}
        ondragend={end}
      />
    {/each}
  </div>
{/snippet}

{#if refusal !== null}
  <div role="alert" class="alert alert-error text-sm">
    <div class="flex flex-col gap-1">
      {#each refusalLines(refusal) as line, i (i)}
        <span class="whitespace-pre">{line}</span>
      {/each}
    </div>
    <button class="btn btn-ghost btn-xs" onclick={() => (refusal = null)}>dismiss</button>
  </div>
{/if}

{#if landed !== null}
  <div role="status" class="alert alert-info text-sm">
    <span>{landed}</span>
    <button class="btn btn-ghost btn-xs" onclick={() => (landed = null)}>dismiss</button>
  </div>
{/if}

{#if b}
  <div class="min-h-0 flex-1 overflow-x-auto">
    <div
      class="grid h-full gap-3"
      style:grid-template-columns="repeat({columns.length}, 16rem)"
      style:grid-template-rows="auto minmax(0, 1fr)"
    >
      {#each columns as column (column.key)}
        <h2
          class="{heading} {dragging !== null &&
          drop(field, dragging.view, dragging.from, column.key) === null
            ? 'opacity-60'
            : ''}"
        >
          {groupTitle(column.key)}
          <span class="text-base-content/40">{column.count}</span>
        </h2>
      {/each}

      {#if lanes === null}
        {#each columns as column (column.key)}
          {@render body(column, column.rows, null)}
        {/each}
      {:else}
        {#each columns as column (column.key)}
          <div class="flex min-h-0 flex-col gap-2 overflow-y-auto">
            {#each lanes as lane (lane.key)}
              {@const count = column.subgroups.find((s) => s.key === lane.key)?.count ?? 0}
              <h3 class={heading}>
                {groupTitle(lane.key)}
                <span class="text-base-content/40">{count}</span>
              </h3>
              {@render body(column, cellRows(column, lane.key), lane.key)}
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}
