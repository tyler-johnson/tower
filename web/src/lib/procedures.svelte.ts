// The installed definitions, fetched lazily and once. A definition is a
// file on disk, not a log event, so refetching it on every board frame
// would be waste the feed cannot justify — `ensure()` is a no-op after
// the first success, and triage calls it when the mode is first switched
// on.

import { get } from './api';
import type { Definition, Listing, TowerError } from './tower';

class Procedures {
	list = $state<Definition[]>([]);
	error = $state<TowerError | null>(null);

	#loaded = false;
	#reading: Promise<void> | null = null;

	async ensure() {
		if (this.#loaded) return;
		// One request even when two rows ask at once.
		this.#reading ??= this.#read();
		await this.#reading;
	}

	async #read() {
		const answer = await get<Listing>('/api/procedures');
		if (answer.error || !answer.data) {
			this.error = answer.error ?? null;
			// Not loaded, so the next switch-on tries again.
			this.#reading = null;
			return;
		}
		this.list = answer.data.procedures;
		this.error = null;
		this.#loaded = true;
	}
}

export const procedures = new Procedures();
