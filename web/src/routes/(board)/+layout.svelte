<script lang="ts">
	import Board from '$lib/Board.svelte';
	import Shell from '$lib/Shell.svelte';
	import { feed } from '$lib/feed.svelte';
	import { query } from '$lib/query.svelte';

	let { children } = $props();

	// The one subscription, keyed on the query: a new search closes the
	// old source and opens one on the new query; a drawer opening over
	// the board changes the path and not the search, so nothing re-runs.
	$effect(() => {
		feed.connect(query.search);
		return () => feed.close();
	});

	// The columns need the width and the list does not.
	let wide = $derived(query.parsed?.mode === 'board');
</script>

<!--
	The shell and the board are the group's layout, not a page: a drawer
	route renders over both while they are still live behind it, and the
	feed is not torn down on navigation.
-->
<main class="mx-auto flex flex-col gap-6 p-4 {wide ? 'max-w-7xl' : 'max-w-4xl'}">
	<Shell />
	<Board />
</main>
{@render children()}
