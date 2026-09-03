<script lang="ts">
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import { query } from './query.svelte';
	import {
		COLUMNS,
		FIELDS,
		WINDOWS,
		defaultQuery,
		fieldLabel,
		groupable,
		orderable,
		parseClosed,
		render,
		renderWindow,
		withColumn,
		type Field,
		type Mode
	} from './query';

	// A refused query on the URL still opens the menu: the controls draw
	// the default, and the first write replaces the bad search — the
	// funnel's own posture.
	let q = $derived(query.parsed ?? defaultQuery());
	let closed = $derived(renderWindow(q.closed));
	// A hand-typed window that is none of the presets — `closed=25`,
	// `closed=36h` — still shows itself, listed last.
	let extraWindow = $derived(WINDOWS.some((w) => w.value === closed) ? null : closed);
	// Reset stands once any axis but the filters differs. Filters are the
	// bar's; this menu never touches them.
	let dirty = $derived(render({ ...q, filters: [] }) !== '');

	const GROUPABLE = FIELDS.filter(groupable);
	const ORDERABLE = FIELDS.filter(orderable);

	function field(value: string): Field | null {
		return value === '' ? null : (value as Field);
	}

	function setMode(mode: Mode) {
		query.replace({ ...q, mode });
	}

	// Ungrouping clears the sub too, and so does grouping by the field the
	// sub holds: core ignores a sub with no group, and a sub equal to its
	// group folds nothing, and neither should stay on the URL.
	function setGroup(value: string) {
		const group = field(value);
		const subgroup = group === null || group === q.subgroup ? null : q.subgroup;
		query.replace({ ...q, group, subgroup });
	}

	function setSubgroup(value: string) {
		query.replace({ ...q, subgroup: field(value) });
	}

	function setOrderField(value: string) {
		query.replace({ ...q, order: { ...q.order, field: value as Field } });
	}

	function flipOrder() {
		query.replace({ ...q, order: { ...q.order, descending: !q.order.descending } });
	}

	function setEmpty(emptyGroups: boolean) {
		query.replace({ ...q, emptyGroups });
	}

	function setClosed(value: string) {
		const window = parseClosed(value);
		if (window !== null) query.replace({ ...q, closed: window });
	}

	function toggleColumn(column: Field) {
		query.replace({ ...q, show: withColumn(q.show, column, !q.show.includes(column)) });
	}

	function reset() {
		query.replace({ ...defaultQuery(), filters: q.filters });
	}
</script>

<!--
	The display menu: the six axes the filter bar does not edit, and the
	columns. Every control reads the URL's query and writes the whole
	query back through `replace`, the same object the bar writes, so the
	menu holds no state of its own. The selects take `value` and
	`onchange` rather than `bind:value`: a write is a goto, and nothing
	here should keep a copy that could disagree with the URL between the
	change and the navigation.
-->
<div
	class="dropdown-content z-10 flex w-72 flex-col gap-3 rounded-box border border-base-300 bg-base-100 p-3 text-sm shadow-sm"
>
	<div class="join">
		<button
			class="btn btn-sm join-item flex-1"
			class:btn-soft={q.mode === 'list'}
			class:btn-primary={q.mode === 'list'}
			aria-pressed={q.mode === 'list'}
			onclick={() => setMode('list')}>list</button
		>
		<button
			class="btn btn-sm join-item flex-1"
			class:btn-soft={q.mode === 'board'}
			class:btn-primary={q.mode === 'board'}
			aria-pressed={q.mode === 'board'}
			onclick={() => setMode('board')}>board</button
		>
	</div>

	<label class="flex items-center justify-between gap-2">
		<span>grouping</span>
		<select
			class="select select-sm w-36"
			value={q.group ?? ''}
			onchange={(event) => setGroup(event.currentTarget.value)}
		>
			<option value="">none</option>
			{#each GROUPABLE as option (option)}
				<option value={option}>{fieldLabel(option)}</option>
			{/each}
		</select>
	</label>

	<label class="flex items-center justify-between gap-2">
		<span>sub-grouping</span>
		<select
			class="select select-sm w-36"
			value={q.subgroup ?? ''}
			disabled={q.group === null}
			onchange={(event) => setSubgroup(event.currentTarget.value)}
		>
			<option value="">none</option>
			{#each GROUPABLE.filter((option) => option !== q.group) as option (option)}
				<option value={option}>{fieldLabel(option)}</option>
			{/each}
		</select>
	</label>

	<label class="flex items-center justify-between gap-2">
		<span>ordering</span>
		<span class="flex items-center gap-1">
			<select
				class="select select-sm w-28"
				value={q.order.field}
				onchange={(event) => setOrderField(event.currentTarget.value)}
			>
				{#each ORDERABLE as option (option)}
					<option value={option}>{fieldLabel(option)}</option>
				{/each}
			</select>
			<button
				class="btn btn-ghost btn-sm btn-square"
				aria-label={q.order.descending ? 'descending' : 'ascending'}
				onclick={flipOrder}
			>
				{#if q.order.descending}<ArrowDown size={16} />{:else}<ArrowUp size={16} />{/if}
			</button>
		</span>
	</label>

	<label class="flex items-center justify-between gap-2">
		<span>show empty groups</span>
		<input
			type="checkbox"
			class="toggle toggle-sm"
			checked={q.emptyGroups}
			disabled={q.group === null}
			onchange={(event) => setEmpty(event.currentTarget.checked)}
		/>
	</label>

	<label class="flex items-center justify-between gap-2">
		<span>closed flights</span>
		<select
			class="select select-sm w-36"
			value={closed}
			onchange={(event) => setClosed(event.currentTarget.value)}
		>
			{#each WINDOWS as window (window.value)}
				<option value={window.value}>{window.label}</option>
			{/each}
			{#if extraWindow !== null}
				<option value={extraWindow}>{extraWindow}</option>
			{/if}
		</select>
	</label>

	<div class="flex flex-col gap-2">
		<span>display properties</span>
		<div class="flex flex-wrap gap-1">
			{#each COLUMNS as column (column)}
				{@const on = q.show.includes(column)}
				<button
					class="btn btn-xs"
					class:btn-soft={on}
					class:btn-primary={on}
					class:btn-ghost={!on}
					aria-pressed={on}
					onclick={() => toggleColumn(column)}>{fieldLabel(column)}</button
				>
			{/each}
		</div>
	</div>

	{#if dirty}
		<div class="flex justify-end">
			<button class="btn btn-ghost btn-xs" onclick={reset}>reset</button>
		</div>
	{/if}
</div>
