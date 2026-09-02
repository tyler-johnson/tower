<script lang="ts">
	import '@fontsource/b612-mono/400.css';
	import '@fontsource/b612-mono/700.css';
	import '../app.css';
	import BayStrip from '$lib/BayStrip.svelte';

	let { children } = $props();
</script>

<!--
	The app frame: the strip is on every page, and the board is the
	(board) group's own layout, so a page outside the group can stand
	without it.
-->
<BayStrip />
{@render children()}
