<script lang="ts">
	import Funnel from '@lucide/svelte/icons/funnel';
	import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
	import { foldRows } from './facets';
	import DisplayMenu from './DisplayMenu.svelte';
	import FilterBar from './FilterBar.svelte';
	import FilterMenu from './FilterMenu.svelte';
	import { feed } from './feed.svelte';
	import { dismiss } from './menu';
	import { query } from './query.svelte';
	import { defaultQuery, type Filter } from './query';
	import { age, refusalLines } from './tower';

	let filtersOpen = $state(false);
	let displayOpen = $state(false);

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
	The chrome every view hangs in: the crumb back to the unfiltered
	board, the view's name, the connection, and the two menus. Each menu
	is a native details so open and close are keyboard-reachable with no
	state of the shell's own; the filter menu fills the funnel's popover,
	and the display menu fills the other.
-->
<header class="flex flex-col gap-2">
	<div class="flex items-center gap-3">
		<nav class="breadcrumbs text-sm text-base-content/60">
			<ul>
				<li><a href="/">tower</a></li>
			</ul>
		</nav>
		<h1 class="font-mono text-lg font-semibold">board</h1>
		{#if feed.conn === 'live'}
			<span class="flex items-center gap-2 text-sm text-base-content/60">
				<span class="status status-success"></span> live
			</span>
		{:else if feed.conn === 'reconnecting'}
			<span class="flex items-center gap-2 text-sm text-warning">
				<span class="status status-warning"></span>
				reconnecting{#if feed.updatedAt !== null}&nbsp;— last update {age(
						feed.now,
						Math.floor(feed.updatedAt / 1000)
					)}{/if}
			</span>
		{:else}
			<span class="flex items-center gap-2 text-sm text-base-content/60">
				<span class="status status-neutral"></span> connecting
			</span>
		{/if}
		<div class="ml-auto flex items-center gap-1">
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
	<!-- The view chips sit under the header row; nothing renders here yet. -->
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
