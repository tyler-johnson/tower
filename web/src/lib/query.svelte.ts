// The query, the page's one piece of state: it lives on the URL, so a
// filtered board is a shareable link, and the board request and the feed
// subscription are both built from it.
//
// A string on this side of the wire on purpose — the server is the only
// parser of record, per DESIGN's *filters and saved views* — so what the
// URL holds is core's rendered form verbatim, never re-encoded here. The
// codec in query.ts is the one place the string becomes a struct: the
// filter bar edits one field of it and writes the whole back through
// `replace`. Its `parse` answers null only for a query the server also
// refuses, so `parsed` being null and the feed's error are one fact.

import { goto } from "$app/navigation";
import { page } from "$app/state";
import { parse, render, type Query } from "./query";

class QueryState {
  /// The query as the URL holds it, without the `?`; `''` is the
  /// default board. `page` is a getter over client state, so this
  /// tracks the URL.
  search = $derived(page.url.search.replace(/^\?/, ""));

  /// The URL's query as a struct, or null while the server is refusing
  /// it — the shell's alert has the words, and the bar draws nothing.
  parsed = $derived(parse(this.search));

  /// `path` with the current query carried along, so a drawer opened
  /// over a filtered board closes back onto the same board.
  href(path: string): string {
    return this.search === "" ? path : `${path}?${this.search}`;
  }

  /// Replace the query on the current path. Replace rather than push:
  /// the filter bar and the display menu call this on every change,
  /// and each change is a revision of one link rather than a place the
  /// back button should revisit.
  set(search: string): Promise<void> {
    const path = page.url.pathname;
    return goto(search === "" ? path : `${path}?${search}`, {
      replaceState: true,
      keepFocus: true,
      noScroll: true,
    });
  }

  /// Write a struct back to the URL: the one way an edit lands.
  replace(query: Query): Promise<void> {
    return this.set(render(query));
  }
}

export const query = new QueryState();
