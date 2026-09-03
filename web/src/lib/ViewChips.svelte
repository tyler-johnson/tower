<script lang="ts">
  import ViewChip from "./ViewChip.svelte";
  import { feed } from "./feed.svelte";
  import { query } from "./query.svelte";
  import { refusalLines } from "./tower";
  import { views } from "./views.svelte";
  import { BUILTINS, isActive } from "./views";

  // The list rides no SSE, so its liveness is this: touching the last
  // frame's stamp subscribes the effect to the board, and `updatedAt`
  // starts null, so it fires once on mount and again on every frame.
  $effect(() => {
    void feed.updatedAt;
    views.refresh();
  });
</script>

<!--
	The chips at the navbar's left: the views, built-in and custom alike,
	drawn the same because to a reader they are the same thing. A pick is
	`query.set` of the view's canonical text, over whatever path is open,
	so a drawer stays up over the new board. The list is the server's
	cut, shared plus the viewer's own, and the viewer is the process's
	git identity, so there is nothing to decide here about who sees what.
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
