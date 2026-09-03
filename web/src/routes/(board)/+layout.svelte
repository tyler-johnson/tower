<script lang="ts">
	import Board from '$lib/Board.svelte';
	import Shell from '$lib/Shell.svelte';
	import { query } from '$lib/query.svelte';

	let { children } = $props();

	// The columns need the width and the list does not.
	let wide = $derived(query.parsed?.mode === 'board');
</script>

<!--
	The shell and the board are the group's layout, not a page: the bay
	drawer renders over both while they are still live behind it. The
	flight page is outside the group, so no board renders behind it.
-->
<main class="mx-auto flex flex-col gap-6 p-4 {wide ? 'max-w-7xl' : 'max-w-4xl'}">
	<Shell />
	<Board />
</main>
{@render children()}
