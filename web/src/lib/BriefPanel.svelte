<script lang="ts">
	import { goto } from '$app/navigation';
	import VerbBar from './VerbBar.svelte';
	import { bays } from './bays.svelte';
	import { feed } from './feed.svelte';
	import { panel } from './panel.svelte';
	import {
		age,
		beatLine,
		briefNote,
		buildRefs,
		fieldsLine,
		flightRef,
		refusalLines,
		statusDot,
		statusWord,
		unknownRows,
		writerOf
	} from './tower';

	let b = $derived(feed.board);
	let refs = $derived(b ? buildRefs(b).refs : new Map<string, string>());
	let brief = $derived(panel.brief);
	let now = $derived(feed.now);

	// The board's display form when the flight is on it; the long form
	// otherwise — a flight past the closed window has left the board, and
	// the brief still knows its own number.
	let ref = $derived(
		brief ? (refs.get(brief.id) ?? flightRef(writerOf(brief.id), brief.number, false)) : ''
	);
	let note = $derived(brief ? briefNote(brief, refs, now) : []);
	let subject = $derived(
		brief
			? brief.progress === null
				? brief.subject
				: `${brief.subject} (${brief.progress[0]}/${brief.progress[1]})`
			: ''
	);
	// The bay flying it, off the shared pool the strip keeps live: one
	// fewer request per open, and the line moves with the strip.
	let bay = $derived(brief ? (bays.pool.find((row) => row.flight === brief.id) ?? null) : null);
	let other = $derived(brief ? unknownRows(brief) : []);

	function escape(event: KeyboardEvent) {
		if (event.key === 'Escape') goto('/');
	}
</script>

<svelte:window onkeydown={escape} />

<!-- The board stays live behind it; the backdrop is the way back. -->
<a href="/" aria-label="close the brief" class="fixed inset-0 z-40 bg-base-300/50"></a>

<aside
	class="fixed inset-y-0 right-0 z-50 flex w-full flex-col border-l border-base-300 bg-base-100 sm:max-w-lg"
>
	{#if brief}
		<div class="flex flex-1 flex-col gap-6 overflow-y-auto p-4">
			<header class="flex flex-col gap-2">
				<div class="flex items-baseline gap-3">
					<span class="font-mono text-primary">{ref}</span>
					<h2 class="flex-1 font-medium">{subject}</h2>
					<a href="/" class="btn btn-ghost btn-sm btn-square" aria-label="close">✕</a>
				</div>
				<p class="text-sm text-base-content/40">
					{#each note as phrase, i (i)}
						{#if i > 0}<span> · </span>{/if}
						<span class={phrase.tone === 'warn' ? 'text-warning' : ''}>{phrase.text}</span>
					{/each}
				</p>
				<!--
					The stored fields, one line, the way cmd/brief.rs prints
					them — with the status ahead as its own dot and word,
					since that is the field a reader looks for first.
				-->
				<p class="flex items-baseline gap-2 text-sm text-base-content/40">
					<span class="status {statusDot(brief.status)}"></span>
					<span>{statusWord(brief.status)} · {fieldsLine(brief)}</span>
				</p>
				{#if brief.edited_by !== null && brief.edited_at !== null}
					<p class="text-sm text-base-content/40">
						edited · by {brief.edited_by} · {age(now, brief.edited_at)}
					</p>
				{/if}
				{#each brief.beat as beaten (beaten.flight)}
					<p class="text-sm text-base-content/40">{beatLine(beaten, refs)}</p>
				{/each}
				{#if bay}
					<p class="text-sm text-base-content/40">
						bay {bay.id} · {bay.path}{#if bay.branch}
							· on {bay.branch}{/if}
					</p>
				{/if}
			</header>

			{#if brief.body}
				<p class="text-sm whitespace-pre-wrap">{brief.body}</p>
			{/if}

			<!--
				The family, parents up and children down: every depends-on
				edge is a parent edge, so `blocks` is this flight's parents
				and `depends_on` is its children.
			-->
			{#if brief.blocks.length > 0}
				<section class="flex flex-col gap-1">
					<h3
						class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
					>
						parents
					</h3>
					{#each brief.blocks as link (link.flight)}
						<a
							href="/f/{link.flight}"
							class="flex items-baseline gap-2 rounded-field px-1 hover:bg-base-200"
						>
							<span class="font-mono text-primary">
								{refs.get(link.flight) ?? link.flight}
							</span>
							<span class="status {statusDot(link.status)}" title={link.status}></span>
							<span class="flex-1 truncate">{link.subject}</span>
						</a>
					{/each}
				</section>
			{/if}

			<!--
				The children as a folder: one level, because a brief carries
				one level of links and no more. The whole tree, parents up
				and children down over the same edges, is the projects
				view's job, and it stops here.
			-->
			{#if brief.depends_on.length > 0}
				<section class="flex flex-col gap-1">
					<h3
						class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
					>
						children
					</h3>
					<p class="flex items-baseline gap-2 font-mono text-sm text-base-content/60">
						<span>📁</span>
						<span class="truncate">{brief.subject}</span>
					</p>
					{#each brief.depends_on as link (link.flight)}
						<a
							href="/f/{link.flight}"
							class="ml-4 flex items-baseline gap-2 rounded-field border-l border-base-300 px-1 pl-3 hover:bg-base-200"
						>
							<span class="font-mono text-primary">
								{refs.get(link.flight) ?? link.flight}
							</span>
							<span class="status {statusDot(link.status)}" title={link.status}></span>
							<span class="flex-1 truncate">{link.subject}</span>
							{#if link.closed}<span class="text-success">✓</span>{/if}
						</a>
					{/each}
				</section>
			{/if}

			{#if brief.comments.length > 0}
				<section class="flex flex-col gap-3">
					<h3
						class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
					>
						comments
					</h3>
					{#each brief.comments as comment (comment.id)}
						<div class="flex flex-col gap-1">
							<!--
								The wire id leads the header: it is a comment's only
								name, and what `edit` takes.
							-->
							<p class="font-mono text-xs text-base-content/40">
								{comment.id} · {comment.author} · {age(now, comment.at)}
							</p>
							<p class="text-sm whitespace-pre-wrap">{comment.text}</p>
						</div>
					{/each}
				</section>
			{/if}

			{#if brief.history.length > 0}
				<section class="flex flex-col gap-1">
					<h3
						class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
					>
						history
					</h3>
					{#each brief.history as moment (moment.id)}
						<p class="font-mono text-xs text-base-content/40">
							{moment.id} · {moment.what} · {moment.by} · {age(now, moment.at)}
						</p>
					{/each}
				</section>
			{/if}

			<!--
				A newer tower's fields, shown badly rather than dropped
				silently — the promise `Kind::Unknown` makes the fold, kept
				here too.
			-->
			{#if other.length > 0}
				<section class="flex flex-col gap-1">
					<h3
						class="font-mono text-xs font-medium tracking-[0.2em] uppercase text-base-content/60"
					>
						other
					</h3>
					{#each other as row (row.label)}
						<p class="font-mono text-xs break-all text-base-content/40">
							{row.label} · {row.value}
						</p>
					{/each}
				</section>
			{/if}
		</div>

		<VerbBar {brief} />
	{:else if panel.error !== null}
		<div class="flex flex-1 flex-col gap-4 p-4">
			<div class="flex items-baseline gap-3">
				<h2 class="flex-1 font-medium">no brief</h2>
				<a href="/" class="btn btn-ghost btn-sm btn-square" aria-label="close">✕</a>
			</div>
			<div class="alert alert-error text-sm">
				<span class="font-mono whitespace-pre-wrap"
					>{refusalLines(panel.error).join('\n')}</span
				>
			</div>
		</div>
	{:else}
		<div class="flex flex-1 items-center justify-center p-4 text-sm text-base-content/60">
			reading the brief…
		</div>
	{/if}
</aside>
