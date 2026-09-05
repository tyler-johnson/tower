// The record's one stream: what happened and what was said, in one
// column, in one order.
//
// The brief carries two lists — `comments` and `history` — and the CLI
// prints them as two sections. On a page they read as one: the history is
// already every gesture, oldest first, and a `commented` moment is the
// same event as the comment it names, so the two lists are one list the
// wire happened to split. The walk is the history's, and each moment is
// given its words: a comment's text from `comments`, a hold's question and
// an answer's answer from the moment itself.
//
// No runes, so it tests under vitest with no shims.

import { momentPhrase, type Brief, type CommentView } from "./tower";

/// One row of the stream. A comment is a block with an author line; a
/// gesture is one dim line, with the free text it carried under it — a
/// cancel's reason, a routing's because, a hold's question.
export type Entry =
  | {
      kind: "comment";
      id: string;
      at: number;
      by: string;
      session: string | null;
      text: string;
    }
  | {
      kind: "gesture";
      id: string;
      at: number;
      by: string;
      /// The session behind the byline, when the event carried one.
      session: string | null;
      what: string;
      line: string;
      note: string | null;
    };

export function stream(brief: Brief): Entry[] {
  const comments = new Map(brief.comments.map((comment) => [comment.id, comment]));
  const paired = new Set<string>();
  const entries: Entry[] = [];
  for (const moment of brief.history) {
    // A comment's moment and the comment are one event, so the moment's
    // id is the comment's name.
    const comment = moment.what === "commented" ? comments.get(moment.id) : undefined;
    if (comment) {
      paired.add(comment.id);
      entries.push(note(comment));
      continue;
    }
    const phrase = momentPhrase(moment, brief.id);
    entries.push({
      kind: "gesture",
      id: moment.id,
      at: moment.at,
      by: moment.by,
      session: moment.session,
      what: moment.what,
      line: phrase.line,
      note: phrase.note ?? null,
    });
  }
  // A comment the history did not name — the two lists disagree, and
  // dropping the words would be the worse answer.
  for (const comment of brief.comments) {
    if (paired.has(comment.id)) continue;
    entries.push(note(comment));
  }
  // Stable, so moments sharing a second keep the log's order and only
  // the unpaired comments move to where they belong.
  return entries.sort((a, b) => a.at - b.at);
}

function note(comment: CommentView): Entry {
  return {
    kind: "comment",
    id: comment.id,
    at: comment.at,
    by: comment.author,
    session: comment.session,
    text: comment.text,
  };
}
