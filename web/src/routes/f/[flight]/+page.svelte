<script lang="ts">
	import { page } from '$app/state';
	import BriefPanel from '$lib/BriefPanel.svelte';
	import { feed } from '$lib/board.svelte';
	import { panel } from '$lib/panel.svelte';

	// Two dependencies, both deliberate: the path, so a different flight
	// loads its own record, and the feed's last frame, so the panel tracks
	// the board under any writer — which is what "over the still-live
	// board" has to mean.
	$effect(() => {
		const flight = page.params.flight;
		// A bare read, and the whole point of it: touching the last
		// frame's stamp is what subscribes this effect to the board.
		void feed.updatedAt;
		if (flight) panel.refresh(flight);
	});
</script>

<BriefPanel />
