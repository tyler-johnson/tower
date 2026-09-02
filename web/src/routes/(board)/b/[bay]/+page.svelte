<script lang="ts">
	import BayPanel from '$lib/BayPanel.svelte';
</script>

<!--
	The URL is the state, exactly as for a flight. No effect and no fetch:
	the strip in the layout keeps the pool live, so the panel derives its
	row and follows the board on its own.
-->
<BayPanel />
