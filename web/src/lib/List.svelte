<script lang="ts">
  import { page } from "$app/state";
  import FlightRow from "./FlightRow.svelte";
  import { template } from "./columns";
  import { feed } from "./feed.svelte";
  import { DEFAULT_SHOW } from "./query";
  import { query } from "./query.svelte";
  import { buildRefs, groupTitle, type FlightView } from "./tower";

  let b = $derived(feed.board);
  let q = $derived(query.parsed);
  let show = $derived(q?.show ?? DEFAULT_SHOW);
  let refs = $derived(b ? buildRefs(b).refs : new Map<string, string>());
  // The open flight, straight off the path — the list marks its row
  // without holding any state of its own.
  let open = $derived(page.params.flight ?? null);

  const heading = "font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60";
</script>

<!--
	The list: the fold's groups and nothing pinned. The inbox is the
	built-in For Me view, a query like any other, so the list draws what
	the fold sent, keyed on whatever the query grouped by, in wire order:
	every group is a details with its name and
	its count on the summary, so a group collapses on its own, and the
	two closed groups of a status fold, done and canceled, start
	collapsed because they are the render's memory of the week rather
	than work on the board. Rows lay out from the query's `show`, one
	grid per body so a section's columns align down its height. The
	list is one scroll region so the header and footer stay put; a
	scroll per group would fight the collapse.
-->

{#snippet rows(views: FlightView[])}
  <div class="grid gap-x-2" style:grid-template-columns={template(show)}>
    {#each views as view (view.id)}
      <FlightRow {view} {refs} {show} now={feed.now} open={view.id === open} />
    {/each}
  </div>
{/snippet}

{#if b}
  <div class="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto">
    {#each b.groups as group (group.key)}
      <details class="flex flex-col gap-1" open={group.key !== "done" && group.key !== "canceled"}>
        <summary class="cursor-pointer {heading}">
          {groupTitle(group.key)}
          <span class="text-base-content/40">{group.count}</span>
        </summary>
        {#if group.subgroups.length > 0}
          {#each group.subgroups as sub (sub.key)}
            <details class="flex flex-col gap-1 pt-1" open>
              <summary class="cursor-pointer {heading}">
                {groupTitle(sub.key)}
                <span class="text-base-content/40">{sub.count}</span>
              </summary>
              {@render rows(sub.rows)}
            </details>
          {/each}
        {:else}
          {@render rows(group.rows)}
        {/if}
      </details>
    {/each}
  </div>
{/if}
