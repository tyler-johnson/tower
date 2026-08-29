<script lang="ts">
	import { page } from '$app/state';
	import FlightRow from './FlightRow.svelte';
	import { feed } from './board.svelte';
	import { procedures } from './procedures.svelte';
	import { age, buildRefs, type FlightView } from './tower';

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
	// Triage is a mode of the pile, not a section of its own. And the pile
	// is not the `open` section: the board's `open` means no branch op,
	// unclaimed, unheld, while the pile is every live flight whose stored
	// stamp still says `open` — a claimed unclassified flight sits in
	// `in_the_air` and is still unclassified. `FlightView.procedure` is
	// already on the wire, so the count costs no extra read, and the
	// number on the button is what explains the picker's reach.
	let triage = $state(false);
	let pile = $derived(
		b
			? [b.waiting_on_you, b.in_the_air, b.holding, b.open]
					.flat()
					.filter((view) => view.procedure === 'open').length
			: 0
	);

	function toggle() {
		triage = !triage;
		// A definition is a file on disk, so the options are read once, on
		// the first switch-on, and never again on a board frame.
		if (triage) procedures.ensure();
	}

	let sections = $derived(
		b
			? ([
					['waiting on you', '?', b.waiting_on_you],
					['in the air', '▸', b.in_the_air],
					['holding', '‖', b.holding],
					['open', '·', b.open]
				] as [string, string, FlightView[]][])
			: []
	);
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

	{#each sections as [title, glyph, views] (title)}
		{#if views.length > 0}
			<section class="flex flex-col gap-1">
				<h2
					class="flex items-baseline gap-3 font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
				>
					{title}
					{#if title === 'open'}
						<button
							type="button"
							class="btn btn-ghost btn-xs {triage ? 'btn-active' : ''}"
							onclick={toggle}
						>
							triage {pile}
						</button>
					{/if}
				</h2>
				{#each views as view (view.id)}
					<FlightRow {view} {refs} {glyph} now={feed.now} open={view.id === open} {triage} />
				{/each}
			</section>
		{/if}
	{/each}

	{#if b && b.unrouted.length > 0}
		<p class="text-sm text-warning">
			{b.unrouted.length}
			{b.unrouted.length === 1 ? 'event' : 'events'} unrouted — a merge ahead of a filing, or a future
			tower
		</p>
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
