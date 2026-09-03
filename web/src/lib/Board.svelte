<script lang="ts">
	import Kanban from './Kanban.svelte';
	import List from './List.svelte';
	import { feed } from './feed.svelte';
	import { query } from './query.svelte';
	import { buildRefs } from './tower';

	let b = $derived(feed.board);
	let q = $derived(query.parsed);
	let mode = $derived(q?.mode ?? 'list');
	let flights = $derived(b ? buildRefs(b).flights : 0);
	let hidden = $derived(b ? b.filtered + b.hidden : 0);
</script>

<!--
	The board: one fold, two renders. The query's `mode` picks the list
	or the kanban, and both read the same frame the feed holds, so a
	switch between them is a URL change and never a second request. The
	footer is shared because it is the fold's, not either render's: the
	live count, and the fold's two disjoint counts summed into one hidden
	number with one remedy, since the closed window is treated as a
	filter too. The region takes what the header and footer leave and
	scrolls inside itself.
-->

<div class="flex min-h-0 flex-1 flex-col gap-3">
	{#if mode === 'board'}
		<Kanban />
	{:else}
		<List />
	{/if}
</div>

{#if b}
	<footer class="flex flex-wrap items-center gap-2 text-sm text-base-content/60">
		{#if flights === 0}
			<span>nothing on the board · ff tower file to add one</span>
		{:else}
			<span>{flights} {flights === 1 ? 'flight' : 'flights'} · ff tower file to add one</span>
		{/if}
		{#if hidden > 0}
			<span>·</span>
			<span>{hidden} hidden by filters</span>
			<button
				class="btn btn-ghost btn-xs"
				onclick={() => {
					if (q) query.replace({ ...q, filters: [], closed: 'all' });
				}}
			>
				show all
			</button>
		{/if}
	</footer>
{/if}
