<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import { foldRows } from "./tower";
  import FilterChip from "./FilterChip.svelte";
  import FilterMenu from "./FilterMenu.svelte";
  import { feed } from "./feed.svelte";
  import { dismiss } from "./menu";
  import { query } from "./query.svelte";
  import type { Filter } from "./query";

  let parsed = $derived(query.parsed);
  let addOpen = $state(false);

  function add(filter: Filter | null) {
    if (filter === null || parsed === null) return;
    query.replace({ ...parsed, filters: [...parsed.filters, filter] });
    addOpen = false;
  }
</script>

<!--
	The bar under the header row: one chip per filter, ANDed in order, a
	+ for the next one, and clear. Nothing while the URL holds no filter
	or a query the server refused — the alert has the words then. Save is
	on the chip row above, not here: this bar renders nothing without a
	filter, and a query worth saving may be a grouping or a mode with no
	filter at all.
-->
{#if parsed !== null && parsed.filters.length > 0}
  <div class="flex flex-wrap items-center gap-2">
    {#each parsed.filters as filter, index (index)}
      <FilterChip {index} {filter} />
    {/each}
    <details class="dropdown" bind:open={addOpen} {@attach dismiss()}>
      <summary class="btn btn-ghost btn-xs btn-square" aria-label="add a filter">
        <Plus size={16} />
      </summary>
      {#if addOpen}
        <FilterMenu rows={feed.board ? foldRows(feed.board) : null} onpick={add} />
      {/if}
    </details>
    <button
      class="btn btn-ghost btn-xs ml-auto"
      onclick={() => parsed && query.replace({ ...parsed, filters: [] })}
    >
      clear
    </button>
  </div>
{/if}
