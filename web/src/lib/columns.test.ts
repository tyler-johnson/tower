// The cells' cases: the default columns draw what the fixed row drew,
// and every field answers with its fact.

import { describe, expect, it } from "vitest";
import { cell, noteStart, template } from "./columns";
import { DEFAULT_SHOW } from "./query";
import { age, type FlightView } from "./tower";

const now = 1_700_000_000;

const view: FlightView = {
  id: "pi-8c2e.3",
  number: 3,
  procedure: null,
  subject: "the list view",
  body: "",
  filed_by: "tyler",
  filed_at: now - 3 * 86_400,
  comments: 0,
  depends_on: [],
  blocks: [],
  status: "in_progress",
  status_by: null,
  status_at: null,
  assignee: null,
  priority: "high",
  labels: ["web", "ui"],
  skill: null,
  bay: null,
  branch: "@detached",
  tip: null,
  last_change: null,
  stale: false,
  changed_since_ready: false,
  progress: [1, 3],
  held: true,
  resolving: false,
  current: false,
  question: null,
  asked_at: null,
  collides: [],
  unanswered: [],
};

const refs = new Map([[view.id, "#3"]]);

describe("the columns", () => {
  it("the default columns draw the row’s anatomy", () => {
    expect(template(DEFAULT_SHOW)).toBe(
      "1ch max-content 1ch minmax(0,1fr) max-content max-content max-content",
    );
    expect(noteStart(DEFAULT_SHOW)).toBe(4);
    expect(template(["ref", "age"])).toMatch(/ 1fr$/);
    expect(noteStart(["ref", "age"])).toBe(1);
  });

  it("a cell is the field’s fact", () => {
    const at = (field: Parameters<typeof cell>[0]) => cell(field, view, refs, now);
    expect(at("priority")).toEqual({ kind: "glyph", text: "↑", title: "priority high" });
    expect(at("ref")).toEqual({ kind: "ref", text: "#3" });
    expect(at("subject")).toEqual({ kind: "subject", text: "the list view (1/3)" });
    expect(at("label")).toEqual({ kind: "chips", words: ["web", "ui"] });
    expect(at("assignee")).toEqual({ kind: "chips", words: [] });
    expect(at("moved")).toEqual({ kind: "dim", text: "" });
    expect(at("filed")).toEqual({ kind: "dim", text: age(now, view.filed_at) });
    expect(at("held")).toEqual({ kind: "flag", text: "held", on: true });
    expect(at("stale")).toEqual({ kind: "flag", text: "stale", on: false });
    expect(at("comments")).toEqual({ kind: "dim", text: "" });
    expect(at("progress")).toEqual({ kind: "dim", text: "1/3" });
    expect(at("branch")).toEqual({ kind: "dim", text: "(detached)" });
  });
});
