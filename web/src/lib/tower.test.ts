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
    filed_at: 0,
    comments: 0,
    depends_on: [],
    blocks: [],
    status,
    status_by: null,
    status_session: null,
    status_at: null,
    assignee: null,
    priority: "none",
    labels,
    skill: null,
    bay: null,
    branch: null,
    tip: null,
    last_change: null,
    stale: false,
    changed_since_ready: false,
    progress: null,
    held: false,
    resolving: false,
    current: false,
    question: null,
    asked_at: null,
    closed_reason: null,
    collides: [],
    unanswered: [],
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

function brief(number: number, depends_on: LinkView[], blocks: LinkView[] = []): Brief {
  return {
    ...flight(number, "ready"),
    status_reason: null,
    edited_by: null,
    edited_at: null,
    asked_by: null,
    depends_on,
    blocks,
    comments: [],
    history: [],
    standing: "ready",
    beat: [],
  };
}

describe("the byline", () => {
  it("shortens a UUID session to its first eight characters in brackets", () => {
    expect(byline("95b36d9d-efdc-4564-9b06-91842f51ef6b", "a@b.c")).toBe("[95b36d9d]");
  });
  it("renders any other session verbatim", () => {
    expect(byline("tyler", "a@b.c")).toBe("tyler");
    expect(byline("hand-typed", "a@b.c")).toBe("hand-typed");
    expect(byline("95b36d9d-efdc-4564-9b06-91842f51ef6", "a@b.c")).toBe(
      "95b36d9d-efdc-4564-9b06-91842f51ef6",
    );
  });
  it("falls back to the author with no session", () => {
    expect(byline(null, "a@b.c")).toBe("a@b.c");
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
