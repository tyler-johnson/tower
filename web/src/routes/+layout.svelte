<script lang="ts">
	import '@fontsource/b612-mono/400.css';
	import '@fontsource/b612-mono/700.css';
	import '../app.css';
	import BayStrip from '$lib/BayStrip.svelte';
	import Board from '$lib/Board.svelte';

	let { children } = $props();
</script>

<!--
	The strip and the board are the layout, not a page: a panel route
	renders over both while they are still live behind it, and neither
	effect is torn down on navigation.
-->
<BayStrip />
<Board />
{@render children()}
