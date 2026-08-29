<script lang="ts">
	import { panel } from './panel.svelte';
	import { allowedVerbs, refusalLines, type Brief, type Verb } from './tower';

	let { brief }: { brief: Brief } = $props();

	let verbs = $derived(allowedVerbs(brief));
	// `comment` has its own box at the bottom rather than a button: it is
	// the one verb that outlives the flight, and a textarea is what it
	// needs anyway.
	let buttons = $derived(verbs.filter((verb) => verb !== 'comment'));

	/// The verb whose one-field form is open — `hold` wants a question,
	/// `answer` wants an answer, and neither can be a bare click.
	let asking = $state<'hold' | 'answer' | null>(null);
	let message = $state('');
	let note = $state('');

	// A verb that lands changes what the flight accepts, so a form left
	// open would offer a gesture the state no longer has.
	$effect(() => {
		if (asking !== null && !verbs.includes(asking)) {
			asking = null;
			message = '';
		}
	});

	function press(verb: Verb) {
		if (verb === 'hold' || verb === 'answer') {
			asking = asking === verb ? null : verb;
			message = '';
			return;
		}
		panel.run(verb);
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

	async function send(event: SubmitEvent) {
		event.preventDefault();
		if (note.trim() === '') return;
		await panel.run('comment', { message: note });
		if (panel.error === null) note = '';
	}
</script>

<div class="flex flex-col gap-4 border-t border-base-300 bg-base-100 p-4">
	<!--
		The affordances above are derived from a fold that may be a frame
		stale, so the server's word is the one that counts — and this is
		where it lands.
	-->
	{#if panel.error !== null}
		<div class="alert alert-error text-sm">
			<span class="font-mono whitespace-pre-wrap">{refusalLines(panel.error).join('\n')}</span>
		</div>
	{/if}

	{#if buttons.length > 0}
		<div class="flex flex-wrap gap-2">
			{#each buttons as verb (verb)}
				<button
					type="button"
					class="btn btn-sm {asking === verb ? 'btn-primary' : ''}"
					disabled={panel.busy}
					onclick={() => press(verb)}
				>
					{verb}
				</button>
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
				<input type="text" class="input w-full" bind:value={message} autofocus />
			</label>
			<div class="flex gap-2">
				<button type="submit" class="btn btn-sm btn-primary" disabled={panel.busy}>
					{asking}
				</button>
				<button type="button" class="btn btn-sm btn-ghost" onclick={() => (asking = null)}>
					cancel
				</button>
			</div>
		</form>
	{/if}

	{#if verbs.includes('comment')}
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
