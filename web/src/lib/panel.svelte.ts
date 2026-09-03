// The open flight's own state: the brief, the standing refusal, and
// whether a verb is in the air. One instance, because one flight is open
// at a time — the route is what says which.
//
// The bay flying it is not here: the pool is a shared store the strip
// keeps live, so the panel's bay line derives from it rather than costing
// a second request per open.

import { get, post } from './api';
import type { Brief, TowerError } from './tower';

class Panel {
	brief = $state<Brief | null>(null);
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
			this.error = null;
			this.#showing = flight;
		}
		const token = ++this.#latest;
		const brief = await get<Brief>(`/api/brief/${encodeURIComponent(flight)}`);
		if (token !== this.#latest) return;
		if (brief.error || !brief.data) {
			this.brief = null;
			this.error = brief.error ?? null;
			return;
		}
		this.brief = brief.data;
	}

	/// A verb keyed on the flight on screen: the body is the verb's own
	/// arguments, and `flight` is added here.
	async run(verb: string, body: Record<string, unknown> = {}) {
		const flight = this.#showing;
		if (flight === null) return;
		await this.write(verb, { flight, ...body });
	}

	/// A write whose body is complete, posted verbatim — `/api/edit` keys
	/// the flight as `target`, and `deny_unknown_fields` rejects a `flight`
	/// beside it, so a route's own shape is the caller's to build.
	///
	/// The refusal envelope is kept as the answer it is; a success refolds
	/// the record, and the board refolds on its own through the feed.
	async write(verb: string, body: Record<string, unknown>) {
		const flight = this.#showing;
		if (flight === null || this.busy) return;
		this.busy = true;
		this.error = null;
		const answer = await post(`/api/${verb}`, body);
		this.busy = false;
		if (answer.error) {
			this.error = answer.error;
			return;
		}
		await this.refresh(flight);
	}
}

export const panel = new Panel();
