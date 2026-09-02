<script lang="ts">
	import {
		ageColumn,
		notePhrases,
		priorityGlyph,
		statusDot,
		subjectColumn,
		type FlightView
	} from './tower';
	import { query } from './query.svelte';

	let {
		view,
		refs,
		now,
		open
	}: {
		view: FlightView;
		refs: Map<string, string>;
		now: number;
		open: boolean;
	} = $props();

	let phrases = $derived(notePhrases(view, refs));
</script>

<!--
	The recognizable anatomy, one row: priority glyph, flight ref, status
	dot, subject with its progress mark, label chips, assignee, and the age
	right-aligned. The phrases only this tracker can print — the audits,
	the collisions — go underneath, warn ones in the warn tone.
-->
<a
	href={query.href(`/f/${view.id}`)}
	class="grid grid-cols-[1ch_max-content_1ch_1fr_max-content] items-baseline gap-x-2 rounded-field px-1 hover:bg-base-200 {open
		? 'bg-base-200'
		: ''}"
>
	<span class="text-base-content/60" title="priority {view.priority}">
		{priorityGlyph(view.priority)}
	</span>
	<span class="font-mono text-primary">{refs.get(view.id)}</span>
	<span class="status {statusDot(view.status)}" title={view.status}></span>
	<span class="truncate">{subjectColumn(view)}</span>
	<span class="flex items-baseline gap-2">
		{#each view.labels as label (label)}
			<span class="badge badge-ghost badge-sm">{label}</span>
		{/each}
		{#if view.assignee !== null}
			<span class="badge badge-ghost badge-sm">{view.assignee}</span>
		{/if}
		<span class="text-sm text-base-content/40">{ageColumn(view, now)}</span>
	</span>

	{#if phrases.length > 0}
		<span class="col-span-2 col-start-4 text-sm text-base-content/40">
			{#each phrases as phrase, i (i)}
				{#if i > 0}<span> · </span>{/if}
				<span class={phrase.tone === 'warn' ? 'text-warning' : ''}>{phrase.text}</span>
			{/each}
		</span>
	{/if}
</a>
