// The open flight's own state: the brief, the bay flying it, the standing
// refusal, and whether a verb is in the air. One instance, because one
// flight is open at a time — the route is what says which.

import { get, post } from './api';
import type { BayView, Brief, Pool, TowerError } from './tower';

class Panel {
	brief = $state<Brief | null>(null);
	bay = $state<BayView | null>(null);
	error = $state<TowerError | null>(null);
	busy = $state(false);

	/// The flight the current contents belong to, so a response that lost
	/// a race cannot overwrite a newer one, and so a refusal survives a
	/// background refresh of the same flight but not a move to another.
	#showing: string | null = null;
	/// Bumped per request; only the freshest one is allowed to land.
	#latest = 0;

	async refresh(flight: string) {
		if (flight !== this.#showing) {
			this.brief = null;
			this.bay = null;
			this.error = null;
			this.#showing = flight;
		}
		const token = ++this.#latest;
		// One round trip, not two in sequence: the panel is opened by a
		// click and the bay row is part of the same answer.
		const [brief, pool] = await Promise.all([
			get<Brief>(`/api/brief/${encodeURIComponent(flight)}`),
			get<Pool>('/api/bays')
		]);
		if (token !== this.#latest) return;
		if (brief.error || !brief.data) {
			this.brief = null;
			this.bay = null;
			this.error = brief.error ?? null;
			return;
		}
		this.brief = brief.data;
		// A bay flies a flight or it does not; a failed pool read leaves
		// the row off rather than the panel empty, because the bay is the
		// least of what a brief is for.
		this.bay = pool.data?.bays.find((bay) => bay.flight === brief.data?.id) ?? null;
	}

	/// A verb, against the flight on screen. The refusal envelope is kept
	/// as the answer it is; a success refolds, and the board behind the
	/// panel refolds on its own through the feed.
	async run(verb: string, body: Record<string, unknown> = {}) {
		const flight = this.#showing;
		if (flight === null || this.busy) return;
		this.busy = true;
		this.error = null;
		const answer = await post(`/api/${verb}`, { flight, ...body });
		this.busy = false;
		if (answer.error) {
			this.error = answer.error;
			return;
		}
		await this.refresh(flight);
	}
}

export const panel = new Panel();
