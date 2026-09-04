// What a body and a comment come out as: the markdown a writer meant,
// every anchor leaving the board, and nothing a writer typed reaching
// the page as markup.

import { describe, expect, it } from "vitest";
import { render } from "./markdown";

describe("the markdown render", () => {
  it("makes a heading a heading", () => {
    expect(render("# the work")).toContain("<h1>the work</h1>");
  });

  it("makes a dashed run a list", () => {
    const html = render("- one\n- two");
    expect(html).toContain("<ul>");
    expect(html).toContain("<li>one</li>");
    expect(html).toContain("<li>two</li>");
  });

  it("makes a numbered run an ordered list", () => {
    expect(render("1. one\n2. two")).toContain("<ol>");
  });

  it("makes a fence a code block", () => {
    const html = render("```sh\nff tower brief\n```");
    expect(html).toContain("<pre>");
    expect(html).toContain("<code");
    expect(html).toContain("ff tower brief");
  });

  it("makes backticks inline code", () => {
    expect(render("see `query.ts:214`")).toContain("<code>query.ts:214</code>");
  });

  it("gives a link the target and the rel", () => {
    const html = render("[the board](https://example.com/b)");
    expect(html).toContain('href="https://example.com/b"');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });

  it("makes a bare url an anchor, carrying the same target and rel", () => {
    const html = render("filed at https://example.com/f/9 today");
    expect(html).toContain('href="https://example.com/f/9"');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });

  it("makes a piped block a table", () => {
    const html = render("| a | b |\n| --- | --- |\n| 1 | 2 |");
    expect(html).toContain("<table>");
    expect(html).toContain("<th>a</th>");
    expect(html).toContain("<td>1</td>");
  });

  it("makes a doubled tilde a strikethrough", () => {
    expect(render("~~gone~~")).toContain("<s>gone</s>");
  });

  it("escapes a script tag rather than passing it through", () => {
    const html = render("<script>alert(1)</script>");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });

  // The handler survives as the text of the escaped tag, which is the
  // point: there is no element for it to hang on.
  it("escapes a raw img tag and its handler", () => {
    const html = render('<img src="x" onerror="alert(1)">');
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img src=&quot;x&quot; onerror=&quot;alert(1)&quot;&gt;");
  });

  it("makes a markdown image an img element", () => {
    const html = render("![the board](https://example.com/b.png)");
    expect(html).toContain('<img src="https://example.com/b.png"');
    expect(html).toContain('alt="the board"');
  });

  it("renders empty text as the empty string", () => {
    expect(render("")).toBe("");
  });

  it("renders whitespace-only text as the empty string", () => {
    expect(render("  \n\t\n ")).toBe("");
  });
});
