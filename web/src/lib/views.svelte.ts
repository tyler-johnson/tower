// The saved views, shared and live. One instance: the chip row reads the
// list, and every write here re-reads it, so a chip lands the moment its
// save answers.
//
// The list rides no SSE — the feed carries the board envelope and nothing
// else — so liveness is a re-GET on the feed's stamp, which `ViewChips`
// drives from the last frame the way the root layout re-reads the pool. A save
// is log motion, so a frame follows it; the re-read after a write is only
// so the chip does not wait for one.

import { get, post } from './api';
import type { TowerError } from './tower';
import type { View } from './views';

class Views {
	list = $state<View[]>([]);
	error = $state<TowerError | null>(null);
	/// A write in flight, so a form can hold its button.
	busy = $state(false);

	/// Bumped per read; only the freshest one is allowed to land, so a
	/// slow response cannot overwrite a newer one.
	#latest = 0;

	async refresh() {
		const token = ++this.#latest;
		const answer = await get<{ views: View[] }>('/api/views');
		if (token !== this.#latest) return;
		// A failed read leaves the last list standing: the row going
		// blank on one bad read would be worse than a stale chip the
		// next frame corrects.
		if (answer.error || !answer.data) {
			this.error = answer.error ?? null;
			return;
		}
		this.list = answer.data.views;
		this.error = null;
	}

	/// One write: the route's own body, a refusal kept as `error`, and a
	/// success followed by a re-read. Answers the route's data, or null.
	async #write<T>(path: string, body: unknown): Promise<T | null> {
		this.busy = true;
		const answer = await post<T>(path, body);
		this.busy = false;
		if (answer.error || !answer.data) {
			this.error = answer.error ?? null;
			return null;
		}
		this.error = null;
		await this.refresh();
		return answer.data;
	}

	save(name: string, query: string, shared: boolean) {
		return this.#write<{ view: View }>('/api/views/save', { name, query, shared });
	}

	edit(view: string, body: { name?: string; shared?: boolean }) {
		return this.#write<{ view: View }>('/api/views/edit', { view, ...body });
	}

	remove(view: string) {
		return this.#write<{ deleted: unknown }>('/api/views/delete', { view });
	}
}

export const views = new Views();
