<script lang="ts">
	import { page } from '$app/state';
	import FlightRow from './FlightRow.svelte';
	import { feed } from './feed.svelte';
	import { age, buildRefs, inbox, rowsOf, type FlightView } from './tower';

	$effect(() => {
		feed.connect();
		return () => feed.close();
	});

	let b = $derived(feed.board);
	let built = $derived(b ? buildRefs(b) : { refs: new Map<string, string>(), flights: 0 });
	let refs = $derived(built.refs);
	let flights = $derived(built.flights);
	// The open flight, straight off the path — the board marks its row
	// without holding any state of its own.
	let open = $derived(page.params.flight ?? null);

	// The inbox pinned on top, then the status groups in lifecycle order.
	// A flight in the inbox still stands in its group below: the inbox is
	// a view of the same rows, not a section that removes them. The feed
	// answers the default query, so the groups are keyed by status.
	let sections = $derived.by(() => {
		if (!b) return [] as [string, FlightView[]][];
		const pinned = inbox(b);
		return [
			['questions', pinned.questions],
			['yours', pinned.yours],
			['triage', rowsOf(b, 'triage')],
			['waiting', rowsOf(b, 'waiting')],
			['ready', rowsOf(b, 'ready')],
			['in progress', rowsOf(b, 'in_progress')],
			['held', rowsOf(b, 'held')]
		] as [string, FlightView[]][];
	});
	let closed = $derived(b ? rowsOf(b, 'closed') : []);
</script>

<svelte:head>
	<title>tower</title>
</svelte:head>

<main class="mx-auto flex max-w-4xl flex-col gap-6 p-4">
	<header class="flex items-baseline gap-3">
		<h1 class="font-mono text-lg font-semibold">tower</h1>
		{#if feed.conn === 'live'}
			<span class="flex items-center gap-2 text-sm text-base-content/60">
				<span class="status status-success"></span> live
			</span>
		{:else if feed.conn === 'reconnecting'}
			<span class="flex items-center gap-2 text-sm text-warning">
				<span class="status status-warning"></span>
				reconnecting{#if feed.updatedAt !== null}&nbsp;— last update {age(
						feed.now,
						Math.floor(feed.updatedAt / 1000)
					)}{/if}
			</span>
		{:else}
			<span class="flex items-center gap-2 text-sm text-base-content/60">
				<span class="status status-neutral"></span> connecting
			</span>
		{/if}
	</header>

	{#each sections as [title, views] (title)}
		{#if views.length > 0}
			<section class="flex flex-col gap-1">
				<h2
					class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
				>
					{title}
				</h2>
				{#each views as view (view.id)}
					<FlightRow {view} {refs} now={feed.now} open={view.id === open} />
				{/each}
			</section>
		{/if}
	{/each}

	<!--
		The closed group is the render's memory of the week rather than work
		on the board, so it is collapsed: there when a reader wants it, and
		costing no height when they do not.
	-->
	{#if closed.length > 0}
		<details class="flex flex-col gap-1">
			<summary
				class="cursor-pointer font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
			>
				closed {closed.length}
			</summary>
			<div class="flex flex-col gap-1 pt-1">
				{#each closed as view (view.id)}
					<FlightRow {view} {refs} now={feed.now} open={view.id === open} />
				{/each}
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
</main>
