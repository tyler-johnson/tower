<script lang="ts">
  import Board from "$lib/Board.svelte";
  import Shell from "$lib/Shell.svelte";

  let { children } = $props();
</script>

<!--
	The shell and the board are the group's layout, not a page: the bay
	drawer renders over both while they are still live behind it. The
	flight page is outside the group, so no board renders behind it.
	The main is the window's height, so the board scrolls inside it and
	the footer stays in view.
-->
<main class="mx-auto flex h-dvh min-h-0 w-full max-w-7xl flex-col gap-6 p-4">
  <Shell />
  <Board />
</main>
{@render children()}
