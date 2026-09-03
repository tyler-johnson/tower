<script lang="ts">
	import { bays } from './bays.svelte';
	import { feed } from './feed.svelte';
	import { panel } from './panel.svelte';
	import type { Field } from './query';
	import { age, allowedVerbs, beatLine, briefNote, unknownRows, type Brief } from './tower';
	import { write } from './write';

	let { brief, refs }: { brief: Brief; refs: Map<string, string> } = $props();

	let now = $derived(feed.now);
	let verbs = $derived(allowedVerbs(brief));
	let note = $derived(briefNote(brief, refs, now));
	let other = $derived(unknownRows(brief));
	// The bay flying it, off the shared pool the strip keeps live: one
	// fewer request per open, and the line moves with the strip.
	let bay = $derived(bays.pool.find((row) => row.flight === brief.id) ?? null);
	// A closed flight refuses every move and every re-laning — `ensure_active`
	// — and takes an edit, which is why the other four stay live.
	let closed = $derived(brief.status === 'done' || brief.status === 'canceled');

	/// The five words `status` takes. `waiting` comes from links and `held`
	/// from a question: neither is a fact a word can assign, so core refuses
	/// both and neither is offered. A flight standing in one of them shows
	/// it as the current value, unpickable.
	const STATUSES = ['triage', 'ready', 'in_progress', 'done', 'canceled'];
	/// `none` is the wire word for clearing the lane, the CLI's own.
	const LANES = ['me', 'agent', 'none'];
	const PRIORITIES = ['urgent', 'high', 'medium', 'low', 'none'];

	let lane = $derived(brief.assignee ?? 'none');
	// A lane or a priority this build has never heard of still shows as
	// itself: the picker names what the record says before it names what
	// the vocabulary allows.
	let lanes = $derived(LANES.includes(lane) ? LANES : [lane, ...LANES]);
	let priorities = $derived(
		PRIORITIES.includes(brief.priority) ? PRIORITIES : [brief.priority, ...PRIORITIES]
	);
	// A status the fold projects rather than a word anyone wrote.
	let projected = $derived(brief.status === 'waiting' || brief.status === 'held');

	/// A cancel's reason, optional — the one status word with a body.
	let canceling = $state(false);
	let reason = $state('');
	/// The verb whose own form is open. A hold wants a question and an
	/// answer wants an answer; neither can be a bare click.
	let asking = $state<'hold' | 'answer' | null>(null);
	let message = $state('');

	// A verb that lands changes what the flight accepts, so a form left
	// open would offer a gesture the state no longer has.
	$effect(() => {
		if (asking !== null && !verbs.includes(asking)) {
			asking = null;
			message = '';
		}
	});

	/// One field, one value, through the table the kanban's drag writes by.
	async function set(field: Field, value: string | string[] | null) {
		const one = write(field, brief.id, value);
		if (one === null) return;
		await panel.write(one.verb, one.body);
	}

	async function move(event: Event & { currentTarget: HTMLSelectElement }) {
		const select = event.currentTarget;
		const word = select.value;
		// The record's word stands until the server moves it, so a refusal
		// leaves the picker saying what the flight actually is.
		select.value = brief.status;
		if (word === brief.status) return;
		if (word === 'canceled') {
			reason = '';
			canceling = true;
			return;
		}
		await set('status', word);
	}

	async function cancel(event: SubmitEvent) {
		event.preventDefault();
		const one = write('status', brief.id, 'canceled');
		if (one === null) return;
		const body = reason.trim() === '' ? one.body : { ...one.body, message: reason };
		await panel.write(one.verb, body);
		if (panel.error === null) {
			canceling = false;
			reason = '';
		}
	}

	async function relane(event: Event & { currentTarget: HTMLSelectElement }) {
		const select = event.currentTarget;
		const word = select.value;
		select.value = lane;
		if (word !== lane) await set('assignee', word);
	}

	async function reprioritize(event: Event & { currentTarget: HTMLSelectElement }) {
		const select = event.currentTarget;
		const word = select.value;
		select.value = brief.priority;
		if (word !== brief.priority) await set('priority', word);
	}

	/// Skill and bay: free text, and no clearing — neither has one on the
	/// wire, so an emptied box puts the record's own word back.
	async function text(field: 'skill' | 'bay', input: HTMLInputElement) {
		const was = (field === 'skill' ? brief.skill : brief.bay) ?? '';
		const value = input.value.trim();
		if (value === '' || value === was) {
			input.value = was;
			return;
		}
		await set(field, value);
	}

	function keydown(event: KeyboardEvent & { currentTarget: HTMLInputElement }, was: string) {
		if (event.key === 'Escape') event.currentTarget.value = was;
		else if (event.key !== 'Enter') return;
		event.preventDefault();
		event.currentTarget.blur();
	}

	async function label(input: HTMLInputElement) {
		const value = input.value.trim();
		input.value = '';
		if (value === '' || brief.labels.includes(value)) return;
		await set('label', [...brief.labels, value]);
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (asking === null || message.trim() === '') return;
		await panel.run(asking, { message });
		if (panel.error === null) {
			asking = null;
			message = '';
		}
	}
</script>

{#snippet heading(title: string)}
	<span class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60">
		{title}
	</span>
{/snippet}

<!--
	The property rail: six fields, and each of them a control rather than
	a printed value. The verbs with a body of their own — a hold's
	question, an answer's answer — keep their buttons under them, and what
	no one sets is the dim block at the bottom.
-->
<aside class="flex flex-col gap-4 rounded-box border border-base-300 p-4">
	<label class="flex w-full flex-col gap-1">
		{@render heading('status')}
		<select
			class="select select-sm w-full"
			disabled={panel.busy || closed}
			value={brief.status}
			onchange={move}
		>
			{#if projected}
				<option value={brief.status} disabled>{brief.status.replaceAll('_', ' ')}</option>
			{/if}
			{#each STATUSES as word (word)}
				<option value={word}>{word.replaceAll('_', ' ')}</option>
			{/each}
		</select>
	</label>

	{#if canceling}
		<form class="flex flex-col gap-2" onsubmit={cancel}>
			<label class="flex w-full flex-col gap-2">
				<span class="text-sm font-medium">the reason — optional</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input type="text" class="input input-sm w-full" bind:value={reason} autofocus />
			</label>
			<div class="flex gap-2">
				<button type="submit" class="btn btn-sm btn-primary" disabled={panel.busy}>cancel</button>
				<button type="button" class="btn btn-sm btn-ghost" onclick={() => (canceling = false)}>
					never mind
				</button>
			</div>
		</form>
	{/if}

	<label class="flex w-full flex-col gap-1">
		{@render heading('assignee')}
		<select
			class="select select-sm w-full"
			disabled={panel.busy || closed}
			value={lane}
			onchange={relane}
		>
			{#each lanes as word (word)}
				<option value={word}>{word}</option>
			{/each}
		</select>
	</label>

	<label class="flex w-full flex-col gap-1">
		{@render heading('priority')}
		<select
			class="select select-sm w-full"
			disabled={panel.busy}
			value={brief.priority}
			onchange={reprioritize}
		>
			{#each priorities as word (word)}
				<option value={word}>{word}</option>
			{/each}
		</select>
	</label>

	<label class="flex w-full flex-col gap-1">
		{@render heading('skill')}
		<input
			type="text"
			class="input input-sm w-full"
			disabled={panel.busy}
			value={brief.skill ?? ''}
			onkeydown={(event) => keydown(event, brief.skill ?? '')}
			onblur={(event) => text('skill', event.currentTarget)}
		/>
	</label>

	<label class="flex w-full flex-col gap-1">
		{@render heading('bay')}
		<input
			type="text"
			class="input input-sm w-full"
			disabled={panel.busy}
			value={brief.bay ?? ''}
			onkeydown={(event) => keydown(event, brief.bay ?? '')}
			onblur={(event) => text('bay', event.currentTarget)}
		/>
	</label>

	<div class="flex w-full flex-col gap-1">
		{@render heading('labels')}
		{#if brief.labels.length > 0}
			<div class="flex flex-wrap gap-1">
				{#each brief.labels as one (one)}
					<span class="badge badge-sm gap-1">
						{one}
						<!--
							An empty `labels` means unchanged on the wire, so the
							last label cannot be removed: the control refuses it
							rather than sending a write the route would ignore.
						-->
						<button
							type="button"
							aria-label="remove {one}"
							class="cursor-pointer disabled:cursor-not-allowed disabled:opacity-40"
							disabled={panel.busy || brief.labels.length === 1}
							onclick={() => set('label', brief.labels.filter((label) => label !== one))}
						>
							✕
						</button>
					</span>
				{/each}
			</div>
		{/if}
		<input
			type="text"
			class="input input-sm w-full"
			placeholder="add a label"
			aria-label="add a label"
			disabled={panel.busy}
			onkeydown={(event) => keydown(event, '')}
			onblur={(event) => label(event.currentTarget)}
		/>
	</div>

	{#if verbs.includes('hold') || verbs.includes('answer')}
		<div class="flex flex-wrap gap-2">
			{#each ['hold', 'answer'] as const as verb (verb)}
				{#if verbs.includes(verb)}
					<button
						type="button"
						class="btn btn-sm {asking === verb ? 'btn-primary' : ''}"
						disabled={panel.busy}
						onclick={() => {
							asking = asking === verb ? null : verb;
							message = '';
						}}
					>
						{verb}
					</button>
				{/if}
			{/each}
		</div>
	{/if}

	{#if asking !== null}
		<form class="flex flex-col gap-2" onsubmit={submit}>
			<label class="flex w-full flex-col gap-2">
				<span class="text-sm font-medium">
					{asking === 'hold' ? 'the question' : 'the answer'}
				</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input type="text" class="input input-sm w-full" bind:value={message} autofocus />
			</label>
			<div class="flex gap-2">
				<button type="submit" class="btn btn-sm btn-primary" disabled={panel.busy}>
					{asking}
				</button>
				<button type="button" class="btn btn-sm btn-ghost" onclick={() => (asking = null)}>
					never mind
				</button>
			</div>
		</form>
	{/if}

	<!--
		What no one sets: where the flight stands and since when, the
		reason under it, the audits, the beat rows, and the bay flying it.
	-->
	<div class="flex flex-col gap-1 border-t border-base-300 pt-4 text-sm text-base-content/40">
		{#each note as phrase, i (i)}
			<p class={phrase.tone === 'warn' ? 'text-warning' : ''}>{phrase.text}</p>
		{/each}
		{#if brief.edited_by !== null && brief.edited_at !== null}
			<p>edited · by {brief.edited_by} · {age(now, brief.edited_at)}</p>
		{/if}
		{#each brief.beat as beaten (beaten.flight)}
			<p>{beatLine(beaten, refs)}</p>
		{/each}
		{#if bay}
			<p>
				bay {bay.id} · {bay.path}{#if bay.branch}
					· on {bay.branch}{/if}
			</p>
		{/if}
	</div>

	<!--
		A newer tower's fields, shown badly rather than dropped silently —
		the promise `Kind::Unknown` makes the fold, kept here too.
	-->
	{#if other.length > 0}
		<div class="flex flex-col gap-1 border-t border-base-300 pt-4">
			{@render heading('other')}
			{#each other as row (row.label)}
				<p class="font-mono text-xs break-all text-base-content/40">{row.label} · {row.value}</p>
			{/each}
		</div>
	{/if}
</aside>
