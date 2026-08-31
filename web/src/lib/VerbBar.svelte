<script lang="ts">
	import { panel } from './panel.svelte';
	import { allowedVerbs, refusalLines, type Brief, type Verb } from './tower';

	let { brief }: { brief: Brief } = $props();

	let verbs = $derived(allowedVerbs(brief));

	/// The status words `status` takes. `done` and `cancel` are not among
	/// them: each has its own route, and `cancel` takes a reason — a verb
	/// with a body of its own does not belong in a picker.
	const STATUSES = ['triage', 'waiting', 'ready', 'in_progress', 'held'];
	/// `none` is the wire word for clearing the lane, the CLI's own.
	const LANES = ['me', 'agent', 'none'];

	/// The verb whose own form is open. `hold` wants a question, `answer`
	/// wants an answer, and `cancel` may carry a reason — none can be a
	/// bare click.
	let asking = $state<'hold' | 'answer' | 'cancel' | null>(null);
	let message = $state('');
	let note = $state('');
	let lane = $state('me');
	let word = $state('ready');

	// A verb that lands changes what the flight accepts, so a form left
	// open would offer a gesture the state no longer has.
	$effect(() => {
		if (asking !== null && !verbs.includes(asking)) {
			asking = null;
			message = '';
		}
	});

	function press(verb: 'hold' | 'answer' | 'cancel') {
		asking = asking === verb ? null : verb;
		message = '';
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (asking === null) return;
		// A hold and an answer are the message; a cancel's reason is
		// optional, so an empty box is a bare cancel rather than a refusal
		// waiting to happen.
		if (asking !== 'cancel' && message.trim() === '') return;
		await panel.run(asking, message.trim() === '' ? {} : { message });
		if (panel.error === null) {
			asking = null;
			message = '';
		}
	}

	async function send(event: SubmitEvent) {
		event.preventDefault();
		if (note.trim() === '') return;
		await panel.run('comment', { message: note });
		if (panel.error === null) note = '';
	}

	function has(verb: Verb): boolean {
		return verbs.includes(verb);
	}
</script>

<div class="flex flex-col gap-4 border-t border-base-300 bg-base-100 p-4">
	<!--
		The affordances here are derived from a fold that may be a frame
		stale, so the server's word is the one that counts — and this is
		where it lands.
	-->
	{#if panel.error !== null}
		<div class="alert alert-error text-sm">
			<span class="font-mono whitespace-pre-wrap">{refusalLines(panel.error).join('\n')}</span>
		</div>
	{/if}

	<div class="flex flex-wrap items-center gap-2">
		{#if has('assign')}
			<div class="join">
				<select class="join-item select select-sm" aria-label="the lane" bind:value={lane}>
					{#each LANES as option (option)}
						<option value={option}>{option}</option>
					{/each}
				</select>
				<button
					type="button"
					class="join-item btn btn-sm"
					disabled={panel.busy}
					onclick={() => panel.run('assign', { assignee: lane })}
				>
					assign
				</button>
			</div>
		{/if}

		{#if has('status')}
			<div class="join">
				<select class="join-item select select-sm" aria-label="the status" bind:value={word}>
					{#each STATUSES as option (option)}
						<option value={option}>{option.replaceAll('_', ' ')}</option>
					{/each}
				</select>
				<button
					type="button"
					class="join-item btn btn-sm"
					disabled={panel.busy}
					onclick={() => panel.run('status', { status: word })}
				>
					status
				</button>
			</div>
		{/if}

		{#if has('hold')}
			<button
				type="button"
				class="btn btn-sm {asking === 'hold' ? 'btn-primary' : ''}"
				disabled={panel.busy}
				onclick={() => press('hold')}
			>
				hold
			</button>
		{/if}
		{#if has('answer')}
			<button
				type="button"
				class="btn btn-sm {asking === 'answer' ? 'btn-primary' : ''}"
				disabled={panel.busy}
				onclick={() => press('answer')}
			>
				answer
			</button>
		{/if}
		{#if has('done')}
			<button
				type="button"
				class="btn btn-sm"
				disabled={panel.busy}
				onclick={() => panel.run('done')}
			>
				done
			</button>
		{/if}
		{#if has('cancel')}
			<button
				type="button"
				class="btn btn-sm {asking === 'cancel' ? 'btn-primary' : ''}"
				disabled={panel.busy}
				onclick={() => press('cancel')}
			>
				cancel
			</button>
		{/if}
	</div>

	{#if asking !== null}
		<form class="flex flex-col gap-2" onsubmit={submit}>
			<label class="flex w-full flex-col gap-2">
				<span class="text-sm font-medium">
					{#if asking === 'hold'}the question{:else if asking === 'answer'}the answer{:else}the
						reason — optional{/if}
				</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input type="text" class="input w-full" bind:value={message} autofocus />
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

	{#if has('comment')}
		<form class="flex flex-col gap-2" onsubmit={send}>
			<label class="flex w-full flex-col gap-2">
				<span class="text-sm font-medium">comment</span>
				<textarea class="textarea w-full" rows="2" bind:value={note}></textarea>
			</label>
			<div>
				<button type="submit" class="btn btn-sm" disabled={panel.busy || note.trim() === ''}>
					comment
				</button>
			</div>
		</form>
	{/if}
</div>
