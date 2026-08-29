<script lang="ts">
	import { notePhrases, tipColumn, type FlightView } from './tower';

	let {
		view,
		refs,
		glyph,
		now
	}: { view: FlightView; refs: Map<string, string>; glyph: string; now: number } = $props();

	let phrases = $derived(notePhrases(view, refs, now));
</script>

<div class="grid grid-cols-[1ch_max-content_1fr_max-content] items-baseline gap-x-2">
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
</div>
