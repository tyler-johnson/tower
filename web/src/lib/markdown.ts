// The render behind every authored prose the page shows: a body, a
// comment, the free text a gesture carried. Everything writing those
// fields writes markdown already — headings, lists, fenced code,
// backticked `path:line` — and the page used to print the asterisks.
//
// `html: false` is the whole sanitizer: no raw HTML reaches the parser,
// so a `<script>` in a body comes back as text and there is no second
// dependency to track.
//
// No runes, so it tests under vitest with no shims.

import MarkdownIt from "markdown-it";

const md = new MarkdownIt({ html: false, linkify: true, typographer: false });

// Every anchor leaves the board: a link in a body points somewhere else,
// and the page under it is a record the reader is still on. The default
// rule is chained rather than replaced, so the renderer keeps whatever
// else it does with a `link_open`.
const link = md.renderer.rules.link_open;
md.renderer.rules.link_open = (tokens, i, options, env, self) => {
  tokens[i].attrSet("target", "_blank");
  tokens[i].attrSet("rel", "noopener noreferrer");
  return link ? link(tokens, i, options, env, self) : self.renderToken(tokens, i, options);
};

/// One text as HTML. Empty or whitespace-only renders to the empty
/// string, so a caller's own placeholder still shows.
export function render(text: string): string {
  if (text.trim() === "") return "";
  return md.render(text);
}
