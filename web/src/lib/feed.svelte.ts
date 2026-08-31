// The live board: one EventSource on /api/feed, every frame a full board
// envelope, replaced wholesale — no client diffing. Recovery is the
// browser's own reconnect: a new subscriber immediately gets the current
// board, so onerror never closes the source.
//
// Named for what it exports rather than for the board, because
// `board.svelte.ts` beside `Board.svelte` is one name on a
// case-insensitive filesystem: the component's own `./board.svelte`
// import resolved to the component, and the build failed on macOS and
// Windows while passing on Linux. Nothing here may take a component's
// name in a different case.

import type { Board, Envelope } from './tower';

class Feed {
	board = $state<Board | null>(null);
	conn = $state<'connecting' | 'live' | 'reconnecting'>('connecting');
	/// Last frame's arrival, in ms — the reconnecting status shows its age.
	updatedAt = $state<number | null>(null);
	/// Epoch seconds, ticked every 30s so ages stay honest between frames.
	now = $state(Math.floor(Date.now() / 1000));

	#source: EventSource | null = null;
	#ticker: ReturnType<typeof setInterval> | null = null;

	connect() {
		if (this.#source) return;
		this.#source = new EventSource('/api/feed');
		this.#source.onmessage = (message) => {
			const env: Envelope<Board> = JSON.parse(message.data);
			if (env.error || !env.data) return;
			this.board = env.data;
			this.updatedAt = Date.now();
			this.now = Math.floor(Date.now() / 1000);
			this.conn = 'live';
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
}

export const feed = new Feed();
