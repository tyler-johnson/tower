// The views, pure: the two built-ins, and the three questions the chip
// row asks of a URL — which view it is, whether it is any view at all,
// and what a save would store. No runes and no `$app` imports, so it
// tests under vitest with no shims.

import { parse, render } from "./query";

/// One saved view as the wire carries it.
export interface View {
  id: string;
  name: string;
  /// `Query::render`'s canonical text; `''` is the default board.
  query: string;
  shared: boolean;
  author: string;
  saved_by: string;
  saved_at: number;
}

/// The two views that ship with the app. Not stored, not editable:
/// All Flights is the default query, and For Me is the rows only a
/// person can handle, an open question in any lane or the `me` lane —
/// the inbox the CLI still pins, as a view.
export const BUILTINS: readonly { name: string; query: string }[] = [
  { name: "All Flights", query: "" },
  { name: "For Me", query: "for=me" },
];

/// The URL's query in the form a view stores, or null while the server
/// refuses it. A hand-typed `?closed=1d` is the default board.
export function canonical(search: string): string | null {
  const parsed = parse(search);
  return parsed === null ? null : render(parsed);
}

/// Whether a view is the one on the URL.
export function isActive(view: { query: string }, search: string): boolean {
  return canonical(search) === view.query;
}

/// Whether the URL holds a query no view names, which is when Save has
/// something to save. A refused query is never saveable.
export function unsaved(search: string, views: View[]): boolean {
  const query = canonical(search);
  if (query === null) return false;
  return ![...BUILTINS, ...views].some((view) => view.query === query);
}
