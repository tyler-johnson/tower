// The fold's counting: a flight dealt into several groups, or nested
// under a subgroup, is still one flight to the ref map and the footer.

import { describe, expect, it } from "vitest";
import {
  buildRefs,
  byline,
  foldRows,
  linkRefs,
  liveRows,
  neighbors,
  project,
  type Brief,
  type FlightView,
  type Folded,
  type Group,
  type LinkView,
} from "./tower";

function flight(number: number, status: string, labels: string[] = []): FlightView {
  return {
    id: `pi-8c2e.${number}`,
    number,
    procedure: null,
    subject: `flight ${number}`,
    body: "",
    filed_by: "tyler",
    filed_session: null,
    filed_callsign: null,
    filed_at: 0,
    comments: 0,
    depends_on: [],
    blocks: [],
    status,
    status_by: null,
    status_session: null,
    status_callsign: null,
    status_at: null,
    assignee: null,
    mine: false,
    priority: "none",
    labels,
    skill: null,
    progress: null,
    question: null,
    asked_at: null,
    closed_reason: null,
  };
}

function group(key: string | null, rows: FlightView[], subgroups: Group[] = []): Group {
  return { key, count: rows.length, rows, subgroups };
}

function link(id: string, number: number, closed = false): LinkView {
  return {
    flight: id,
    number,
    subject: `flight ${number}`,
    status: closed ? "done" : "ready",
    closed,
  };
}

function brief(
  number: number,
  depends_on: LinkView[],
  blocks: LinkView[] = [],
  references: LinkView[] = [],
  referenced_by: LinkView[] = [],
): Brief {
  return {
    ...flight(number, "ready"),
    status_reason: null,
    edited_by: null,
    edited_at: null,
    asked_by: null,
    handoff: null,
    depends_on,
    blocks,
    parents: [],
    references,
    referenced_by,
    comments: [],
    history: [],
    standing: "ready",
  };
}

describe("the byline", () => {
  it("shortens a UUID session to its first eight characters in brackets", () => {
    expect(byline(null, "95b36d9d-efdc-4564-9b06-91842f51ef6b", "a@b.c")).toBe("[95b36d9d]");
  });
  it("renders any other session verbatim", () => {
    expect(byline(null, "tyler", "a@b.c")).toBe("tyler");
    expect(byline(null, "hand-typed", "a@b.c")).toBe("hand-typed");
    expect(byline(null, "95b36d9d-efdc-4564-9b06-91842f51ef6", "a@b.c")).toBe(
      "95b36d9d-efdc-4564-9b06-91842f51ef6",
    );
  });
  it("falls back to the author with no session", () => {
    expect(byline(null, null, "a@b.c")).toBe("a@b.c");
  });
  it("the callsign wins over the session and the author", () => {
    expect(byline("claude", "95b36d9d-efdc-4564-9b06-91842f51ef6b", "a@b.c")).toBe("claude");
    expect(byline("claude", null, "a@b.c")).toBe("claude");
  });
});

describe("the fold", () => {
  it("a fold is counted once however it is grouped", () => {
    const both = flight(1, "ready", ["web", "ui"]);
    const one = flight(2, "in_progress", ["web"]);
    const done = flight(3, "done", ["ui"]);
    const canceled = flight(4, "canceled", ["web"]);
    const byLabel: Folded = {
      groups: [group("web", [both, one, canceled]), group("ui", [both, done])],
      hidden: 0,
      filtered: 0,
    };
    expect(foldRows(byLabel)).toHaveLength(4);
    expect(liveRows(byLabel)).toHaveLength(2);
    expect(buildRefs(byLabel).flights).toBe(2);
    expect(buildRefs(byLabel).refs.get(done.id)).toBe("#3");

    const nested: Folded = {
      groups: [
        group("tyler", [], [group("high", [both]), group("none", [one])]),
        group(null, [], [group("none", [done, canceled])]),
      ],
      hidden: 0,
      filtered: 0,
    };
    expect(foldRows(nested)).toHaveLength(4);
    expect(liveRows(nested)).toHaveLength(2);
    expect(buildRefs(nested).flights).toBe(2);
    expect(buildRefs(nested).refs.size).toBe(4);
  });
});

describe("the link refs", () => {
  it("a child gone from the board still takes its number", () => {
    // The child closed past the board's window: no row in the fold, so
    // the board's ref map has nothing for it, and the brief's number is
    // the whole of what names it.
    const ids = ["pi-8c2e.1", "pi-8c2e.2"];
    const refs = linkRefs(ids, brief(1, [link("pi-8c2e.2", 2), link("pi-8c2e.7", 7, true)]));
    expect(refs.get("pi-8c2e.2")).toBe("#2");
    expect(refs.get("pi-8c2e.7")).toBe("#7");
  });

  it("a link from a second writer makes every ref long", () => {
    const ids = ["pi-8c2e.1"];
    const refs = linkRefs(ids, brief(1, [link("pi-8c2e.2", 2)], [link("mac-1f00.3", 3)]));
    expect(refs.get("pi-8c2e.2")).toBe("pi-8c2e#2");
    expect(refs.get("mac-1f00.3")).toBe("mac-1f00#3");
  });

  it("a referenced flight names itself the same way, both directions", () => {
    const ids = ["pi-8c2e.1"];
    const refs = linkRefs(
      ids,
      brief(1, [], [], [link("pi-8c2e.4", 4, true)], [link("pi-8c2e.6", 6)]),
    );
    expect(refs.get("pi-8c2e.4")).toBe("#4");
    expect(refs.get("pi-8c2e.6")).toBe("#6");
  });
});

describe("the projection", () => {
  const refs = new Map([
    ["pi-8c2e.2", "#2"],
    ["mac-1f00.3", "mac-1f00#3"],
  ]);

  it("puts the display form back and keeps the sentence's punctuation", () => {
    expect(project("blocked on #pi-8c2e.2.", refs)).toBe("blocked on #2.");
    expect(project("#pi-8c2e.2's test (#mac-1f00.3)", refs)).toBe("#2's test (mac-1f00#3)");
  });

  it("leaves an unknown id, a bare number, and other hashes alone", () => {
    const text = "see #pi-8c2e.9, #3, #ff0000, C#";
    expect(project(text, refs)).toBe(text);
  });

  it("leaves text with no reference alone", () => {
    expect(project("", refs)).toBe("");
    expect(project("plain words", refs)).toBe("plain words");
  });
});

describe("the neighbors", () => {
  it("walks the render order, subgroups and all, and the ends take no arrow", () => {
    const one = flight(1, "ready");
    const two = flight(2, "in_progress");
    const three = flight(3, "done");
    const nested: Folded = {
      groups: [
        group("tyler", [], [group("high", [one]), group("none", [two])]),
        group(null, [three]),
      ],
      hidden: 0,
      filtered: 0,
    };
    expect(neighbors(nested, two.id)).toEqual({ prev: one.id, next: three.id });
    expect(neighbors(nested, one.id)).toEqual({ prev: null, next: two.id });
    expect(neighbors(nested, three.id)).toEqual({ prev: two.id, next: null });
    // A flight the fold does not carry stands alone.
    expect(neighbors(nested, "pi-8c2e.9")).toEqual({ prev: null, next: null });
  });
});
