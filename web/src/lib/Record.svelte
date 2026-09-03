<script lang="ts">
	import { panel } from './panel.svelte';
	import { query } from './query.svelte';
	import { stream } from './stream';
	import { age, statusDot, type Brief } from './tower';
	import { feed } from './feed.svelte';

	let { brief, refs }: { brief: Brief; refs: Map<string, string> } = $props();

	let now = $derived(feed.now);
	let entries = $derived(stream(brief));

	/// Which of the two texts is open, and the draft standing in for it.
	/// One at a time: an inline edit is a click on the text, and a reader
	/// clicks one thing.
	let editing = $state<'subject' | 'body' | null>(null);
	let draft = $state('');
	/// The comment box, which is always open — a note on a closed record
	/// is fine, and comment.rs is the one verb with no active guard.
	let note = $state('');

	function open(which: 'subject' | 'body') {
		editing = which;
		draft = which === 'subject' ? brief.subject : brief.body;
	}

	/// Enter and Escape both end through the blur: Escape puts the record's
	/// own words back first, so the commit that follows finds nothing
	/// changed and writes nothing. A textarea's Enter is a newline — a body
	/// is prose — so only the subject's input takes it as the commit.
	function keydown(event: KeyboardEvent, which: 'subject' | 'body') {
		if (event.key === 'Escape') {
			draft = which === 'subject' ? brief.subject : brief.body;
			(event.currentTarget as HTMLElement).blur();
		} else if (event.key === 'Enter' && which === 'subject') {
			event.preventDefault();
			(event.currentTarget as HTMLElement).blur();
		}
	}

	async function commit(which: 'subject' | 'body') {
		editing = null;
		const value = draft.trim();
		const was = which === 'subject' ? brief.subject : brief.body;
		// An empty box commits nothing: neither text has a clearing on the
		// wire, and the same words are no edit at all.
		if (value === '' || value === was) return;
		await panel.write(
			'edit',
			which === 'subject' ? { target: brief.id, subject: value } : { target: brief.id, message: value }
		);
	}

	async function send(event: SubmitEvent) {
		event.preventDefault();
		if (note.trim() === '') return;
		await panel.run('comment', { message: note });
		if (panel.error === null) note = '';
	}
</script>

<article class="flex flex-col gap-6">
	<!--
		The subject and the body are the record, and both are edits in
		place: the text a reader reads is the text they type into, so
		nothing about the page says which of the two it is.
	-->
	<header class="flex flex-col gap-2">
		{#if editing === 'subject'}
			<!-- svelte-ignore a11y_autofocus -->
			<input
				type="text"
				class="input input-lg w-full font-medium"
				aria-label="the subject"
				autofocus
				bind:value={draft}
				onkeydown={(event) => keydown(event, 'subject')}
				onblur={() => commit('subject')}
			/>
		{:else}
			<h1 class="flex items-baseline gap-2">
				<button
					type="button"
					class="cursor-text text-left text-xl font-medium"
					onclick={() => open('subject')}
				>
					{brief.subject}
				</button>
				{#if brief.progress !== null}
					<span class="font-mono text-sm text-base-content/40">
						({brief.progress[0]}/{brief.progress[1]})
					</span>
				{/if}
			</h1>
		{/if}
		<p class="font-mono text-xs text-base-content/40">
			filed by {brief.filed_by} · {age(now, brief.filed_at)}
		</p>
	</header>

	{#if editing === 'body'}
		<!-- svelte-ignore a11y_autofocus -->
		<textarea
			class="textarea w-full text-sm"
			rows="6"
			aria-label="the body"
			autofocus
			bind:value={draft}
			onkeydown={(event) => keydown(event, 'body')}
			onblur={() => commit('body')}
		></textarea>
	{:else}
		<button
			type="button"
			class="cursor-text text-left text-sm whitespace-pre-wrap {brief.body
				? ''
				: 'text-base-content/40'}"
			onclick={() => open('body')}
		>
			{brief.body || 'no body'}
		</button>
	{/if}

	<!--
		The family, parents up and children down: every depends-on edge is
		a parent edge, so `blocks` is this flight's parents and
		`depends_on` is its children.
	-->
	{#if brief.blocks.length > 0}
		<section class="flex flex-col gap-1">
			<h2 class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60">
				parents
			</h2>
			{#each brief.blocks as link (link.flight)}
				<a
					href={query.href(`/f/${link.flight}`)}
					class="flex items-baseline gap-2 rounded-field px-1 hover:bg-base-200"
				>
					<span class="font-mono text-primary">{refs.get(link.flight) ?? link.flight}</span>
					<span class="status {statusDot(link.status)}" title={link.status}></span>
					<span class="flex-1 truncate">{link.subject}</span>
				</a>
			{/each}
		</section>
	{/if}

	<!--
		The children as a folder: one level, because a brief carries one
		level of links and no more. The whole tree, parents up and children
		down over the same edges, is the projects view's job, and it stops
		here.
	-->
	{#if brief.depends_on.length > 0}
		<section class="flex flex-col gap-1">
			<h2 class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60">
				children
			</h2>
			<p class="flex items-baseline gap-2 font-mono text-sm text-base-content/60">
				<span>📁</span>
				<span class="truncate">{brief.subject}</span>
			</p>
			{#each brief.depends_on as link (link.flight)}
				<a
					href={query.href(`/f/${link.flight}`)}
					class="ml-4 flex items-baseline gap-2 rounded-field border-l border-base-300 px-1 pl-3 hover:bg-base-200"
				>
					<span class="font-mono text-primary">{refs.get(link.flight) ?? link.flight}</span>
					<span class="status {statusDot(link.status)}" title={link.status}></span>
					<span class="flex-1 truncate">{link.subject}</span>
					{#if link.closed}<span class="text-success">✓</span>{/if}
				</a>
			{/each}
		</section>
	{/if}

	<!--
		What happened and what was said, one column and one order: the two
		lists the wire splits are the same events, and a reader following a
		record follows time.
	-->
	{#if entries.length > 0}
		<section class="flex flex-col gap-3">
			<h2 class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60">
				stream
			</h2>
			{#each entries as entry (entry.id)}
				{#if entry.kind === 'comment'}
					<div class="flex flex-col gap-1">
						<!--
							The wire id leads the header: it is a comment's only
							name, and what `edit` takes.
						-->
						<p class="font-mono text-xs text-base-content/40">
							{entry.id} · {entry.by} · {age(now, entry.at)}
						</p>
						<p class="text-sm whitespace-pre-wrap">{entry.text}</p>
					</div>
				{:else}
					<div class="flex flex-col gap-1">
						<p class="font-mono text-xs text-base-content/40">
							{entry.id} · {entry.what}{entry.line} · {entry.by} · {age(now, entry.at)}
						</p>
						{#if entry.note}
							<p class="pl-4 text-sm whitespace-pre-wrap text-base-content/60">{entry.note}</p>
						{/if}
					</div>
				{/if}
			{/each}
		</section>
	{/if}

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
</article>
