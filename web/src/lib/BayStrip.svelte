<script lang="ts">
	import { page } from '$app/state';
	import { bays } from './bays.svelte';
	import { feed } from './feed.svelte';
	import { buildRefs, liveRows, statusDot, type BayView, type Folded } from './tower';

	// The pool rides no SSE, so its liveness is this: touching the last
	// frame's stamp subscribes the effect to the board, and `updatedAt`
	// starts null, so it fires once on mount and again on every frame.
	$effect(() => {
		void feed.updatedAt;
		bays.refresh();
	});

	let b = $derived(feed.board);
	let refs = $derived(b ? buildRefs(b).refs : new Map<string, string>());
	// The open bay, straight off the path — the strip marks its chip
	// without holding any state of its own.
	let here = $derived(page.params.bay ?? null);

	/// A chip carries its occupant's status dot, so the strip and the
	/// board are one vocabulary rather than two. Derived entirely from the
	/// board frame already in hand — `BayView` carries only
	/// free-or-occupied by design.
	///
	/// A board that has not arrived, or an occupant outside the live
	/// groups, renders as held rather than free: the pool read and the
	/// board frame are two reads, and rounding an unknown down to free
	/// would invite a release on a bay somebody is sitting in.
	function chip(bay: BayView, board: Folded | null): { dot: string; title: string } {
		if (bay.flight === null) return { dot: 'status-neutral', title: 'free' };
		if (board) {
			const view = liveRows(board).find((row) => row.id === bay.flight);
			if (view) return { dot: statusDot(view.status), title: view.status };
		}
		return { dot: 'status-error', title: 'occupied' };
	}
</script>

<!--
	The strip is the layout's, not a page's: what is in the air right now
	is the operational question, so it stays mounted and live behind every
	panel. Before the first pool response it renders nothing — no
	placeholder row, the way the board draws no sections until a board
	exists.
-->
<nav class="mx-auto flex w-full max-w-4xl flex-wrap items-baseline gap-2 px-4 pt-4">
	{#each bays.pool as bay (bay.id)}
		{@const state = chip(bay, b)}
		<a
			href="/b/{encodeURIComponent(bay.id)}"
			title={bay.path}
			class="flex items-baseline gap-2 rounded-field px-2 hover:bg-base-200 {bay.id === here
				? 'bg-base-200'
				: ''}"
		>
			<span class="status {state.dot}" title={state.title}></span>
			<span class="font-mono">{bay.id}</span>
			{#if bay.flight !== null}
				<span class="font-mono text-primary">{refs.get(bay.flight) ?? bay.flight}</span>
			{/if}
			{#if bay.current}<span class="text-base-content/40">here</span>{/if}
		</a>
	{/each}
</nav>
