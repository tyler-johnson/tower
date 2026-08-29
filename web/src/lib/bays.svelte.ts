// The pool, shared and live. One instance: the strip and the bay panel
// read the same rows, so a panel opened over the strip costs no second
// request and follows every board frame the strip does.
//
// The pool rides no SSE — the feed carries the board envelope and nothing
// else — so liveness is a re-GET on repository motion, which `BayStrip`
// drives from the feed's last frame.

import { get } from './api';
import type { BayView, Pool, TowerError } from './tower';

class Bays {
	pool = $state<BayView[]>([]);
	error = $state<TowerError | null>(null);

	/// Bumped per request; only the freshest one is allowed to land, so a
	/// slow response cannot overwrite a newer one.
	#latest = 0;

	async refresh() {
		const token = ++this.#latest;
		const answer = await get<Pool>('/api/bays');
		if (token !== this.#latest) return;
		// A failed read leaves the last pool standing: the strip going
		// blank on one bad read would be worse than a stale row the next
		// frame corrects.
		if (answer.error || !answer.data) {
			this.error = answer.error ?? null;
			return;
		}
		this.pool = answer.data.bays;
		this.error = null;
	}
}

export const bays = new Bays();
