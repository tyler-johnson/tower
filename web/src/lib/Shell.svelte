<script lang="ts">
	import { feed } from './feed.svelte';
	import { query } from './query.svelte';
	import { age, refusalLines } from './tower';
</script>

<svelte:head>
	<title>tower</title>
</svelte:head>

<!--
	The chrome every view hangs in: the crumb back to the unfiltered
	board, the view's name, the connection, and the two menus. Each menu
	is a native details so open and close are keyboard-reachable with no
	state of the shell's own; the filter bar and the display menu fill
	the popovers.
-->
<header class="flex flex-col gap-2">
	<div class="flex items-center gap-3">
		<nav class="breadcrumbs text-sm text-base-content/60">
			<ul>
				<li><a href="/">tower</a></li>
			</ul>
		</nav>
		<h1 class="font-mono text-lg font-semibold">board</h1>
		{#if feed.conn === 'live'}
			<span class="flex items-center gap-2 text-sm text-base-content/60">
				<span class="status status-success"></span> live
			</span>
		{:else if feed.conn === 'reconnecting'}
			<span class="flex items-center gap-2 text-sm text-warning">
				<span class="status status-warning"></span>
				reconnecting{#if feed.updatedAt !== null}&nbsp;— last update {age(
						feed.now,
						Math.floor(feed.updatedAt / 1000)
					)}{/if}
			</span>
		{:else}
			<span class="flex items-center gap-2 text-sm text-base-content/60">
				<span class="status status-neutral"></span> connecting
			</span>
		{/if}
		<div class="ml-auto flex items-center gap-1">
			<details class="dropdown dropdown-end">
				<summary class="btn btn-ghost btn-sm btn-square" aria-label="filters">
					<svg
						class="size-4"
						fill="none"
						stroke="currentColor"
						stroke-width="1.5"
						viewBox="0 0 24 24"
						aria-hidden="true"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M12 3c2.755 0 5.455.232 8.083.678.533.09.917.556.917 1.096v1.044a2.25 2.25 0 0 1-.659 1.591l-5.432 5.432a2.25 2.25 0 0 0-.659 1.591v2.927a2.25 2.25 0 0 1-1.244 2.013L9.75 21v-6.568a2.25 2.25 0 0 0-.659-1.591L3.659 7.409A2.25 2.25 0 0 1 3 5.818V4.774c0-.54.384-1.006.917-1.096A48.32 48.32 0 0 1 12 3Z"
						/>
					</svg>
				</summary>
				<div
					class="dropdown-content z-10 w-56 rounded-box border border-base-300 bg-base-100 p-3 shadow-sm"
				>
					<p class="text-sm text-base-content/40">filters</p>
				</div>
			</details>
			<details class="dropdown dropdown-end">
				<summary class="btn btn-ghost btn-sm btn-square" aria-label="display">
					<svg
						class="size-4"
						fill="none"
						stroke="currentColor"
						stroke-width="1.5"
						viewBox="0 0 24 24"
						aria-hidden="true"
					>
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							d="M10.5 6h9.75M10.5 6a1.5 1.5 0 1 1-3 0m3 0a1.5 1.5 0 1 0-3 0M3.75 6H7.5m3 12h9.75m-9.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-3.75 0H7.5m9-6h3.75m-3.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-9.75 0h9.75"
						/>
					</svg>
				</summary>
				<div
					class="dropdown-content z-10 w-56 rounded-box border border-base-300 bg-base-100 p-3 shadow-sm"
				>
					<p class="text-sm text-base-content/40">display</p>
				</div>
			</details>
		</div>
	</div>
	<!-- The view chips sit under the header row; nothing renders here yet. -->
	{#if feed.error}
		<!--
			The query on the URL did not parse. Core's own words, the way a
			verb's refusal renders in the brief, and the one way back from a
			link nothing here can repair.
		-->
		<div role="alert" class="alert alert-error text-sm">
			<div class="flex flex-col gap-1">
				{#each refusalLines(feed.error) as line, i (i)}
					<span class="whitespace-pre">{line}</span>
				{/each}
			</div>
			<button class="btn btn-ghost btn-sm" onclick={() => query.set('')}>clear the query</button>
		</div>
	{/if}
</header>
