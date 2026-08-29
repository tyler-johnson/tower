<script lang="ts">
	import '@fontsource/b612-mono/400.css';
	import '@fontsource/b612-mono/700.css';
	import '../app.css';
	import Board from '$lib/Board.svelte';

	let { children } = $props();
</script>

<!--
	The board is the layout, not a page: a flight route renders its panel
	over a board that is still live behind it, and the feed is never torn
	down on navigation.
-->
<Board />
{@render children()}
