<script lang="ts">
  import BayPanel from "$lib/BayPanel.svelte";
</script>

<!--
	The URL is the state, exactly as for a flight. No effect and no fetch:
	the root layout keeps the pool live, so the panel derives its row and
	follows the board on its own.
-->
<BayPanel />
