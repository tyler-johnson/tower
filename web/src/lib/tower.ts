// The wire types and the pure render helpers, ported from
// crates/atc-cli's render.rs and cmd/brief.rs so the two stay
// comparable — one function here per function there, same names in
// camelCase, same order of phrases.

export interface Envelope<T> {
  atc: number;
  cmd: string;
  data?: T;
  error?: TowerError;
}

export interface TowerError {
  id: string;
  message: string;
  /// The `try:` hints the raise site or the registry gave — commands to
  /// run, not an exit code.
  exits: string[];
}

/// A query's answer: `/api/board?<query>` and every feed frame. The
/// groups are keyed on whatever the query grouped by — status under the
/// default query, so the keys are the six status words — and the two
/// counts are disjoint: `hidden` is what the closed window cut, `filtered`
/// what the filters rejected.
export interface Folded {
  groups: Group[];
  hidden: number;
  filtered: number;
}

/// One group of rows, and the groups nested inside it. A group holds
/// rows or subgroups, never both; `null` keys the rows carrying no value
/// for the grouped field, and the single group of an ungrouped fold.
export interface Group {
  key: string | null;
  count: number;
  rows: FlightView[];
  subgroups: Group[];
}

/// The rows of the group keyed `key`, or none when the fold has no such
/// group — an empty status group is dropped from the wire.
export function rowsOf(folded: Folded, key: string | null): FlightView[] {
  return folded.groups.find((group) => group.key === key)?.rows ?? [];
}

/// Every row the fold has: every group's rows and every subgroup's,
/// walked to the bottom, flattened and deduped by id, because grouping by
/// label deals one flight into several groups.
export function foldRows(folded: Folded): FlightView[] {
  const seen = new Map<string, FlightView>();
  const walk = (groups: Group[]) => {
    for (const group of groups) {
      for (const row of group.rows) if (!seen.has(row.id)) seen.set(row.id, row);
      walk(group.subgroups);
    }
  };
  walk(folded.groups);
  return [...seen.values()];
}

/// The flight before and the flight after `id` in the render's own order:
/// `foldRows` is that order, since `walk` takes each group's rows then its
/// subgroups and a group holds one or the other. A flight the fold does
/// not carry — closed and past the closed window — has no neighbors, and
/// the page offers no arrows.
export function neighbors(
  folded: Folded,
  id: string,
): { prev: string | null; next: string | null } {
  const rows = foldRows(folded);
  const at = rows.findIndex((row) => row.id === id);
  if (at === -1) return { prev: null, next: null };
  return {
    prev: at > 0 ? rows[at - 1].id : null,
    next: at < rows.length - 1 ? rows[at + 1].id : null,
  };
}

/// The board's own sections, the seven status words core's grouping
/// deals rows under, as themselves; a status this build has never heard
/// of names no section at all.
export function section(status: string): string | null {
  switch (status) {
    case "backlog":
    case "waiting":
    case "ready":
    case "in_progress":
    case "held":
    case "done":
    case "canceled":
      return status;
    default:
      return null;
  }
}

/// Whether a row is closed: `done` and `canceled` are two words for one
/// end, and the record keeps both.
export function closedRow(view: FlightView): boolean {
  return view.status === "done" || view.status === "canceled";
}

/// Every live row, once: the fold walked and deduped, then each row kept
/// or dropped by its own status rather than by the group it stands in,
/// so the live board is the same set under any grouping. A closed flight
/// is on the record rather than on the board.
export function liveRows(folded: Folded): FlightView[] {
  return foldRows(folded).filter((row) => !closedRow(row));
}

export interface FlightView {
  id: string;
  writer: string;
  display: string;
  /// Provenance only: the procedure the filing was minted under, or a
  /// match rule chose at file time.
  procedure: string | null;
  subject: string;
  body: string;
  filed_by: string;
  /// The filer's session, when the filing carried one: fufu's tag, or
  /// the login name at a terminal.
  filed_session: string | null;
  /// The filer's callsign, when the filing carried one.
  filed_callsign: string | null;
  filed_at: number;
  comments: number;
  depends_on: string[];
  blocks: string[];
  /// The stored status, verbatim.
  status: string;
  /// Who last moved the status, and when — `null` while the flight still
  /// stands where it was filed.
  status_by: string | null;
  /// The mover's session, when that gesture carried one.
  status_session: string | null;
  /// The mover's callsign, when that gesture carried one — the pilot.
  status_callsign: string | null;
  status_at: number | null;
  assignee: string | null;
  /// Whether the lane is the viewer's: `me`, or the viewer's own
  /// callsign — derived server-side against the process's callsign, so
  /// `for=me` reads one flag.
  mine: boolean;
  priority: string;
  labels: string[];
  skill: string | null;
  /// Closed children over total, a JSON array — Rust's `(usize, usize)`
  /// serializes as one, not as an object.
  progress: [number, number] | null;
  question: string | null;
  asked_at: number | null;
  /// A close's `-m` — a cancel's reason, most often — standing where the
  /// question stood; `null` while open or when the close said nothing.
  closed_reason: string | null;
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

/// The byline: the callsign when the event carries one, else the session,
/// else the author. A callsign renders verbatim — it is the chosen
/// readable name. A session shaped like a UUID renders as its first eight
/// characters in brackets, the short form fufu's capture summaries use;
/// anything else — a login name, a hand-typed tag — renders verbatim.
export function byline(callsign: string | null, session: string | null, by: string): string {
  if (callsign !== null) return callsign;
  if (session === null) return by;
  return isUuid(session) ? `[${session.slice(0, 8)}]` : session;
}

/// The 8-4-4-4-12 shape: 36 characters, hyphens at the four joints, hex
/// everywhere else.
function isUuid(text: string): boolean {
  return /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(text);
}

/// The priority glyph, urgent first. A word this build has never heard of
/// falls to `·` rather than being given a rung of its own — the same
/// posture `rank()` takes when it sorts an unknown priority last.
export function priorityGlyph(priority: string): string {
  switch (priority) {
    case "urgent":
      return "!";
    case "high":
      return "↑";
    case "medium":
      return "→";
    case "low":
      return "↓";
    default:
      return "·";
  }
}

/// The status dot's daisyUI modifier. A status this build has never heard
/// of gets the neutral dot: the row still renders, and the word beside it
/// is the honest answer about what the status is.
export function statusDot(status: string): string {
  switch (status) {
    case "waiting":
      return "status-warning";
    case "ready":
      return "status-info";
    case "in_progress":
      return "status-primary";
    case "held":
      return "status-error";
    case "done":
      return "status-success";
    case "backlog":
    case "canceled":
    default:
      return "status-neutral";
  }
}

/// The status as a person reads it — `in_progress` is the only stored
/// word with an underscore in it, and cmd/brief.rs spells it out the same
/// way.
export function statusWord(status: string): string {
  return status.replaceAll("_", " ");
}

/// A group's heading. `null` keys the rows with no value for the grouped
/// field; a status word reads as a person reads it. The wire does not say
/// which field grouped, so the substitution stays limited to the status
/// words the board's own sections use — only `in_progress` is changed by
/// it — and any other key prints verbatim.
export function groupTitle(key: string | null): string {
  if (key === null) return "none";
  switch (key) {
    case "backlog":
    case "waiting":
    case "ready":
    case "in_progress":
    case "held":
    case "done":
    case "canceled":
      return statusWord(key);
    default:
      return key;
  }
}

/// The right-aligned age column, on `note()`'s own precedence: the ask if
/// there is one, else the filing.
export function ageColumn(view: FlightView, now: number): string {
  if (view.asked_at !== null) return age(now, view.asked_at);
  return age(now, view.filed_at);
}

/// The subject column: the subject, then the progress mark for a flight
/// that has children. On a flat board the mark is the whole of what says
/// a row is a family.
export function subjectColumn(view: FlightView): string {
  if (view.progress === null) return view.subject;
  return `${view.subject} (${view.progress[0]}/${view.progress[1]})`;
}

/// Wire id to display form, over every row of the fold at once — the
/// walk reaches the subgroups, so a sub-grouped fold names every row.
/// Also the live flight count, for the footer: every live row once,
/// however the fold dealt it.
export function buildRefs(folded: Folded): { refs: Map<string, string>; flights: number } {
  const views = foldRows(folded);
  const refs = new Map(views.map((view) => [view.id, view.display]));
  return { refs, flights: liveRows(folded).length };
}

/// Names from the brief, including linked flights outside the board's window.
export function linkRefs(brief: Brief): Map<string, string> {
  const links = [...brief.blocks, ...brief.depends_on, ...brief.references, ...brief.referenced_by];
  return new Map([
    [brief.id, brief.display],
    ...links.map((link) => [link.flight, link.display] as [string, string]),
  ]);
}

// The core's tokenizer, for plain text: a run of token chars, the head
// trimmed of its trailing non-alphanumerics, and the `#<writer>.<seq>`
// shape alone.
const RUN = /[A-Za-z0-9_#~.-]+/g;
const WIRE = /^#([A-Za-z0-9_.-]+\.\d+)$/;

/// Render-time, plain text: every `#<wire id>` `refs` knows becomes what
/// `refs` says, and the rest stays — refs.rs's `project`, for the note
/// phrases the page prints as text rather than markdown.
export function project(text: string, refs: Map<string, string>): string {
  return text.replace(RUN, (run) => {
    const head = run.replace(/[^A-Za-z0-9]+$/, "");
    const id = WIRE.exec(head)?.[1];
    const shown = id === undefined ? undefined : refs.get(id);
    return shown === undefined ? run : shown + run.slice(head.length);
  });
}

export interface NotePhrase {
  text: string;
  tone: "warn" | "dim";
}

/// The note line's phrases, in render.rs's urgency order: question, the
/// pilot, comments.
///
/// The one deliberate divergence from `note()`: the trailing age phrase is
/// omitted, because the web row has a column for the age and the CLI has
/// no room for one. `ageColumn` is that phrase's other half. The
/// question and a close's reason are prose, projected through `refs`.
export function notePhrases(view: FlightView, refs: Map<string, string>): NotePhrase[] {
  const phrases: NotePhrase[] = [];
  const warn = (text: string) => phrases.push({ text, tone: "warn" });
  const dim = (text: string) => phrases.push({ text, tone: "dim" });
  if (view.question !== null) warn(project(view.question, refs));
  else if (view.closed_reason !== null) dim(project(view.closed_reason, refs));
  // The pilot: the stored In Progress and who set it — the byline and
  // its session are the pilot, the field is the chip.
  if (view.status === "in_progress") {
    dim(
      view.status_by !== null
        ? `in progress — ${byline(view.status_callsign, view.status_session, view.status_by)}`
        : "in progress",
    );
  }
  if (view.comments > 0) {
    dim(`${view.comments} ${view.comments === 1 ? "comment" : "comments"}`);
  }
  return phrases;
}

/// Where one flight stands, `Standing`'s tag — flattened onto the brief
/// beside the facts it arbitrates. Every variant is a bare tag.
export type StandingTag = "done" | "question" | "in-progress" | "yours" | "ready";

/// One linked flight, as the brief carries it. `status` is the stored
/// word; `closed` is the arbitrated fact, since done and canceled are two
/// words for one end.
export interface LinkView {
  flight: string;
  writer: string;
  display: string;
  subject: string;
  status: string;
  closed: boolean;
}

/// A parent, as a sub-flight's brief carries it: a link row plus the
/// body — the flight's real context, one level up and no further.
export interface ParentView extends LinkView {
  body: string;
}

/// A note on the record. `id` is the wire id — a comment's only name, and
/// what `edit` takes.
export interface CommentView {
  id: string;
  author: string;
  /// The session behind the author, when the event carried one.
  session: string | null;
  /// The pilot, when the event carried a callsign.
  callsign: string | null;
  at: number;
  text: string;
  /// Flagged as the state of play; the brief pins the newest.
  handoff: boolean;
}

/// One gesture on the record: who did what, when, and the words the verb
/// took, flat beside `what` and present only where the kind carries them.
/// Deliberately thin — the subject, the body, and a comment's text already
/// sit elsewhere on the brief. A hold and an answer are the exception: an
/// open question sits flat on the brief, but a resolved one and the words
/// that closed it sit nowhere else at all.
export interface Moment {
  id: string;
  at: number;
  by: string;
  /// The session behind the author, when the event carried one: fufu's
  /// tag, or the login name at a terminal.
  session: string | null;
  /// The pilot, when the event carried a callsign — what the byline
  /// prints first.
  callsign: string | null;
  what: string;
  /// `status`: the word used, verbatim; `reason` a cancel's `-m`.
  status?: string;
  reason?: string;
  /// `assigned`: the lane; `null` is the clearing.
  assignee?: string | null;
  /// `edited`: the fields touched; `comment` the comment's event id when
  /// the target was a comment rather than the flight.
  fields?: string[];
  comment?: string;
  /// `linked` and `unlinked`: both ends, wire ids, `from` depends on `to`.
  from?: string;
  to?: string;
  /// `held` and `answered`: the words each carried — the only copy once
  /// the hold is over.
  question?: string;
  answer?: string;
  /// `routed`: which procedure and rule fired, and why.
  procedure?: string;
  rule?: string;
  because?: string;
}

/// The words after a moment's verb — a leading space and the words, or
/// `''` when the kind carries none — and the free text that follows on its
/// own line: a move's reason, a routing's because. Link endpoints print as
/// wire ids: the page has no number map.
///
/// The one divergence from `phrase()`: a hold and an answer render their
/// words here as the note. The CLI's arms are inert because it prints the
/// open question in the note line and keeps comments and history apart;
/// the page blends the two into one stream, where a question with no words
/// is a gap.
export function momentPhrase(moment: Moment, briefId: string): { line: string; note?: string } {
  switch (moment.what) {
    case "status":
      return moment.status === undefined
        ? { line: "" }
        : { line: ` ${moment.status}`, note: moment.reason };
    case "assigned":
      return moment.assignee === undefined
        ? { line: "" }
        : { line: ` ${moment.assignee ?? "none"}` };
    case "edited":
      if (moment.comment !== undefined) return { line: ` comment ${moment.comment}` };
      return moment.fields === undefined ? { line: "" } : { line: ` ${moment.fields.join(", ")}` };
    case "linked":
    case "unlinked":
      if (moment.from === undefined || moment.to === undefined) return { line: "" };
      return moment.from === briefId
        ? { line: ` depends on ${moment.to}` }
        : { line: ` blocks ${moment.from}` };
    case "held":
      return moment.question === undefined ? { line: "" } : { line: "", note: moment.question };
    case "answered":
      return moment.answer === undefined ? { line: "" } : { line: "", note: moment.answer };
    case "routed":
      return moment.procedure === undefined
        ? { line: "" }
        : { line: ` ${moment.procedure}`, note: moment.because || undefined };
    default:
      return { line: "" };
  }
}

export interface Brief {
  id: string;
  writer: string;
  display: string;
  procedure: string | null;
  subject: string;
  body: string;
  filed_by: string;
  filed_session: string | null;
  filed_callsign: string | null;
  filed_at: number;
  /// The stored fields, read here because the brief is the read surface
  /// for one flight.
  status: string;
  status_by: string | null;
  status_session: string | null;
  status_callsign: string | null;
  status_at: number | null;
  /// A cancel's `-m`, or the closing of a dependency that moved this
  /// flight — the words behind the move, which nothing else carries.
  status_reason: string | null;
  assignee: string | null;
  priority: string;
  labels: string[];
  skill: string | null;
  /// The last edit touching the record — the flight's own fields or a
  /// comment's text — flat like the status mark.
  edited_by: string | null;
  edited_at: number | null;
  question: string | null;
  asked_by: string | null;
  asked_at: number | null;
  closed_reason: string | null;
  /// The newest comment flagged as the state of play, pinned above the
  /// stream that still holds it; `null` when none carries the flag.
  handoff: CommentView | null;
  progress: [number, number] | null;
  depends_on: LinkView[];
  blocks: LinkView[];
  /// The `blocks` rows again, each with its body.
  parents: ParentView[];
  /// The flights this flight's prose names, as link rows — the number
  /// map for its references.
  references: LinkView[];
  /// The backlinks: the flights whose prose names this one.
  referenced_by: LinkView[];
  comments: CommentView[];
  history: Moment[];
  standing: StandingTag;
}

/// One procedure as the registry holds it, mirroring
/// atc-core/src/procedure/mod.rs. This is the *definition* — a file
/// on disk, and nothing a flight carries: a filing keeps only the
/// procedure's name, as provenance.
export interface Definition {
  name: string;
  matches: ProcedureMatch[];
  flights: FlightDef[];
  source: Source;
}

/// One intake rule: a name — what the routing event records as having
/// fired — and the predicates, which all AND; an absent predicate is
/// null. Named ProcedureMatch to avoid the DOM's Match.
export interface ProcedureMatch {
  name: string;
  source: string | null;
  event: string | null;
  label: string | null;
  priority: string | null;
  skill: string | null;
  assignee: string | null;
  status: string | null;
}

/// One flight a definition declares. `done` stays a free string: a newer
/// tower's completion word must not fail an older tower's parse.
export interface FlightDef {
  id: string;
  assignee: "me" | "agent";
  skill: string | null;
  after: string[];
  done: string;
  /// The priority and labels the flight is born with — free here because
  /// they are free on the flight.
  priority: string | null;
  labels: string[];
}

/// Which layer a definition was read from, and the file it came from.
/// Both layers are directories, so every definition has a path.
export interface Source {
  layer: "user" | "repo";
  path: string;
}

export interface Listing {
  procedures: Definition[];
}

/// The brief's note line, ported from cmd/brief.rs's `note()`: the status
/// ahead of everything, because a reader must know first where the flight
/// stands and who put it there, then the question, the standing, and the
/// age. Precedence makes the standing exclusive with the mark phrases,
/// so the line never says a thing twice. The prose phrases — the since
/// line, the question, a close's reason — project through `refs`.
export function briefNote(brief: Brief, now: number, refs: Map<string, string>): NotePhrase[] {
  const phrases: NotePhrase[] = [];
  const warn = (text: string) => phrases.push({ text, tone: "warn" });
  const dim = (text: string) => phrases.push({ text, tone: "dim" });
  const status = statusWord(brief.status);
  dim(
    brief.status_by !== null && brief.status_at !== null
      ? `${status} — ${byline(brief.status_callsign, brief.status_session, brief.status_by)} ${age(now, brief.status_at)}`
      : status,
  );
  if (brief.status_reason !== null) dim(project(brief.status_reason, refs));
  if (brief.question !== null) warn(project(brief.question, refs));
  else if (brief.closed_reason !== null) dim(project(brief.closed_reason, refs));
  switch (brief.standing) {
    // Said above, from the brief's own flat facts.
    case "done":
    case "question":
    case "in-progress":
      break;
    case "yours":
      dim(brief.assignee !== null ? `yours — assigned ${brief.assignee}` : "yours — unassigned");
      break;
    case "ready":
      dim("ready");
      break;
  }
  if (brief.asked_at !== null) dim(`asked ${age(now, brief.asked_at)}`);
  else dim(`filed ${age(now, brief.filed_at)}`);
  return phrases;
}

/// The stored fields, one line, ported from cmd/brief.rs's `fields_line()`:
/// lane, priority, labels, skill, and the procedure the filing was
/// minted under. Its own line rather than phrases in the note — the note
/// is urgency ordered, and a field is not urgency.
export function fieldsLine(brief: Brief): string {
  const phrases = [brief.assignee !== null ? `assignee ${brief.assignee}` : "unassigned"];
  if (brief.priority !== "none") phrases.push(`priority ${brief.priority}`);
  if (brief.labels.length > 0) phrases.push(brief.labels.join(", "));
  if (brief.skill !== null) phrases.push(`skill ${brief.skill}`);
  if (brief.procedure !== null) phrases.push(`under ${brief.procedure}`);
  return phrases.join(" · ");
}

/// A refusal as lines, in main.rs's `report()` shape minus the
/// `atc:` prefix — a terminal artifact, and this is not a terminal.
export function refusalLines(error: TowerError): string[] {
  const lines = [error.message];
  if (error.exits.length > 0) {
    lines.push("  try:");
    for (const hint of error.exits) lines.push(`    ${hint}`);
  }
  return lines;
}

export type Verb = "assign" | "status" | "hold" | "answer" | "done" | "cancel" | "comment";

/// The verbs this flight's state accepts, from the guards in
/// atc-core/src/verb/.
///
/// A closed flight is what `ensure_active` refuses on, so it keeps only
/// `comment` — a note on a closed record is fine, and comment.rs runs no
/// `ensure_active` for exactly that reason. An open question closes two
/// more: `status` refuses with `status/held` for any target but done or
/// canceled, and `hold` refuses with `hold/exists`. `assign` re-lanes a
/// held flight freely, which is how a question gets handed to someone.
///
/// Derived from a fold that may be a frame stale, so this decides what to
/// offer and never what is allowed: the server's refusal is still the
/// word that counts.
export function allowedVerbs(brief: Brief): Verb[] {
  if (brief.status === "done" || brief.status === "canceled") return ["comment"];
  if (brief.question !== null) return ["assign", "answer", "done", "cancel", "comment"];
  return ["assign", "status", "hold", "done", "cancel", "comment"];
}

/// Every top-level key this build does not know, as labelled rows.
///
/// A newer tower's brief carries fields this page has never heard of, and
/// showing them badly beats dropping them silently — the same promise
/// `Kind::Unknown` makes the fold.
const KNOWN_BRIEF_KEYS = new Set([
  "id",
  "writer",
  "display",
  "procedure",
  "subject",
  "body",
  "filed_by",
  "filed_session",
  "filed_callsign",
  "filed_at",
  "status",
  "status_by",
  "status_session",
  "status_callsign",
  "status_at",
  "status_reason",
  "assignee",
  "priority",
  "labels",
  "skill",
  "edited_by",
  "edited_at",
  "question",
  "asked_by",
  "asked_at",
  "closed_reason",
  "handoff",
  "progress",
  "depends_on",
  "blocks",
  "parents",
  "references",
  "referenced_by",
  "comments",
  "history",
  "standing",
]);

export function unknownRows(brief: Brief): { label: string; value: string }[] {
  return Object.entries(brief as unknown as Record<string, unknown>)
    .filter(([key]) => !KNOWN_BRIEF_KEYS.has(key))
    .map(([label, value]) => ({
      label,
      // A scalar reads as itself; anything else is shown as the JSON
      // it arrived as, which is at least honest about its shape.
      value: value === null || typeof value !== "object" ? String(value) : JSON.stringify(value),
    }));
}
