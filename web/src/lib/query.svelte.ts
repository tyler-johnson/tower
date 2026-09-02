// The query, the page's one piece of state: it lives on the URL, so a
// filtered board is a shareable link, and the board request and the feed
// subscription are both built from it.
//
// A string on this side of the wire on purpose — the server is the only
// parser, per DESIGN's *filters and saved views* — so what the URL holds
// is core's rendered form verbatim, never re-encoded here. The codec
// arrives with the filter bar, which is the first thing that has to edit
// a field rather than carry the whole.

import { goto } from '$app/navigation';
import { page } from '$app/state';

class QueryState {
	/// The query as the URL holds it, without the `?`; `''` is the
	/// default board. `page` is a getter over client state, so this
	/// tracks the URL.
	search = $derived(page.url.search.replace(/^\?/, ''));

	/// `path` with the current query carried along, so a drawer opened
	/// over a filtered board closes back onto the same board.
	href(path: string): string {
		return this.search === '' ? path : `${path}?${this.search}`;
	}

	/// Replace the query on the current path. Replace rather than push:
	/// the filter bar and the display menu will call this on every
	/// change, and each change is a revision of one link rather than a
	/// place the back button should revisit.
	set(search: string): Promise<void> {
		const path = page.url.pathname;
		return goto(search === '' ? path : `${path}?${search}`, {
			replaceState: true,
			keepFocus: true,
			noScroll: true
		});
	}
}

export const query = new QueryState();
