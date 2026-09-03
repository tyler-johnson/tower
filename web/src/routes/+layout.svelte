<script lang="ts">
	import '@fontsource/b612-mono/400.css';
	import '@fontsource/b612-mono/700.css';
	import '../app.css';
	import BayStrip from '$lib/BayStrip.svelte';
	import { feed } from '$lib/feed.svelte';
	import { query } from '$lib/query.svelte';

	let { children } = $props();

	// The one subscription, keyed on the query: a new search closes the
	// old source and opens one on the new query; a path change alone —
	// a bay drawer, a flight page — leaves the search alone, so nothing
	// re-runs. It lives here rather than in the (board) group because the
	// strip reads the same feed and stands on every page, the flight page
	// included.
	$effect(() => {
		feed.connect(query.search);
		return () => feed.close();
	});
</script>

<!--
	The app frame: the strip is on every page, and the board is the
	(board) group's own layout, so a page outside the group can stand
	without it.
-->
<BayStrip />
{@render children()}
