// The render behind every authored prose the page shows: a body, a
// comment, the free text a gesture carried. Everything writing those
// fields writes markdown already — headings, lists, fenced code,
// backticked `path:line` — and the page used to print the asterisks.
//
// `html: false` is the whole sanitizer: no raw HTML reaches the parser,
// so a `<script>` in a body comes back as text and there is no second
// dependency to track.
//
// A flight named in prose is stored as `#<wire id>`, and the page
// renders it as a link to the flight, with the current display form as
// its text. The scan is the core's run tokenizer, over the inline text
// tokens alone: a wire id inside a code span, a fence, or a link the
// writer made is left as written.
//
// No runes, so it tests under vitest with no shims.

import MarkdownIt, { type StateCore, type Token } from "markdown-it";

const md = new MarkdownIt({ html: false, linkify: true, typographer: false });

/// What one render knows: the display form per wire id, and where a
/// flight's page is.
interface Refs {
  refs: Map<string, string>;
  href: (id: string) => string;
}

// The core's tokenizer: a run of token chars, the head trimmed of its
// trailing non-alphanumerics, and the `#<writer>.<seq>` shape alone.
const RUN = /[A-Za-z0-9_#~.-]+/g;
const WIRE = /^#([A-Za-z0-9_.-]+\.\d+)$/;

/// A text token split around every wire id it carries: a link for each,
/// text for the rest. The text unchanged when it carries none.
function split(state: StateCore, text: Token, env: Refs): Token[] {
  const out: Token[] = [];
  const content = text.content;
  let from = 0;
  const plain = (to: number) => {
    if (to === from) return;
    const token = new state.Token("text", "", 0);
    token.content = content.slice(from, to);
    out.push(token);
  };
  for (const run of content.matchAll(RUN)) {
    const at = run.index;
    const head = run[0].replace(/[^A-Za-z0-9]+$/, "");
    const id = WIRE.exec(head)?.[1];
    if (id === undefined) continue;
    plain(at);
    const open = new state.Token("link_open", "a", 1);
    open.attrSet("href", env.href(id));
    open.attrSet("data-flight", id);
    const label = new state.Token("text", "", 0);
    label.content = env.refs.get(id) ?? head;
    const close = new state.Token("link_close", "a", -1);
    out.push(open, label, close);
    from = at + head.length;
  }
  if (out.length === 0) return [text];
  plain(content.length);
  return out;
}

// After every other core rule, so the text tokens are joined and the
// bare urls are already anchors: inside a link the writer or linkify
// made, nothing is scanned.
md.core.ruler.push("flight_refs", (state) => {
  const env = state.env as Partial<Refs>;
  const full: Refs = { refs: env.refs ?? new Map(), href: env.href ?? ((id) => `/f/${id}`) };
  for (const token of state.tokens) {
    if (token.type !== "inline" || token.children === null) continue;
    const children: Token[] = [];
    let inLink = 0;
    for (const child of token.children) {
      if (child.type === "link_open") inLink += 1;
      else if (child.type === "link_close") inLink -= 1;
      if (child.type === "text" && inLink === 0) children.push(...split(state, child, full));
      else children.push(child);
    }
    token.children = children;
  }
});

// Every anchor leaves the board, except a flight's: a link in a body
// points somewhere else, and the page under it is a record the reader
// is still on — while a flight link stays in the app, and the router
// takes it. The default rule is chained rather than replaced, so the
// renderer keeps whatever else it does with a `link_open`.
const link = md.renderer.rules.link_open;
md.renderer.rules.link_open = (tokens, i, options, env, self) => {
  if (tokens[i].attrGet("data-flight") === null) {
    tokens[i].attrSet("target", "_blank");
    tokens[i].attrSet("rel", "noopener noreferrer");
  }
  return link ? link(tokens, i, options, env, self) : self.renderToken(tokens, i, options);
};

/// One text as HTML. Empty or whitespace-only renders to the empty
/// string, so a caller's own placeholder still shows. `refs` is the
/// display form per wire id — a reference it lacks links with the wire
/// id as its text — and `href` is where a flight's page is, the bare
/// path when unsaid.
export function render(
  text: string,
  refs?: Map<string, string>,
  href?: (id: string) => string,
): string {
  if (text.trim() === "") return "";
  return md.render(text, { refs, href });
}
