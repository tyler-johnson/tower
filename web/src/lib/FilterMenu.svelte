<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import { untrack } from "svelte";
  import { facets } from "./facets";
  import {
    FIELDS,
    defaultOp,
    fieldLabel,
    filterable,
    operators,
    parseMoment,
    shape,
    type Field,
    type Filter,
    type Op,
  } from "./query";
  import { statusWord, type FlightView } from "./tower";

  let {
    filter = null,
    start = "field",
    rows,
    onpick,
  }: {
    /// The chip being edited, or null for a new one.
    filter?: Filter | null;
    /// Which level opens first: the funnel starts at the fields, a
    /// segment at its own level.
    start?: "field" | "op" | "value";
    /// The rows the counts are taken from — the board in hand for a
    /// new chip, the probe for an edit; null while neither has landed,
    /// and the count column stays off the values.
    rows: FlightView[] | null;
    /// The finished filter; null drops the chip. `done` says the visit
    /// is over and the caller closes the menu; a words toggle on an
    /// edit leaves it open, so several toggles are one visit.
    onpick: (filter: Filter | null, done: boolean) => void;
  } = $props();

  // The menu mounts on open, so the props seed the visit's state once
  // and only the chip's set is read live.
  const seed = untrack(() => ({ start, filter }));
  let level = $state(seed.start);
  let draft = $state<{ field: Field; op: Op }>(
    seed.filter ? { field: seed.filter.field, op: seed.filter.op } : { field: "status", op: "is" },
  );
  // Whether the value being picked belongs to a field picked on this
  // visit rather than to the chip: a pick then replaces the whole
  // filter rather than toggling one word in the chip's set.
  let fresh = $state(seed.filter === null);
  let text = $state(seed.filter && "text" in seed.filter.value ? seed.filter.value.text : "");
  let span = $state("");

  let kind = $derived(shape(draft.field));
  // The set the chip holds, for the checks — nothing on a new chip or
  // a freshly picked field.
  let picked = $derived(
    !fresh && filter && "words" in filter.value ? filter.value.words : ([] as string[]),
  );
  // The vocabulary with no rows is still a list for status, priority,
  // and the flags; the count column waits for rows.
  let values = $derived(facets(draft.field, rows ?? []));
  let moment = $derived(parseMoment(span.trim()));

  const PRESETS = ["1h", "1d", "3d", "1w", "2w", "4w"];

  function pickField(field: Field) {
    draft = { field, op: defaultOp(field) ?? "after" };
    fresh = true;
    level = "value";
  }

  function pickOp(op: Op) {
    draft.op = op;
    // Every operator a field takes keeps its value's shape, so an
    // edit's value carries over and the pick is the whole change.
    if (!fresh && filter) onpick({ field: draft.field, op, value: filter.value }, true);
    else level = "value";
  }

  function pickWord(word: string) {
    if (fresh) {
      onpick({ field: draft.field, op: draft.op, value: { words: [word] } }, true);
      return;
    }
    const words = picked.includes(word) ? picked.filter((w) => w !== word) : [...picked, word];
    if (words.length === 0) onpick(null, true);
    else onpick({ field: draft.field, op: draft.op, value: { words } }, false);
  }

  function pickText() {
    if (text === "") return;
    onpick({ field: draft.field, op: draft.op, value: { text } }, true);
  }

  function pickAgo(preset: string) {
    const when = parseMoment(preset);
    if (when !== null) onpick({ field: draft.field, op: draft.op, value: { when } }, true);
  }

  function pickMoment() {
    if (moment !== null)
      onpick({ field: draft.field, op: draft.op, value: { when: moment } }, true);
  }

  function word(value: string): string {
    return draft.field === "status" ? statusWord(value) : value;
  }
</script>

<!--
	The cascading menu: the filterable fields, then the picked field's
	values with a count beside each. One component for the funnel (a new
	chip) and for each chip's three segments (an edit). It holds no query
	and calls no goto: it hands a Filter up and the caller writes the URL.
-->
<div
  class="dropdown-content rounded-box border-base-300 bg-base-100 z-10 flex w-64 flex-col gap-1 border p-2 shadow-sm"
>
  {#if level === "field"}
    <ul class="menu menu-sm max-h-80 w-full flex-nowrap overflow-y-auto p-0">
      {#each FIELDS.filter(filterable) as field (field)}
        <li><button onclick={() => pickField(field)}>{fieldLabel(field)}</button></li>
      {/each}
    </ul>
  {:else}
    <ul class="menu menu-sm max-h-80 w-full flex-nowrap overflow-y-auto p-0">
      <li>
        <button class="text-base-content/60" onclick={() => (level = "field")}>
          <ChevronLeft size={16} />
          {fieldLabel(draft.field)}
        </button>
      </li>
      {#if level === "op"}
        {#each operators(draft.field) as op (op)}
          <li>
            <button onclick={() => pickOp(op)}>
              {#if op === draft.op}<Check size={16} />{:else}<span class="size-4"></span>{/if}
              {op === "not" ? "is not" : op}
            </button>
          </li>
        {/each}
      {:else if kind === "words"}
        {#each values as facet (facet.value)}
          <li>
            <button onclick={() => pickWord(facet.value)}>
              {#if picked.includes(facet.value)}
                <Check size={16} />
              {:else}
                <span class="size-4"></span>
              {/if}
              <span class="flex-1">{word(facet.value)}</span>
              {#if rows !== null}<span class="text-base-content/40">{facet.count}</span>{/if}
            </button>
          </li>
        {/each}
      {:else if kind === "time"}
        {#each PRESETS as preset (preset)}
          <li><button onclick={() => pickAgo(preset)}>{preset} ago</button></li>
        {/each}
      {/if}
    </ul>
    <!-- The typed values sit outside the list: a menu item is a button. -->
    {#if level === "value" && kind === "text"}
      <div class="flex items-center gap-2 p-1">
        <input
          class="input input-sm min-w-0 flex-1"
          placeholder="text"
          bind:value={text}
          onkeydown={(event) => {
            if (event.key === "Enter") pickText();
          }}
        />
        <button class="btn btn-sm" disabled={text === ""} onclick={pickText}>contains</button>
      </div>
    {:else if level === "value" && kind === "time"}
      <div class="flex items-center gap-2 p-1">
        <input
          class="input input-sm min-w-0 flex-1"
          placeholder="3d or @epoch"
          bind:value={span}
          onkeydown={(event) => {
            if (event.key === "Enter") pickMoment();
          }}
        />
        <button class="btn btn-sm" disabled={moment === null} onclick={pickMoment}>set</button>
      </div>
    {/if}
  {/if}
</div>
