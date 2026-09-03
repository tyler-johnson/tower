<script lang="ts">
	import Funnel from '@lucide/svelte/icons/funnel';
	import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
	import DisplayMenu from './DisplayMenu.svelte';
	import FilterBar from './FilterBar.svelte';
	import FilterMenu from './FilterMenu.svelte';
	import ViewChips from './ViewChips.svelte';
	import { feed } from './feed.svelte';
	import { dismiss } from './menu';
	import { query } from './query.svelte';
	import { defaultQuery, type Filter } from './query';
	import { foldRows, refusalLines } from './tower';
	import { views } from './views.svelte';
	import { canonical, unsaved } from './views';

	let saveOpen = $state(false);
	let saveName = $state('');
	let saveShared = $state(false);
	let filtersOpen = $state(false);
	let displayOpen = $state(false);

	async function save(event: SubmitEvent) {
		event.preventDefault();
		const text = canonical(query.search);
		if (text === null) return;
		if ((await views.save(saveName, text, saveShared)) === null) return;
		// The chip appears active on the next list, since its query is
		// the URL's.
		saveOpen = false;
		saveName = '';
		saveShared = false;
	}

	// A new chip from the funnel. A refused query on the URL still opens
	// it: `parsed` is null, so the chip starts from the default, and the
	// write replaces the bad search — a second way out beside the clear
	// link.
	function add(filter: Filter | null) {
		if (filter === null) return;
		const parsed = query.parsed ?? defaultQuery();
		query.replace({ ...parsed, filters: [...parsed.filters, filter] });
		filtersOpen = false;
	}
</script>

<svelte:head>
	<title>tower</title>
</svelte:head>

<!--
	The chrome every view hangs in, one row: the view chips on the left,
	and on the right save, the funnel, and the display sliders. Each
	control is a native details so open and close are keyboard-reachable
	with no state of the shell's own beyond the open flags; the filter
	menu fills the funnel's popover, and the display menu fills the other.

	Save lives here and not on the filter bar because the bar renders
	nothing without a filter, and a query worth saving may be a grouping
	or a mode with no filter at all.
-->
<header class="flex flex-col gap-2">
	<div class="flex items-center gap-2">
		<ViewChips />
		<div class="ml-auto flex items-center gap-1">
			{#if unsaved(query.search, views.list)}
				<details class="dropdown dropdown-end" bind:open={saveOpen} {@attach dismiss()}>
					<summary class="btn btn-sm btn-ghost">save view</summary>
					{#if saveOpen}
						<form
							class="dropdown-content z-10 flex w-64 flex-col gap-2 rounded-box border border-base-300 bg-base-100 p-2 text-sm shadow-sm"
							onsubmit={save}
						>
							<input
								class="input input-sm w-full"
								placeholder="name"
								bind:value={saveName}
								aria-label="name"
							/>
							<label class="flex items-center gap-2">
								<input type="checkbox" class="checkbox checkbox-sm" bind:checked={saveShared} />
								shared with everyone
							</label>
							<button class="btn btn-sm btn-primary" type="submit" disabled={views.busy}>save</button>
						</form>
					{/if}
				</details>
			{/if}
			<details class="dropdown dropdown-end" bind:open={filtersOpen} {@attach dismiss()}>
				<summary class="btn btn-ghost btn-sm btn-square" aria-label="filters">
					<Funnel size={16} />
				</summary>
				{#if filtersOpen}
					<FilterMenu rows={feed.board ? foldRows(feed.board) : null} onpick={add} />
				{/if}
			</details>
			<details class="dropdown dropdown-end" bind:open={displayOpen} {@attach dismiss()}>
				<summary class="btn btn-ghost btn-sm btn-square" aria-label="display">
					<SlidersHorizontal size={16} />
				</summary>
				{#if displayOpen}
					<DisplayMenu />
				{/if}
			</details>
		</div>
	</div>
	<FilterBar />
	{#if feed.error}
		<!--
			The query on the URL did not parse. Core's own words, the way a
			verb's refusal renders in the brief, and the one way back from a
			link nothing here can repair.
		-->
		<div role="alert" class="alert alert-error text-sm">
			<div class="flex flex-col gap-1">
				{#each refusalLines(feed.error) as line, i (i)}
					<span class="whitespace-pre">{line}</span>
				{/each}
			</div>
			<button class="btn btn-ghost btn-sm" onclick={() => query.set('')}>clear the query</button>
		</div>
	{/if}
</header>
