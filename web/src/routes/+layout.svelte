<script lang="ts">
  import "@fontsource/b612-mono/400.css";
  import "@fontsource/b612-mono/700.css";
  import "../app.css";
  import { bays } from "$lib/bays.svelte";
  import { feed } from "$lib/feed.svelte";
  import { query } from "$lib/query.svelte";

  let { children } = $props();

  // The one subscription, keyed on the query: a new search closes the
  // old source and opens one on the new query; a path change alone —
  // a bay drawer, a flight page — leaves the search alone, so nothing
  // re-runs. It lives here rather than in the (board) group because the
  // flight page, outside the group, reads the same feed.
  $effect(() => {
    feed.connect(query.search);
    return () => feed.close();
  });

  // The pool rides no SSE, so its liveness is this: touching the last
  // frame's stamp subscribes the effect to the board, and `updatedAt`
  // starts null, so it fires once on mount and again on every frame.
  // It stands beside the feed because the bay drawer and the flight
  // page's rail both read `bays.pool`, on either side of the group.
  $effect(() => {
    void feed.updatedAt;
    bays.refresh();
  });
</script>

<!--
	The app frame: the feed and the pool stand behind every page, and the
	board is the (board) group's own layout, so a page outside the group
	can stand without it.
-->
{@render children()}
