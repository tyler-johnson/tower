// The live board: one EventSource on /api/feed, carrying the query the
// URL holds — every frame the query's full answer, replaced wholesale —
// no client diffing. Recovery is the browser's own reconnect: a new
// subscriber immediately gets the current board, so onerror never closes
// the source.
//
// A non-default query is probed on /api/board before the source opens,
// because an EventSource cannot read the 400 envelope a bad query
// answers: it sees only a failed connection and retries forever. The
// probe reads the refusal, and the source opens only on a query the
// server accepted. The default query is `Query::default()` and cannot
// refuse, so it skips the probe and the first load stays one fold.
//
// The wire carries the web's closed window when the URL is silent:
// serve's default is the CLI's three newest, the URL elides the web's
// past day, so `withDefaultWindow` states it before either request.
//
// Named for what it exports rather than for the board, because
// `board.svelte.ts` beside `Board.svelte` is one name on a
// case-insensitive filesystem: the component's own `./board.svelte`
// import resolved to the component, and the build failed on macOS and
// Windows while passing on Linux. Nothing here may take a component's
// name in a different case.

import { get } from './api';
import { withDefaultWindow } from './query';
import type { Envelope, Folded, TowerError } from './tower';

class Feed {
	board = $state<Folded | null>(null);
	conn = $state<'connecting' | 'live' | 'reconnecting'>('connecting');
	/// The refusal for the query on the URL, from the probe; `null` while
	/// the query parses.
	error = $state<TowerError | null>(null);
	/// Last frame's arrival, in ms — the reconnecting status shows its age.
	updatedAt = $state<number | null>(null);
	/// Epoch seconds, ticked every 30s so ages stay honest between frames.
	now = $state(Math.floor(Date.now() / 1000));

	#source: EventSource | null = null;
	#ticker: ReturnType<typeof setInterval> | null = null;
	/// Bumped per connect; a probe that lost a race to a newer query
	/// neither lands nor opens a source.
	#latest = 0;

	async connect(search: string) {
		this.close();
		const token = ++this.#latest;
		const wire = withDefaultWindow(search);
		if (search !== '') {
			this.board = null;
			this.error = null;
			this.conn = 'connecting';
			const probe = await get<Folded>('/api/board?' + wire);
			if (token !== this.#latest) return;
			if (probe.error || !probe.data) {
				this.error = probe.error ?? null;
				return;
			}
			this.land(probe.data);
		}
		this.#source = new EventSource('/api/feed?' + wire);
		this.#source.onmessage = (message) => {
			const env: Envelope<Folded> = JSON.parse(message.data);
			if (env.error || !env.data) return;
			this.land(env.data);
		};
		this.#source.onerror = () => {
			this.conn = 'reconnecting';
		};
		this.#ticker = setInterval(() => {
			this.now = Math.floor(Date.now() / 1000);
		}, 30_000);
	}

	close() {
		this.#source?.close();
		this.#source = null;
		if (this.#ticker !== null) clearInterval(this.#ticker);
		this.#ticker = null;
	}

	private land(board: Folded) {
		this.board = board;
		this.updatedAt = Date.now();
		this.now = Math.floor(Date.now() / 1000);
		this.conn = 'live';
	}
}

export const feed = new Feed();
