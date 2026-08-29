// The wire types and the pure render helpers, ported from
// crates/ff-tower-cli/src/render.rs so the two stay comparable — one
// function here per function there, same names in camelCase, same order
// of phrases.

export interface Envelope<T> {
	tower: number;
	cmd: string;
	data?: T;
	error?: TowerError;
}

export interface TowerError {
	id: string;
	message: string;
	exits: number;
}

export interface Board {
	waiting_on_you: FlightView[];
	in_the_air: FlightView[];
	holding: FlightView[];
	open: FlightView[];
	unrouted: TowerEvent[];
}

export interface FlightView {
	id: string;
	number: number;
	procedure: string;
	part: PartStamp | null;
	subject: string;
	filed_by: string;
	filed_at: number;
	comments: number;
	depends_on: string[];
	blocks: string[];
	branch: string | null;
	tip: string | null;
	last_motion: number | null;
	held: boolean;
	resolving: boolean;
	current: boolean;
	claimed_by: string | null;
	taken: boolean;
	requeued_at: number | null;
	question: string | null;
	asked_at: number | null;
	collides: CollideView[];
	unanswered: string[];
}

export interface CollideView {
	with: string;
	paths: string[];
}

export interface PartStamp {
	id: string;
	crew: string;
	skill?: string;
	done: boolean;
	bay?: string;
	branch?: string;
}

// The unrouted rows are raw log events; the board renders only their count,
// so the shape stays loose. Named TowerEvent to avoid the DOM's Event.
export interface TowerEvent {
	writer: string;
	author: string;
	time: number;
	id: string;
	kind: unknown;
}

/// The writer half of a wire id — everything before the last `.`.
export function writerOf(id: string): string {
	const dot = id.lastIndexOf('.');
	return dot === -1 ? id : id.slice(0, dot);
}

/// Whether the given full ids span at most one writer, so `#<n>` alone
/// names a flight unambiguously.
export function shortIds(ids: string[]): boolean {
	const writers = ids.map(writerOf);
	return writers.every((writer) => writer === writers[0]);
}

/// The display form of a flight's number: `#3` when short, `pi-8c2e#3`
/// otherwise — the long form takes no leading `#`.
export function flightRef(writer: string, number: number, short: boolean): string {
	return short ? `#${number}` : `${writer}#${number}`;
}

/// The collide path phrase: the one path, or a count.
export function pathsPhrase(paths: string[]): string {
	return paths.length === 1 ? paths[0] : `${paths.length} paths`;
}

/// `4m ago`, `2d ago` — s/m/h/d/w. `now` is an argument so a render is a
/// pure function of its inputs.
export function age(now: number, then: number): string {
	const delta = Math.max(now - then, 0);
	if (delta < 60) return `${delta}s ago`;
	if (delta < 3_600) return `${Math.floor(delta / 60)}m ago`;
	if (delta < 86_400) return `${Math.floor(delta / 3_600)}h ago`;
	if (delta < 604_800) return `${Math.floor(delta / 86_400)}d ago`;
	return `${Math.floor(delta / 604_800)}w ago`;
}

/// The tip column: the branch tip short, `—` for a flight with no tip, and
/// the literal `(detached)` for `@detached` — printing the sentinel as a
/// branch name would read as a real branch.
export function tipColumn(view: FlightView): string {
	if (view.branch === '@detached') return '(detached)';
	return view.tip ? view.tip.slice(0, 8) : '—';
}

/// Wire id to display form, over every section at once: a verdict partner
/// is always a live flight, so the map answers for `collides` and
/// `unanswered` entries too. Also the total flight count, for the footer.
export function buildRefs(board: Board): { refs: Map<string, string>; flights: number } {
	const views = [board.waiting_on_you, board.in_the_air, board.holding, board.open].flat();
	const short = shortIds(views.map((view) => view.id));
	const refs = new Map(
		views.map((view) => [view.id, flightRef(writerOf(view.id), view.number, short)])
	);
	return { refs, flights: views.length };
}

export interface NotePhrase {
	text: string;
	tone: 'warn' | 'dim';
}

/// The note line's phrases, in render.rs's urgency order: question, held,
/// resolving, collides, no-verdicts, waiting-on, ownership, branch,
/// comments, age. The age phrase means the line is never empty.
export function notePhrases(
	view: FlightView,
	refs: Map<string, string>,
	now: number
): NotePhrase[] {
	const phrases: NotePhrase[] = [];
	const warn = (text: string) => phrases.push({ text, tone: 'warn' });
	const dim = (text: string) => phrases.push({ text, tone: 'dim' });
	if (view.question !== null) warn(view.question);
	if (view.held) warn('held');
	if (view.resolving) warn('resolving');
	for (const collide of view.collides) {
		warn(`collides ${refs.get(collide.with)} on ${pathsPhrase(collide.paths)}`);
	}
	for (const with_ of view.unanswered) {
		dim(`no verdict vs ${refs.get(with_)}`);
	}
	// A dependency absent from `refs` is done — done flights leave the
	// board — so the phrase covers only the live ones and clears itself as
	// they land.
	const waiting = view.depends_on.filter((dep) => refs.has(dep));
	if (waiting.length === 1) dim(`waiting on ${refs.get(waiting[0])}`);
	else if (waiting.length > 1) dim(`waiting on ${waiting.length} flights`);
	// Ownership, ahead of the branch. `taken` prints whether or not a
	// branch exists; a bare `claimed` needs no branch; `requeued` stands
	// where neither does.
	if (view.taken) dim('taken');
	else if (view.claimed_by !== null) {
		if (view.branch === null) dim('claimed');
	} else if (view.requeued_at !== null) dim('requeued');
	if (view.branch !== null && view.branch !== '@detached') dim(`on ${view.branch}`);
	if (view.comments > 0) {
		dim(`${view.comments} ${view.comments === 1 ? 'comment' : 'comments'}`);
	}
	if (view.asked_at !== null) dim(`asked ${age(now, view.asked_at)}`);
	else if (view.last_motion !== null) dim(`moved ${age(now, view.last_motion)}`);
	else dim(`filed ${age(now, view.filed_at)}`);
	return phrases;
}
