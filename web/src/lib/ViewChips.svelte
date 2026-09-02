<script lang="ts">
	import ViewChip from './ViewChip.svelte';
	import { feed } from './feed.svelte';
	import { dismiss } from './menu';
	import { query } from './query.svelte';
	import { refusalLines } from './tower';
	import { views } from './views.svelte';
	import { BUILTINS, canonical, isActive, unsaved } from './views';

	// The list rides no SSE, so its liveness is this: touching the last
	// frame's stamp subscribes the effect to the board, and `updatedAt`
	// starts null, so it fires once on mount and again on every frame.
	$effect(() => {
		void feed.updatedAt;
		views.refresh();
	});

	let saveOpen = $state(false);
	let saveName = $state('');
	let saveShared = $state(false);

	async function save(event: SubmitEvent) {
		event.preventDefault();
		const text = canonical(query.search);
		if (text === null) return;
		if ((await views.save(saveName, text, saveShared)) === null) return;
		// The chip appears active on the next list, since its query is
		// the URL's.
		saveOpen = false;
		saveName = '';
		saveShared = false;
	}
</script>

<!--
	The chip row under the header: the views, built-in and custom alike,
	drawn the same because to a reader they are the same thing. A pick is
	`query.set` of the view's canonical text, over whatever path is open,
	so a drawer stays up over the new board. The list is the server's
	cut, shared plus the viewer's own, and the viewer is the process's
	git identity, so there is nothing to decide here about who sees what.

	Save lives here and not on the filter bar because the bar renders
	nothing without a filter, and a query worth saving may be a grouping
	or a mode with no filter at all.
-->
<div class="flex flex-wrap items-center gap-2">
	{#each BUILTINS as view (view.name)}
		<button
			class="btn btn-sm btn-ghost"
			class:btn-active={isActive(view, query.search)}
			onclick={() => query.set(view.query)}
		>
			{view.name}
		</button>
	{/each}
	{#each views.list as view (view.id)}
		<ViewChip {view} />
	{/each}
	{#if unsaved(query.search, views.list)}
		<details class="dropdown dropdown-end ml-auto" bind:open={saveOpen} {@attach dismiss()}>
			<summary class="btn btn-sm btn-ghost">save view</summary>
			{#if saveOpen}
				<form
					class="dropdown-content z-10 flex w-64 flex-col gap-2 rounded-box border border-base-300 bg-base-100 p-2 text-sm shadow-sm"
					onsubmit={save}
				>
					<input
						class="input input-sm w-full"
						placeholder="name"
						bind:value={saveName}
						aria-label="name"
					/>
					<label class="flex items-center gap-2">
						<input type="checkbox" class="checkbox checkbox-sm" bind:checked={saveShared} />
						shared with everyone
					</label>
					<button class="btn btn-sm btn-primary" type="submit" disabled={views.busy}>save</button>
				</form>
			{/if}
		</details>
	{/if}
</div>
{#if views.error}
	<div role="alert" class="alert alert-error text-sm">
		<div class="flex flex-col gap-1">
			{#each refusalLines(views.error) as line, i (i)}
				<span class="whitespace-pre">{line}</span>
			{/each}
		</div>
		<button class="btn btn-ghost btn-xs" onclick={() => (views.error = null)}>dismiss</button>
	</div>
{/if}
