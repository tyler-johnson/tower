// A drag is a verb or it is not offered: what each grouping seeds, which
// cards pick up, and what one drop writes.

import { describe, expect, it } from "vitest";
import { draggable, drop, seeded } from "./drag";
import type { FlightView } from "./tower";

function flight(
  number: number,
  status: string,
  labels: string[] = [],
  question: string | null = null,
): FlightView {
  return {
    id: `pi-8c2e.${number}`,
    number,
    procedure: null,
    subject: `flight ${number}`,
    body: "",
    filed_by: "tyler",
    filed_at: 0,
    comments: 0,
    depends_on: [],
    blocks: [],
    status,
    status_by: null,
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
    held: question !== null,
    resolving: false,
    current: false,
    question,
    asked_at: question !== null ? 0 : null,
    collides: [],
    unanswered: [],
  };
}

describe("the drag", () => {
  it("a grouping seeds its vocabulary", () => {
    expect(seeded("status")).toEqual([
      "triage",
      "waiting",
      "ready",
      "in_progress",
      "held",
      "done",
      "canceled",
    ]);
    expect(seeded("priority")).toEqual(["urgent", "high", "medium", "low", "none"]);
    expect(seeded("assignee")).toEqual(["me", "agent", null]);
    expect(seeded("label")).toEqual([]);
    expect(seeded(null)).toEqual([]);
  });

  it("a status drop is the verb the column names", () => {
    const ready = flight(1, "ready");
    expect(drop("status", ready, "ready", "in_progress")).toEqual({
      verb: "status",
      body: { flight: ready.id, status: "in_progress" },
    });
    expect(drop("status", ready, "ready", "done")).toEqual({
      verb: "done",
      body: { flight: ready.id },
    });
    expect(drop("status", ready, "ready", "canceled")).toEqual({
      verb: "cancel",
      body: { flight: ready.id },
    });
    expect(drop("status", ready, "ready", "waiting")).toBeNull();
    expect(drop("status", ready, "ready", "held")).toBeNull();
    expect(drop("status", ready, "ready", "ready")).toBeNull();
  });

  it("a closed flight moves under edit and not under status", () => {
    const done = flight(2, "done");
    expect(draggable("status", done)).toBe(false);
    expect(draggable("assignee", done)).toBe(false);
    expect(draggable("priority", done)).toBe(true);
    expect(drop("status", done, "done", "ready")).toBeNull();
    expect(drop("priority", done, "none", "high")).toEqual({
      verb: "edit",
      body: { target: done.id, priority: "high" },
    });
  });

  it("a questioned flight drops only on the closed columns", () => {
    const held = flight(3, "held", [], "which?");
    expect(drop("status", held, "held", "ready")).toBeNull();
    expect(drop("status", held, "held", "done")).toEqual({
      verb: "done",
      body: { flight: held.id },
    });
  });

  it("the none lane is the wire word", () => {
    const ready = flight(4, "ready");
    expect(drop("assignee", ready, "me", null)).toEqual({
      verb: "assign",
      body: { flight: ready.id, assignee: "none" },
    });
    expect(drop("assignee", ready, null, "agent")).toEqual({
      verb: "assign",
      body: { flight: ready.id, assignee: "agent" },
    });
  });

  it("a label drop rewrites the set and never empties it", () => {
    const two = flight(5, "ready", ["web", "ui"]);
    expect(drop("label", two, "web", "ui")).toEqual({
      verb: "edit",
      body: { target: two.id, labels: ["ui"] },
    });
    expect(drop("label", two, "web", "infra")).toEqual({
      verb: "edit",
      body: { target: two.id, labels: ["ui", "infra"] },
    });
    expect(drop("label", two, "web", null)).toEqual({
      verb: "edit",
      body: { target: two.id, labels: ["ui"] },
    });
    const one = flight(6, "ready", ["web"]);
    expect(drop("label", one, "web", null)).toBeNull();
    expect(drop("label", one, "web", "ui")).toEqual({
      verb: "edit",
      body: { target: one.id, labels: ["ui"] },
    });
  });

  it("a skill or bay has no clear path", () => {
    const ready = flight(7, "ready");
    expect(drop("skill", ready, "rust", null)).toBeNull();
    expect(drop("skill", ready, null, "rust")).toEqual({
      verb: "edit",
      body: { target: ready.id, skill: "rust" },
    });
    expect(drop("bay", ready, "a", null)).toBeNull();
    expect(drop("procedure", ready, "a", "b")).toBeNull();
  });
});
