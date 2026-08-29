<script lang="ts">
	import { post } from './api';
	import { procedures } from './procedures.svelte';
	import { notePhrases, refusalLines, tipColumn, type FlightView, type TowerError } from './tower';

	let {
		view,
		refs,
		glyph,
		now,
		open,
		triage
	}: {
		view: FlightView;
		refs: Map<string, string>;
		glyph: string;
		now: number;
		open: boolean;
		triage: boolean;
	} = $props();

	let phrases = $derived(notePhrases(view, refs, now));
	// The stored stamp is what makes a flight unclassified, whatever
	// section it renders in.
	let unclassified = $derived(view.procedure === 'open');

	/// `open` is the stamp it already carries, so it is never a route —
	/// the button stays disabled until a real choice is made.
	let choice = $state('open');
	let busy = $state(false);
	let error = $state<TowerError | null>(null);

	async function route() {
		if (busy || choice === 'open') return;
		busy = true;
		error = null;
		const answer = await post('/api/triage', { flight: view.id, procedure: choice });
		busy = false;
		// Success updates nothing here. The routed event reaches the board
		// through the feed like every other writer's, `view.procedure`
		// changes, and the picker removes itself.
		if (answer.error) error = answer.error;
	}
</script>

<!--
	The row's link is the id and subject cells rather than the whole row:
	the picker is an interactive control, and no control can live inside an
	anchor. `grid-cols-subgrid` keeps the two columns on the row's own
	tracks, so the anchor moving inward changes no alignment.
-->
<div
	class="grid grid-cols-[1ch_max-content_1fr_max-content] items-baseline gap-x-2 rounded-field px-1 hover:bg-base-200 {open
		? 'bg-base-200'
		: ''}"
>
	<span class="text-base-content/60">{glyph}</span>
	<a
		href="/f/{view.id}"
		class="col-span-2 col-start-2 grid grid-cols-subgrid items-baseline gap-x-2 text-left"
	>
		<span class="font-mono text-primary">{refs.get(view.id)}</span>
		<span class="truncate">{view.subject}</span>
	</a>
	<span class="font-mono text-base-content/60">{tipColumn(view)}</span>
	<div class="col-span-2 col-start-3 text-sm text-base-content/40">
		{#each phrases as phrase, i (i)}
			{#if i > 0}<span> · </span>{/if}
			<span class={phrase.tone === 'warn' ? 'text-warning' : ''}>{phrase.text}</span>
		{/each}
	</div>

	{#if triage && unclassified}
		<div class="col-span-2 col-start-3 flex flex-wrap items-baseline gap-2 py-1">
			<select
				class="select select-xs"
				aria-label="route {refs.get(view.id)} to a procedure"
				bind:value={choice}
			>
				{#each procedures.list as definition (definition.name)}
					<option value={definition.name}>{definition.name}</option>
				{/each}
			</select>
			<button
				type="button"
				class="btn btn-xs"
				disabled={busy || choice === 'open'}
				onclick={route}
			>
				route
			</button>
		</div>
		<!--
			The affordance came from a fold that may be a frame stale, so
			the server's word is the one that counts — and this is where it
			lands.
		-->
		{#if error !== null}
			<div class="alert alert-error col-span-2 col-start-3 text-xs">
				<span class="font-mono whitespace-pre-wrap">{refusalLines(error).join('\n')}</span>
			</div>
		{/if}
	{/if}
</div>
