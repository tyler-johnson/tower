<script lang="ts">
	import { page } from '$app/state';
	import FlightRow from './FlightRow.svelte';
	import { feed } from './feed.svelte';
	import { buildRefs, groupTitle, inbox, type FlightView, type Group } from './tower';

	let b = $derived(feed.board);
	let built = $derived(b ? buildRefs(b) : { refs: new Map<string, string>(), flights: 0 });
	let refs = $derived(built.refs);
	let flights = $derived(built.flights);
	// The open flight, straight off the path — the board marks its row
	// without holding any state of its own.
	let open = $derived(page.params.flight ?? null);

	// The inbox pinned on top. A flight in the inbox still stands in its
	// group below: the inbox is a view of the same rows, not a section
	// that removes them.
	let pinned = $derived.by(() => {
		if (!b) return [] as [string, FlightView[]][];
		const box = inbox(b);
		return [
			['questions', box.questions],
			['yours', box.yours]
		] as [string, FlightView[]][];
	});
	// Then the wire's groups in wire order: the board draws only what
	// came back, keyed on whatever the query grouped by. The closed group
	// is the render's memory of the week rather than work on the board,
	// so it collapses.
	let groups = $derived(b ? b.groups.filter((group) => group.key !== 'closed') : []);
	let closed = $derived(b?.groups.find((group) => group.key === 'closed') ?? null);
</script>

{#snippet rows(views: FlightView[])}
	{#each views as view (view.id)}
		<FlightRow {view} {refs} now={feed.now} open={view.id === open} />
	{/each}
{/snippet}

{#each pinned as [title, views] (title)}
	{#if views.length > 0}
		<section class="flex flex-col gap-1">
			<h2
				class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
			>
				{title}
			</h2>
			{@render rows(views)}
		</section>
	{/if}
{/each}

{#each groups as group (group.key)}
	<section class="flex flex-col gap-1">
		<h2
			class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
		>
			{groupTitle(group.key)}
		</h2>
		{#if group.subgroups.length > 0}
			{#each group.subgroups as sub (sub.key)}
				<section class="flex flex-col gap-1 pt-1">
					<h3
						class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
					>
						{groupTitle(sub.key)}
					</h3>
					{@render rows(sub.rows)}
				</section>
			{/each}
		{:else}
			{@render rows(group.rows)}
		{/if}
	</section>
{/each}

{#if closed && closed.count > 0}
	<details class="flex flex-col gap-1">
		<summary
			class="cursor-pointer font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
		>
			closed {closed.count}
		</summary>
		<div class="flex flex-col gap-1 pt-1">
			{@render rows(closed.rows)}
		</div>
	</details>
{/if}

{#if b}
	<footer class="text-sm text-base-content/60">
		{#if flights === 0}
			nothing on the board · ff tower file to add one
		{:else}
			{flights}
			{flights === 1 ? 'flight' : 'flights'} · ff tower file to add one
		{/if}
	</footer>
{/if}
