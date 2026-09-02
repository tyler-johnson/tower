<script lang="ts">
	import { page } from '$app/state';
	import FlightRow from './FlightRow.svelte';
	import { template } from './columns';
	import { feed } from './feed.svelte';
	import { DEFAULT_SHOW } from './query';
	import { query } from './query.svelte';
	import { buildRefs, groupTitle, inbox, type FlightView } from './tower';

	let b = $derived(feed.board);
	let q = $derived(query.parsed);
	let show = $derived(q?.show ?? DEFAULT_SHOW);
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

	const heading = 'font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60';
</script>

<!--
	The list. It draws what the fold sent, keyed on whatever the query
	grouped by, in wire order: every group is a details with its name and
	its count on the summary, so a group collapses on its own, and the
	closed group of a status fold starts collapsed because it is the
	render's memory of the week rather than work on the board. Rows lay
	out from the query's `show`, one grid per body so a section's columns
	align down its height.
-->

{#snippet rows(views: FlightView[])}
	<div class="grid gap-x-2" style:grid-template-columns={template(show)}>
		{#each views as view (view.id)}
			<FlightRow {view} {refs} {show} now={feed.now} open={view.id === open} />
		{/each}
	</div>
{/snippet}

{#each pinned as [title, views] (title)}
	{#if views.length > 0}
		<section class="flex flex-col gap-1">
			<h2 class={heading}>{title}</h2>
			{@render rows(views)}
		</section>
	{/if}
{/each}

{#if b}
	{#each b.groups as group (group.key)}
		<details class="flex flex-col gap-1" open={group.key !== 'closed'}>
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

	<footer class="flex flex-wrap items-center gap-2 text-sm text-base-content/60">
		{#if flights === 0}
			<span>nothing on the board · ff tower file to add one</span>
		{:else}
			<span>{flights} {flights === 1 ? 'flight' : 'flights'} · ff tower file to add one</span>
		{/if}
		{#if b.filtered > 0}
			<span>·</span>
			<span>{b.filtered} filtered out</span>
			<button
				class="btn btn-ghost btn-xs"
				onclick={() => {
					if (q) query.replace({ ...q, filters: [] });
				}}
			>
				clear
			</button>
		{/if}
		{#if b.hidden > 0}
			<span>·</span>
			<span>{b.hidden} closed hidden</span>
			<button
				class="btn btn-ghost btn-xs"
				onclick={() => {
					if (q) query.replace({ ...q, closed: 'all' });
				}}
			>
				show all
			</button>
		{/if}
	</footer>
{/if}
