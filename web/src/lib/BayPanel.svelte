<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { bays } from './bays.svelte';
	import { feed } from './feed.svelte';
	import { query } from './query.svelte';
	import { buildRefs } from './tower';

	// Nothing is fetched here: the pool is already live from the root
	// layout's effect, so the row derives by id and follows every board
	// frame for free.
	let id = $derived(page.params.bay ?? null);
	let bay = $derived(bays.pool.find((row) => row.id === id) ?? null);
	let b = $derived(feed.board);
	let refs = $derived(b ? buildRefs(b).refs : new Map<string, string>());

	function escape(event: KeyboardEvent) {
		if (event.key === 'Escape') goto(query.href('/'));
	}
</script>

<svelte:window onkeydown={escape} />

<!-- The board stays live behind it; the backdrop is the way back. -->
<a href={query.href('/')} aria-label="close the bay" class="fixed inset-0 z-40 bg-base-300/50"></a>

<aside
	class="fixed inset-y-0 right-0 z-50 flex w-full flex-col border-l border-base-300 bg-base-100 sm:max-w-lg"
>
	{#if bay}
		<div class="flex flex-1 flex-col gap-6 overflow-y-auto p-4">
			<header class="flex flex-col gap-2">
				<div class="flex items-baseline gap-3">
					<h2 class="flex-1 font-mono font-medium">{bay.id}</h2>
					{#if bay.current}<span class="text-sm text-base-content/40">here</span>{/if}
					<a href={query.href('/')} class="btn btn-ghost btn-sm btn-square" aria-label="close">✕</a>
				</div>
				<p class="font-mono text-sm break-all text-base-content/40">{bay.path}</p>
				{#if bay.branch}
					<p class="text-sm text-base-content/40">on {bay.branch}</p>
				{:else}
					<p class="text-sm text-base-content/40">(detached)</p>
				{/if}
			</header>

			<section class="flex flex-col gap-1">
				<h3 class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60">
					flying
				</h3>
				{#if bay.flight !== null}
					<a
						href={query.href(`/f/${bay.flight}`)}
						class="flex items-baseline gap-2 rounded-field px-1 hover:bg-base-200"
					>
						<span class="font-mono text-primary">{refs.get(bay.flight) ?? bay.flight}</span>
						<span class="flex-1 truncate">{bay.subject}</span>
					</a>
				{:else}
					<p class="text-sm text-base-content/40">free</p>
				{/if}
			</section>
		</div>
	{:else}
		<div class="flex flex-1 flex-col gap-4 p-4">
			<div class="flex items-baseline gap-3">
				<h2 class="flex-1 font-medium">no such bay</h2>
				<a href={query.href('/')} class="btn btn-ghost btn-sm btn-square" aria-label="close">✕</a>
			</div>
			<p class="text-sm text-base-content/60">
				the pool carries no bay called <span class="font-mono">{id}</span>
			</p>
		</div>
	{/if}
</aside>
