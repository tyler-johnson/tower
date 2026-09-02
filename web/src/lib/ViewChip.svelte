<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Users from '@lucide/svelte/icons/users';
	import { dismiss } from './menu';
	import { query } from './query.svelte';
	import { views } from './views.svelte';
	import { isActive, type View } from './views';

	let { view }: { view: View } = $props();

	let open = $state(false);
	let name = $state('');
	// A delete is a final event on the log, so the first click only arms
	// the second.
	let confirming = $state(false);

	// The menu mounts fresh each time it opens, so the draft starts from
	// the name as saved and a half-armed delete does not survive a close.
	$effect(() => {
		if (open) {
			name = view.name;
			confirming = false;
		}
	});

	async function rename(event: SubmitEvent) {
		event.preventDefault();
		if ((await views.edit(view.id, { name })) !== null) open = false;
	}

	async function remove() {
		if (!confirming) {
			confirming = true;
			return;
		}
		if ((await views.remove(view.id)) !== null) open = false;
	}
</script>

<!--
	One custom view: the name button that picks it, and a menu holding
	rename, the personal/shared toggle, and delete. The query is not
	edited here — a view's query is the one it was saved with.
-->
<div class="join">
	<button
		class="btn btn-sm btn-ghost join-item gap-1"
		class:btn-active={isActive(view, query.search)}
		onclick={() => query.set(view.query)}
	>
		{view.name}
		{#if view.shared}
			<Users size={12} class="text-base-content/40" aria-label="shared" />
		{/if}
	</button>
	<details class="dropdown dropdown-end" bind:open {@attach dismiss()}>
		<summary class="btn btn-sm btn-ghost btn-square join-item" aria-label="view menu">
			<ChevronDown size={14} />
		</summary>
		{#if open}
			<div
				class="dropdown-content z-10 flex w-64 flex-col gap-2 rounded-box border border-base-300 bg-base-100 p-2 text-sm shadow-sm"
			>
				<form class="flex gap-2" onsubmit={rename}>
					<input class="input input-sm min-w-0 flex-1" bind:value={name} aria-label="name" />
					<button class="btn btn-sm" type="submit" disabled={views.busy}>rename</button>
				</form>
				<label class="flex flex-col gap-1">
					<span class="flex items-center gap-2">
						<input
							type="checkbox"
							class="checkbox checkbox-sm"
							checked={view.shared}
							disabled={views.busy}
							onchange={(event) =>
								views.edit(view.id, { shared: event.currentTarget.checked })}
						/>
						shared with everyone
					</span>
					<span class="text-xs text-base-content/70">
						A personal view is still on the log; it only renders for its author.
					</span>
				</label>
				<button
					class="btn btn-sm btn-ghost self-start text-error"
					disabled={views.busy}
					onclick={remove}
				>
					{confirming ? 'delete for real' : 'delete'}
				</button>
			</div>
		{/if}
	</details>
</div>
