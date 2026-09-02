<script lang="ts">
	import { cell, type Cell } from './columns';
	import { draggable as grabbable } from './drag';
	import { fieldLabel, type Field } from './query';
	import { query } from './query.svelte';
	import { notePhrases, statusDot, type FlightView } from './tower';

	let {
		view,
		refs,
		now,
		show,
		open,
		field,
		pending,
		ondragstart,
		ondragend
	}: {
		view: FlightView;
		refs: Map<string, string>;
		now: number;
		show: Field[];
		open: boolean;
		field: Field | null;
		pending: boolean;
		ondragstart: () => void;
		ondragend: () => void;
	} = $props();

	let phrases = $derived(notePhrases(view, refs));
	let canDrag = $derived(grabbable(field, view));

	// The cells of `show`, the grouped field's left out: the column the
	// card stands in already says it.
	let cells = $derived(
		show
			.filter((f) => f !== field)
			.map((f) => ({ field: f, cell: cell(f, view, refs, now) }))
	);
	let head = $derived(cells.filter((c) => ['glyph', 'ref', 'dot'].includes(c.cell.kind)));
	let age = $derived(cells.find((c) => c.field === 'age') ?? null);
	let subject = $derived(cells.find((c) => c.cell.kind === 'subject') ?? null);
	// Everything else, empties dropped so the line closes up when the
	// display menu turns a column off or a row has nothing to say.
	let rest = $derived(
		cells.filter(
			(c) => !head.includes(c) && c !== age && c !== subject && !empty(c.cell)
		)
	);

	function empty(c: Cell): boolean {
		switch (c.kind) {
			case 'chips':
				return c.words.length === 0;
			case 'dim':
				return c.text === '';
			case 'flag':
				return !c.on;
			default:
				return false;
		}
	}

	function start(event: DragEvent) {
		if (!canDrag) {
			event.preventDefault();
			return;
		}
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'move';
			// Firefox starts no drag without a payload.
			event.dataTransfer.setData('text/plain', view.id);
		}
		ondragstart();
	}
</script>

<!--
	One card of the kanban: the row's anatomy folded into a tile under
	the same `show`, drawn from the same cells the row draws. Three
	lines rather than columns — the glyph, ref, dot and the age on the
	first; the subject wrapping on the second; the chips and every other
	dim and flag on the third — and the note phrases underneath, warn
	ones in the warn tone. The tile is a link to the flight, and it picks
	up only where the grouping has a verb for the drop.
-->
<a
	href={query.href(`/f/${view.id}`)}
	draggable={canDrag}
	ondragstart={start}
	ondragend={() => ondragend()}
	class="flex flex-col gap-1 rounded-box border border-base-300 bg-base-100 p-2 hover:bg-base-200 {open
		? 'bg-base-200'
		: ''} {pending ? 'opacity-50' : ''} {canDrag ? 'cursor-grab' : ''}"
>
	{#if head.length > 0 || age !== null}
		<div class="flex items-baseline gap-2">
			{#each head as c (c.field)}
				{#if c.cell.kind === 'glyph'}
					<span class="text-base-content/60" title={c.cell.title}>{c.cell.text}</span>
				{:else if c.cell.kind === 'ref'}
					<span class="font-mono text-primary">{c.cell.text}</span>
				{:else if c.cell.kind === 'dot'}
					<span class="status {statusDot(c.cell.status)}" title={c.cell.status}></span>
				{/if}
			{/each}
			{#if age !== null && age.cell.kind === 'dim'}
				<span class="ml-auto text-sm text-base-content/40" title={fieldLabel('age')}>
					{age.cell.text}
				</span>
			{/if}
		</div>
	{/if}

	{#if subject !== null && subject.cell.kind === 'subject'}
		<span>{subject.cell.text}</span>
	{/if}

	{#if rest.length > 0}
		<div class="flex flex-wrap items-baseline gap-2">
			{#each rest as c (c.field)}
				{#if c.cell.kind === 'chips'}
					{#each c.cell.words as word (word)}
						<span class="badge badge-ghost badge-sm">{word}</span>
					{/each}
				{:else if c.cell.kind === 'dim'}
					<span class="text-sm text-base-content/40" title={fieldLabel(c.field)}>
						{c.cell.text}
					</span>
				{:else if c.cell.kind === 'flag'}
					<span class="text-sm text-warning" title={fieldLabel(c.field)}>{c.cell.text}</span>
				{/if}
			{/each}
		</div>
	{/if}

	{#if phrases.length > 0}
		<span class="text-sm text-base-content/40">
			{#each phrases as phrase, i (i)}
				{#if i > 0}<span> · </span>{/if}
				<span class={phrase.tone === 'warn' ? 'text-warning' : ''}>{phrase.text}</span>
			{/each}
		</span>
	{/if}
</a>
