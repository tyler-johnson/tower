<script lang="ts">
  import { page } from "$app/state";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Rail from "$lib/Rail.svelte";
  import Record from "$lib/Record.svelte";
  import { feed } from "$lib/feed.svelte";
  import { panel } from "$lib/panel.svelte";
  import { query } from "$lib/query.svelte";
  import { buildRefs, flightRef, neighbors, refusalLines, writerOf } from "$lib/tower";

  // Two dependencies, both deliberate: the path, so a different flight
  // loads its own record, and the feed's last frame, so the page tracks
  // the board under any writer.
  $effect(() => {
    const flight = page.params.flight;
    // A bare read, and the whole point of it: touching the last
    // frame's stamp is what subscribes this effect to the board.
    void feed.updatedAt;
    if (flight) panel.refresh(flight);
  });

  let b = $derived(feed.board);
  let refs = $derived(b ? buildRefs(b).refs : new Map<string, string>());
  let brief = $derived(panel.brief);

  // The board's display form when the flight is on it; the long form
  // otherwise — a flight past the closed window has left the board, and
  // the brief still knows its own number.
  let ref = $derived(
    brief ? (refs.get(brief.id) ?? flightRef(writerOf(brief.id), brief.number, false)) : "",
  );
  // The current view's order, so the arrows step the way the board reads.
  // A flight the fold does not carry gets neither.
  let step = $derived(b && brief ? neighbors(b, brief.id) : { prev: null, next: null });
</script>

<svelte:head>
  <title>{brief ? `${ref} ${brief.subject}` : "tower"}</title>
</svelte:head>

<!--
	The record is a page, not a drawer: the board does not render behind
	it, so the left column can be the record itself and the right one a
	rail of live controls. One column under lg, the rail beneath.
-->
<main class="mx-auto flex max-w-7xl flex-col gap-6 p-4">
  <div class="flex items-center gap-3">
    <nav class="breadcrumbs text-base-content/60 flex-1 text-sm">
      <ul>
        <li><a href="/">tower</a></li>
        <!-- The crumb carries the query, so it returns to the view the
					reader came from rather than to the default board. -->
        <li><a href={query.href("/")}>board</a></li>
        <li><span class="text-primary font-mono">{ref}</span></li>
      </ul>
    </nav>
    <div class="join">
      <a
        href={step.prev ? query.href(`/f/${step.prev}`) : undefined}
        class="join-item btn btn-ghost btn-sm btn-square {step.prev ? '' : 'btn-disabled'}"
        aria-label="the flight before"
      >
        <ChevronLeft size={16} />
      </a>
      <a
        href={step.next ? query.href(`/f/${step.next}`) : undefined}
        class="join-item btn btn-ghost btn-sm btn-square {step.next ? '' : 'btn-disabled'}"
        aria-label="the flight after"
      >
        <ChevronRight size={16} />
      </a>
    </div>
  </div>

  {#if brief}
    <!--
			The one standing refusal, above both columns: the affordances on
			either side are derived from a fold that may be a frame stale, so
			the server's word is the one that counts, and there is one of it.
		-->
    {#if panel.error !== null}
      <div role="alert" class="alert alert-error text-sm">
        <span class="font-mono whitespace-pre-wrap">{refusalLines(panel.error).join("\n")}</span>
      </div>
    {/if}
    <div class="flex flex-col gap-8 lg:flex-row lg:items-start">
      <div class="min-w-0 flex-1">
        <Record {brief} {refs} />
      </div>
      <div class="shrink-0 lg:w-72">
        <Rail {brief} {refs} />
      </div>
    </div>
  {:else if panel.error !== null}
    <div role="alert" class="alert alert-error text-sm">
      <span class="font-mono whitespace-pre-wrap">{refusalLines(panel.error).join("\n")}</span>
    </div>
  {:else}
    <p class="text-base-content/60 p-4 text-sm">reading the brief…</p>
  {/if}
</main>
