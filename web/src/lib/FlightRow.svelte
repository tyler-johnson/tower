<script lang="ts">
	import { notePhrases, tipColumn, type FlightView } from './tower';

	let {
		view,
		refs,
		glyph,
		now,
		open
	}: {
		view: FlightView;
		refs: Map<string, string>;
		glyph: string;
		now: number;
		open: boolean;
	} = $props();

	let phrases = $derived(notePhrases(view, refs, now));
</script>

<!--
	A row is a link to the flight's own path, so back/forward work and a
	panel is something you can send someone.
-->
<a
	href="/f/{view.id}"
	class="grid grid-cols-[1ch_max-content_1fr_max-content] items-baseline gap-x-2 rounded-field px-1 text-left hover:bg-base-200 {open
		? 'bg-base-200'
		: ''}"
>
	<span class="text-base-content/60">{glyph}</span>
	<span class="font-mono text-primary">{refs.get(view.id)}</span>
	<span class="truncate">{view.subject}</span>
	<span class="font-mono text-base-content/60">{tipColumn(view)}</span>
	<div class="col-span-2 col-start-3 text-sm text-base-content/40">
		{#each phrases as phrase, i (i)}
			{#if i > 0}<span> · </span>{/if}
			<span class={phrase.tone === 'warn' ? 'text-warning' : ''}>{phrase.text}</span>
		{/each}
	</div>
</a>
