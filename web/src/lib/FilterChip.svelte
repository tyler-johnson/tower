<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { untrack } from 'svelte';
	import { get } from './api';
	import { foldRows } from './facets';
	import FilterMenu from './FilterMenu.svelte';
	import { feed } from './feed.svelte';
	import { dismiss } from './menu';
	import { query } from './query.svelte';
	import { fieldLabel, opLabel, render, shape, valueLabel, type Filter } from './query';
	import type { Folded, FlightView } from './tower';

	let { index, filter }: { index: number; filter: Filter } = $props();

	let fieldOpen = $state(false);
	let opOpen = $state(false);
	let valueOpen = $state(false);
	let open = $derived(fieldOpen || opOpen || valueOpen);

	// An edit: the chip copies the URL's query, replaces or splices its
	// own slot, and writes the whole back. Any pick that finishes closes
	// whichever segment was open; a words toggle leaves it.
	function pick(next: Filter | null, done: boolean) {
		const parsed = query.parsed;
		if (parsed === null) return;
		const filters = [...parsed.filters];
		if (next === null) filters.splice(index, 1);
		else filters[index] = next;
		query.replace({ ...parsed, filters });
		if (done) fieldOpen = opOpen = valueOpen = false;
	}

	// The probe: the query with this one chip dropped, so each count is
	// the rows the pick would leave — the whole board's counts beside
	// every status, not the ready rows'.
	let base = $derived.by(() => {
		const parsed = query.parsed;
		if (parsed === null) return null;
		return render({ ...parsed, filters: parsed.filters.filter((_, i) => i !== index) });
	});
	let probe = $state<FlightView[] | null>(null);
	let latest = 0;
	$effect(() => {
		if (!open || base === null) {
			probe = null;
			return;
		}
		// The board in hand already answers the base query only when the
		// URL holds exactly that, which a chip's own presence rules out;
		// the check is kept so the two paths stay one rule.
		if (base === untrack(() => query.search)) return;
		const token = ++latest;
		const target = base;
		get<Folded>('/api/board?' + target).then((answer) => {
			if (token !== latest) return;
			// A probe that fails leaves the count column off; the picker
			// still lists the vocabulary.
			probe = answer.data ? foldRows(answer.data) : null;
		});
	});
	let rows = $derived(
		base !== null && base === query.search ? (feed.board ? foldRows(feed.board) : null) : probe
	);
</script>

<!--
	One chip: field, operator, value, each a segment reopening its own
	picker, and an × to drop it. The menus mount on open so each visit
	starts at the chip's current words.
-->
<div class="flex items-center rounded-field border border-base-300 text-sm">
	<details class="dropdown" bind:open={fieldOpen} {@attach dismiss()}>
		<summary class="btn btn-ghost btn-xs">{fieldLabel(filter.field)}</summary>
		{#if fieldOpen}
			<FilterMenu {filter} start="field" {rows} onpick={pick} />
		{/if}
	</details>
	{#if shape(filter.field) === 'text'}
		<span class="px-1 text-base-content/60">{opLabel(filter)}</span>
	{:else}
		<details class="dropdown" bind:open={opOpen} {@attach dismiss()}>
			<summary class="btn btn-ghost btn-xs text-base-content/60">{opLabel(filter)}</summary>
			{#if opOpen}
				<FilterMenu {filter} start="op" {rows} onpick={pick} />
			{/if}
		</details>
	{/if}
	<details class="dropdown" bind:open={valueOpen} {@attach dismiss()}>
		<summary class="btn btn-ghost btn-xs">{valueLabel(filter)}</summary>
		{#if valueOpen}
			<FilterMenu {filter} start="value" {rows} onpick={pick} />
		{/if}
	</details>
	<button
		class="btn btn-ghost btn-xs btn-square"
		aria-label="drop the filter"
		onclick={() => pick(null, true)}
	>
		<X size={16} />
	</button>
</div>
