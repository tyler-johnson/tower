//! The query: filters, grouping, ordering, and the display window as one
//! type, parsed once here and folded server-side.
//!
//! DESIGN's *filters and saved views* answered on the first of the two
//! readings it left open — the predicates live in core and every surface
//! shares them, rather than being written twice against the same
//! envelope. A saved view stores one thing, the web holds one piece of
//! state, and there is exactly one place a query can be spelled wrong.
//!
//! **A query is a string on every wire and a struct only in memory.**
//! [`Query::parse`] and [`Query::render`] are its whole wire contract:
//! the rendered param string is what a route takes, what a browser URL
//! holds, and what a saved-view event stores. `Query` therefore carries
//! no `Serialize` and no `Deserialize` — only the fold's *output* is
//! serialized, which is what one codec rather than three buys.
//!
//! The codec is hand-rolled, like every other grammar in this crate
//! ([`parse_cadence`](crate::config::parse_cadence),
//! [`parse_closed`](super::parse_closed), [`parse_ref`](super::parse_ref)),
//! and it includes its own percent decode and encode: labels are freeform
//! strings, `,` separates values, and a label carrying a comma, a space,
//! or a colon has to survive the round trip.
//!
//! ```text
//! status=ready,in_progress&priority=high&label=infra
//!  &filed=after:3d&group=assignee&sub=priority
//!  &order=priority&closed=7d&show=ref,status,assignee,label,age
//! ```
//!
//! **Field names, operators, and axes are closed and refuse; values are
//! open.** `status=bogus` parses and matches nothing. The doctrine at
//! [`crate::model`] — the fold and the wire carry whatever got written,
//! so a newer tower's value costs one flight's fidelity rather than the
//! whole board — binds harder on a *saved* query than on a single render:
//! a view saved by a newer tower must not become unparseable here.

use std::cmp::Ordering;

use serde::Serialize;

use super::model::{self, ClosedWindow, DEFAULT_CLOSED, FlightView, closed_at, rank};

/// One axis of the record a query can name.
///
/// Every name is a field a person can type, and the four capability
/// tables below say what each one is allowed to be: a predicate, a
/// grouping, an ordering, a column. `Ref`, `Age`, `Comments` and
/// `Progress` are columns and never predicates — they are derivations a
/// render prints, not stored facts to compare against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Status,
    Assignee,
    Priority,
    Label,
    Skill,
    Bay,
    Subject,
    Body,
    Procedure,
    Branch,
    Filed,
    Moved,
    Changed,
    Stale,
    ChangedSinceReady,
    Held,
    Ref,
    Age,
    Comments,
    Progress,
}

/// Every field, in the order the refusals list them.
pub const FIELDS: [Field; 20] = [
    Field::Status,
    Field::Assignee,
    Field::Priority,
    Field::Label,
    Field::Skill,
    Field::Bay,
    Field::Subject,
    Field::Body,
    Field::Procedure,
    Field::Branch,
    Field::Filed,
    Field::Moved,
    Field::Changed,
    Field::Stale,
    Field::ChangedSinceReady,
    Field::Held,
    Field::Ref,
    Field::Age,
    Field::Comments,
    Field::Progress,
];

/// What a field compares as, which is what decides the operators it
/// takes and the shape of the value beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A word or a set of words, compared exactly.
    Words,
    /// Free prose, compared by substring.
    Text,
    /// An epoch, compared against a moment.
    Time,
    /// A column: no predicate at all.
    Column,
}

impl Field {
    /// The wire name — the param key, and the word every refusal prints.
    pub fn name(&self) -> &'static str {
        match self {
            Field::Status => "status",
            Field::Assignee => "assignee",
            Field::Priority => "priority",
            Field::Label => "label",
            Field::Skill => "skill",
            Field::Bay => "bay",
            Field::Subject => "subject",
            Field::Body => "body",
            Field::Procedure => "procedure",
            Field::Branch => "branch",
            Field::Filed => "filed",
            Field::Moved => "moved",
            Field::Changed => "changed",
            Field::Stale => "stale",
            Field::ChangedSinceReady => "changed_since_ready",
            Field::Held => "held",
            Field::Ref => "ref",
            Field::Age => "age",
            Field::Comments => "comments",
            Field::Progress => "progress",
        }
    }

    /// The wire name back to the field; `None` for a word that names no
    /// axis. Unlike a value, a field name is closed and refuses.
    pub fn from_name(name: &str) -> Option<Field> {
        FIELDS.into_iter().find(|field| field.name() == name)
    }

    /// Whether a filter can be written against it.
    pub fn filterable(&self) -> bool {
        self.shape() != Shape::Column
    }

    /// The six a board can be grouped into columns by.
    pub fn groupable(&self) -> bool {
        matches!(
            self,
            Field::Status
                | Field::Assignee
                | Field::Priority
                | Field::Label
                | Field::Skill
                | Field::Bay
        )
    }

    /// The axes rows sort along. Every one is a stored fact with a total
    /// order; a set-valued field like `label` has none, so it is absent.
    pub fn orderable(&self) -> bool {
        matches!(
            self,
            Field::Status
                | Field::Assignee
                | Field::Priority
                | Field::Subject
                | Field::Filed
                | Field::Moved
                | Field::Changed
        )
    }

    /// Whether a row can carry it as a column. Everything but the body,
    /// which is a flight's prose — the single view's, never a list's.
    pub fn showable(&self) -> bool {
        *self != Field::Body
    }

    fn shape(&self) -> Shape {
        match self {
            Field::Status
            | Field::Assignee
            | Field::Priority
            | Field::Label
            | Field::Skill
            | Field::Bay
            | Field::Procedure
            | Field::Branch
            | Field::Stale
            | Field::ChangedSinceReady
            | Field::Held => Shape::Words,
            Field::Subject | Field::Body => Shape::Text,
            Field::Filed | Field::Moved | Field::Changed => Shape::Time,
            Field::Ref | Field::Age | Field::Comments | Field::Progress => Shape::Column,
        }
    }

    fn accepts(&self, op: Op) -> bool {
        match self.shape() {
            Shape::Words => matches!(op, Op::Is | Op::IsNot),
            Shape::Text => op == Op::Contains,
            Shape::Time => matches!(op, Op::Before | Op::After),
            Shape::Column => false,
        }
    }

    /// The operator an unprefixed value means. A time has none — a
    /// moment on its own says nothing about which side of it to keep.
    fn default_op(&self) -> Option<Op> {
        match self.shape() {
            Shape::Words => Some(Op::Is),
            Shape::Text => Some(Op::Contains),
            Shape::Time | Shape::Column => None,
        }
    }
}

/// The five operators.
///
/// `Is` and `IsNot` take a *set*, which collapses the usual seven into
/// these five with nothing lost: one value is the common case, several
/// is "any of", and a chip renders "is" or "is any of" off the set's
/// length. For `label`, `Is` over a set means *carries any of these* —
/// the only reading a list view offers either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Is,
    IsNot,
    Contains,
    Before,
    After,
}

impl Op {
    /// The wire spelling — the prefix before the `:` in a value.
    pub fn name(&self) -> &'static str {
        match self {
            Op::Is => "is",
            Op::IsNot => "not",
            Op::Contains => "contains",
            Op::Before => "before",
            Op::After => "after",
        }
    }

    fn from_name(name: &str) -> Option<Op> {
        [Op::Is, Op::IsNot, Op::Contains, Op::Before, Op::After]
            .into_iter()
            .find(|op| op.name() == name)
    }
}

/// What a filter compares against, in the shape its operator takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Value {
    /// `Is` and `IsNot`: exact words, any of which counts as a hit.
    Words(Vec<String>),
    /// `Contains`: a substring, matched case-insensitively.
    Text(String),
    /// `Before` and `After`: a moment.
    When(When),
}

/// A moment, either relative to the caller's clock or absolute.
///
/// The distinction is the whole reason a query is a string on the wire:
/// a saved view filed `after:3d` means *the last three days* every day
/// it is opened, and resolving it to an epoch at save time would freeze
/// it to the day it was saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum When {
    /// Seconds before `now` — spelled `3d`, `12h`, `2w`.
    Ago(i64),
    /// An epoch, spelled `@1700000000`.
    At(i64),
}

/// One predicate. Filters AND together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Filter {
    pub field: Field,
    pub op: Op,
    pub value: Value,
}

impl Filter {
    /// The checked constructor: a field/operator pair with no meaning —
    /// `before` on `subject`, `contains` on `status` — refuses and names
    /// both, and so does a value in the wrong shape for its operator.
    pub fn new(field: Field, op: Op, value: Value) -> Result<Filter, QueryError> {
        if !field.filterable() {
            return Err(QueryError::NotFilterable {
                field: field.name(),
            });
        }
        if !field.accepts(op) {
            return Err(QueryError::BadOperator {
                field: field.name(),
                op: op.name(),
            });
        }
        let shaped = match op {
            Op::Is | Op::IsNot => matches!(value, Value::Words(_)),
            Op::Contains => matches!(value, Value::Text(_)),
            Op::Before | Op::After => matches!(value, Value::When(_)),
        };
        if !shaped {
            return Err(QueryError::BadValue {
                field: field.name(),
                op: op.name(),
            });
        }
        Ok(Filter { field, op, value })
    }
}

/// How rows sort inside a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Order {
    pub field: Field,
    pub descending: bool,
}

impl Order {
    /// The checked constructor: an axis with no total order refuses.
    pub fn new(field: Field, descending: bool) -> Result<Order, QueryError> {
        if !field.orderable() {
            return Err(QueryError::NotOrderable {
                field: field.name(),
            });
        }
        Ok(Order { field, descending })
    }
}

impl Default for Order {
    /// Today's board: priority, urgent first, oldest first within it.
    fn default() -> Self {
        Order {
            field: Field::Priority,
            descending: false,
        }
    }
}

/// Which render the query is aimed at. The fold ignores it — a mode is
/// a display property, carried here because a saved view stores one
/// thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    List,
    Board,
}

impl Mode {
    /// The wire spelling.
    pub fn name(&self) -> &'static str {
        match self {
            Mode::List => "list",
            Mode::Board => "board",
        }
    }
}

/// The columns today's row renders, which is what a query that names
/// none asks for.
pub const DEFAULT_SHOW: [Field; 7] = [
    Field::Priority,
    Field::Ref,
    Field::Status,
    Field::Subject,
    Field::Label,
    Field::Assignee,
    Field::Age,
];

/// One query: the axes the fold reads, and the display properties it
/// ignores, in one type.
///
/// [`Query::default`] is today's board — grouped by status, ordered
/// priority then age, the compiled-in closed window, empty groups
/// dropped, list mode, and the columns a row renders now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// ANDed together: every one must hold.
    pub filters: Vec<Filter>,
    /// The columns rows are dealt into; `None` is one flat list.
    pub group: Option<Field>,
    /// A second grouping inside each column. Ignored when `group` is
    /// `None` — there is nothing to nest inside.
    pub subgroup: Option<Field>,
    pub order: Order,
    /// How much of the closed record the fold carries.
    pub closed: ClosedWindow,
    /// Whether a group the vocabulary names but no row landed in still
    /// appears.
    pub empty_groups: bool,
    /// A display property; the fold ignores it.
    pub mode: Mode,
    /// A display property; the fold ignores it.
    pub show: Vec<Field>,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            filters: Vec::new(),
            group: Some(Field::Status),
            subgroup: None,
            order: Order::default(),
            closed: DEFAULT_CLOSED,
            empty_groups: false,
            mode: Mode::List,
            show: DEFAULT_SHOW.to_vec(),
        }
    }
}

/// One group of rows, and the groups nested inside it.
///
/// A group holds rows or subgroups, never both. `count` is the rows the
/// group itself received, before any nesting, so a subgrouped column
/// still says how tall it is.
///
/// Serialization follows the board's doctrine and not the log's: every
/// key is emitted, `null` for absent, and nothing is skipped.
#[derive(Debug, Serialize)]
pub struct Group {
    /// The value this column is keyed on; `null` is the column for rows
    /// that carry none — no assignee, no label — and the single group of
    /// an ungrouped fold.
    pub key: Option<String>,
    pub count: usize,
    pub rows: Vec<FlightView>,
    pub subgroups: Vec<Group>,
}

/// A query's answer: the groups, and the two counts.
///
/// The counts are disjoint by construction, which is what lets a footer
/// print both with two different remedies beside them. The closed window
/// runs first and `hidden` counts what it hid; the filters run over what
/// survived and `filtered` counts what they rejected. No flight is
/// counted twice.
#[derive(Debug, Serialize)]
pub struct Folded {
    pub groups: Vec<Group>,
    /// Closed flights the window kept off the board.
    pub hidden: usize,
    /// Flights the filters rejected.
    pub filtered: usize,
}

impl Query {
    /// Parse a rendered query. An empty string is [`Query::default`],
    /// and a leading `?` is tolerated so a browser's own `location.search`
    /// can be handed over whole.
    ///
    /// Any key that is not one of the seven axes — `group`, `sub`,
    /// `order`, `closed`, `empty`, `mode`, `show` — is a field name, and
    /// a name that no field answers to refuses.
    pub fn parse(raw: &str) -> Result<Query, QueryError> {
        let mut query = Query::default();
        query.filters.clear();
        let raw = raw.trim();
        let raw = raw.strip_prefix('?').unwrap_or(raw);
        for part in raw.split('&') {
            if part.is_empty() {
                continue;
            }
            let Some((key, value)) = part.split_once('=') else {
                return Err(QueryError::BadParam { text: decode(part) });
            };
            let key = decode(key);
            match key.as_str() {
                "group" => query.group = grouping(value)?,
                "sub" => query.subgroup = grouping(value)?,
                "order" => query.order = ordering(value)?,
                "closed" => {
                    let text = decode(value);
                    query.closed =
                        super::parse_closed(&text).ok_or(QueryError::BadWindow { text })?;
                }
                "empty" => query.empty_groups = flag(&decode(value))?,
                "mode" => query.mode = mode(&decode(value))?,
                "show" => query.show = columns(value)?,
                _ => query.filters.push(filter(&key, value)?),
            }
        }
        Ok(query)
    }

    /// Render the query back to its param string.
    ///
    /// Only what differs from [`Query::default`] is spelled, so the
    /// default renders to the empty string and a filtered board's link
    /// stays hand-editable and greppable. Values are percent-encoded
    /// against the structural characters — `&`, `=`, `,`, `:` — so a
    /// label carrying any of them round-trips.
    pub fn render(&self) -> String {
        let default = Query::default();
        let mut parts: Vec<String> = self.filters.iter().map(render_filter).collect();
        if self.group != default.group {
            parts.push(format!(
                "group={}",
                self.group.map(|field| field.name()).unwrap_or_default()
            ));
        }
        if self.subgroup != default.subgroup {
            parts.push(format!(
                "sub={}",
                self.subgroup.map(|field| field.name()).unwrap_or_default()
            ));
        }
        if self.order != default.order {
            let sign = if self.order.descending { "-" } else { "" };
            parts.push(format!("order={sign}{}", self.order.field.name()));
        }
        if self.closed != default.closed {
            parts.push(format!("closed={}", render_window(self.closed)));
        }
        if self.empty_groups != default.empty_groups {
            parts.push(format!("empty={}", self.empty_groups));
        }
        if self.mode != default.mode {
            parts.push(format!("mode={}", self.mode.name()));
        }
        if self.show != default.show {
            let names: Vec<&str> = self.show.iter().map(|field| field.name()).collect();
            parts.push(format!("show={}", names.join(",")));
        }
        parts.join("&")
    }

    /// Whether a hand-built query names only axes that mean something.
    /// [`Query::parse`] checks the same things as it goes; this is for
    /// the callers that assemble a query in memory.
    pub fn check(&self) -> Result<(), QueryError> {
        for filter in &self.filters {
            Filter::new(filter.field, filter.op, filter.value.clone())?;
        }
        for field in self.group.iter().chain(self.subgroup.iter()) {
            if !field.groupable() {
                return Err(QueryError::NotGroupable {
                    field: field.name(),
                });
            }
        }
        Order::new(self.order.field, self.order.descending)?;
        for field in &self.show {
            if !field.showable() {
                return Err(QueryError::NotShowable {
                    field: field.name(),
                });
            }
        }
        Ok(())
    }

    /// Fold rows into the query's answer.
    ///
    /// The order of operations is what keeps the two counts disjoint:
    /// the closed window first, counting what it hid; the filters over
    /// what survived, counting what they rejected; then group, then
    /// order, then drop empty groups unless the query asked for them.
    ///
    /// Grouping by `label` puts a flight in every column it carries, and
    /// a flight carrying none lands in the `null` column. Grouping by
    /// `status` deals rows into the board's own sections — `done` and
    /// `canceled` together under `closed` — and a status this binary has
    /// never heard of routes nowhere, exactly as
    /// [`enrich`](super::enrich) has it.
    ///
    /// `now` arrives from the caller, like everywhere else in this
    /// module: a relative filter and a span window both need a clock,
    /// and the fold reads none for itself.
    pub fn fold(&self, flights: Vec<FlightView>, now: i64) -> Folded {
        let (flights, hidden) = window(flights, self.closed, now);

        let before = flights.len();
        let flights: Vec<FlightView> = flights
            .into_iter()
            .filter(|view| self.filters.iter().all(|filter| holds(filter, view, now)))
            .collect();
        let filtered = before - flights.len();

        let mut groups = match self.group {
            None => vec![Group {
                key: None,
                count: flights.len(),
                rows: flights,
                subgroups: Vec::new(),
            }],
            Some(field) => grouped(field, flights, self.empty_groups),
        };

        for group in &mut groups {
            // The closed column keeps the window's own newest-first
            // order: the window selected by recency, and re-sorting what
            // it chose would answer a different question than the one it
            // was asked.
            let newest_first =
                self.group == Some(Field::Status) && group.key.as_deref() == Some("closed");
            sort(&mut group.rows, self.order, newest_first);
            if let (Some(_), Some(sub)) = (self.group, self.subgroup) {
                let rows = std::mem::take(&mut group.rows);
                group.subgroups = grouped(sub, rows, self.empty_groups);
                for subgroup in &mut group.subgroups {
                    sort(&mut subgroup.rows, self.order, newest_first);
                }
            }
        }

        Folded {
            groups,
            hidden,
            filtered,
        }
    }
}

/// What naming a query can be refused with. Each variant carries a
/// stable id and its own exits, on [`ResolveError`](super::ResolveError)'s
/// template, so a caller with no registry of its own still answers in the
/// same words the CLI does.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// Text with no `=` in it at all.
    #[error("`{text}` is not a query parameter — every one is `<name>=<value>`")]
    BadParam { text: String },

    #[error("`{text}` is not a field — name one of {}", names())]
    UnknownField { text: String },

    #[error("`{text}` is not an operator — `is`, `not`, `contains`, `before`, or `after`")]
    UnknownOperator { text: String },

    #[error("`{field}` is a column and never a predicate — nothing filters on it")]
    NotFilterable { field: &'static str },

    #[error("`{field}` takes no `{op}`")]
    BadOperator {
        field: &'static str,
        op: &'static str,
    },

    #[error("`{field}` takes no value of that shape beside `{op}`")]
    BadValue {
        field: &'static str,
        op: &'static str,
    },

    #[error(
        "`{field}` is not a column a board groups by — group by status, assignee, priority, \
         label, skill, or bay"
    )]
    NotGroupable { field: &'static str },

    #[error(
        "`{field}` is not an axis rows order along — order by status, assignee, priority, \
         subject, filed, moved, or changed"
    )]
    NotOrderable { field: &'static str },

    #[error("`{field}` is not a column a row can show")]
    NotShowable { field: &'static str },

    #[error("`{text}` is not a moment — a span like `3d` or `12h`, or `@<epoch>`")]
    BadTime { text: String },

    #[error("`{text}` is not a closed window — a count, a span like `7d`, `all`, or `none`")]
    BadWindow { text: String },

    #[error("`{text}` is not `true` or `false`")]
    BadFlag { text: String },

    #[error("`{text}` is not a mode — `list` or `board`")]
    BadMode { text: String },
}

impl QueryError {
    /// The stable id, tower's `category/kebab-case`. Every one is a
    /// usage refusal: a query is text a person typed, and nothing here
    /// depends on what is filed.
    pub fn id(&self) -> &'static str {
        match self {
            QueryError::BadParam { .. } => "usage/bad-query",
            QueryError::UnknownField { .. } => "usage/unknown-field",
            QueryError::UnknownOperator { .. } => "usage/unknown-operator",
            QueryError::NotFilterable { .. } => "usage/not-filterable",
            QueryError::BadOperator { .. } => "usage/bad-operator",
            QueryError::BadValue { .. } => "usage/bad-value",
            QueryError::NotGroupable { .. } => "usage/not-groupable",
            QueryError::NotOrderable { .. } => "usage/not-orderable",
            QueryError::NotShowable { .. } => "usage/not-showable",
            QueryError::BadTime { .. } => "usage/bad-time",
            QueryError::BadWindow { .. } => "usage/bad-window",
            QueryError::BadFlag { .. } => "usage/bad-flag",
            QueryError::BadMode { .. } => "usage/bad-mode",
        }
    }

    /// Commands that lead out of it. One answer for all of them: the
    /// board is what shows which values are actually filed, in the same
    /// vocabulary the grammar accepts.
    pub fn exits(&self) -> Vec<String> {
        vec!["ff tower".to_string()]
    }
}

/// Every field name, comma-separated — what an unknown one is measured
/// against.
fn names() -> String {
    FIELDS
        .iter()
        .map(|field| field.name())
        .collect::<Vec<_>>()
        .join(", ")
}

// ---- the codec ------------------------------------------------------

/// A grouping axis: an empty value is "ungrouped", anything else is a
/// field that has to be groupable.
fn grouping(value: &str) -> Result<Option<Field>, QueryError> {
    let text = decode(value);
    if text.is_empty() {
        return Ok(None);
    }
    let field = Field::from_name(&text).ok_or(QueryError::UnknownField { text })?;
    if !field.groupable() {
        return Err(QueryError::NotGroupable {
            field: field.name(),
        });
    }
    Ok(Some(field))
}

/// An ordering: a field name, optionally `-`-prefixed for descending.
fn ordering(value: &str) -> Result<Order, QueryError> {
    let text = decode(value);
    let (descending, name) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.to_string()),
        None => (false, text),
    };
    let field = Field::from_name(&name).ok_or(QueryError::UnknownField { text: name.clone() })?;
    Order::new(field, descending)
}

/// The column list. An empty value is no columns at all.
fn columns(value: &str) -> Result<Vec<Field>, QueryError> {
    let mut show = Vec::new();
    for piece in value.split(',') {
        if piece.is_empty() {
            continue;
        }
        let text = decode(piece);
        let field = Field::from_name(&text).ok_or(QueryError::UnknownField { text })?;
        if !field.showable() {
            return Err(QueryError::NotShowable {
                field: field.name(),
            });
        }
        show.push(field);
    }
    Ok(show)
}

fn flag(text: &str) -> Result<bool, QueryError> {
    match text {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(QueryError::BadFlag {
            text: text.to_string(),
        }),
    }
}

fn mode(text: &str) -> Result<Mode, QueryError> {
    match text {
        "list" => Ok(Mode::List),
        "board" => Ok(Mode::Board),
        _ => Err(QueryError::BadMode {
            text: text.to_string(),
        }),
    }
}

/// One filter, from its param key and its still-encoded value.
///
/// The operator prefix is split before anything is decoded, so a value
/// carrying a literal `:` — which the encoder writes as `%3A` — is never
/// mistaken for one. A prefix that names no operator refuses rather than
/// being read as part of the value: a typo has to be told about.
fn filter(key: &str, value: &str) -> Result<Filter, QueryError> {
    let field = Field::from_name(key).ok_or(QueryError::UnknownField {
        text: key.to_string(),
    })?;
    if !field.filterable() {
        return Err(QueryError::NotFilterable {
            field: field.name(),
        });
    }
    let (op, rest) = match value.split_once(':') {
        Some((prefix, rest)) => {
            let op = Op::from_name(prefix).ok_or(QueryError::UnknownOperator {
                text: prefix.to_string(),
            })?;
            (op, rest)
        }
        None => {
            let op = field.default_op().ok_or(QueryError::BadOperator {
                field: field.name(),
                op: Op::Is.name(),
            })?;
            (op, value)
        }
    };
    let value = match op {
        Op::Is | Op::IsNot => Value::Words(
            rest.split(',')
                .filter(|piece| !piece.is_empty())
                .map(decode)
                .collect(),
        ),
        Op::Contains => Value::Text(decode(rest)),
        Op::Before | Op::After => Value::When(moment(&decode(rest))?),
    };
    Filter::new(field, op, value)
}

/// A moment: `@<epoch>` absolute, anything else the duration grammar
/// this project already has, read as seconds before now.
fn moment(text: &str) -> Result<When, QueryError> {
    if let Some(digits) = text.strip_prefix('@') {
        return digits
            .parse()
            .map(When::At)
            .map_err(|_| QueryError::BadTime {
                text: text.to_string(),
            });
    }
    crate::config::parse_duration(text)
        .map(When::Ago)
        .ok_or(QueryError::BadTime {
            text: text.to_string(),
        })
}

fn render_filter(filter: &Filter) -> String {
    let name = filter.field.name();
    match &filter.value {
        // `is` is the unprefixed form, which is what makes the common
        // filter read as `status=ready`.
        Value::Words(words) => {
            let body = words
                .iter()
                .map(|word| encode(word))
                .collect::<Vec<_>>()
                .join(",");
            match filter.op {
                Op::Is => format!("{name}={body}"),
                _ => format!("{name}={}:{body}", filter.op.name()),
            }
        }
        Value::Text(text) => format!("{name}={}:{}", filter.op.name(), encode(text)),
        Value::When(when) => format!("{name}={}:{}", filter.op.name(), render_moment(*when)),
    }
}

fn render_moment(when: When) -> String {
    match when {
        When::At(at) => format!("@{at}"),
        When::Ago(secs) => render_span(secs),
    }
}

/// Seconds back to the largest unit that divides them exactly, so a
/// relative filter stays words rather than becoming a count of seconds.
/// The spelling normalizes — `7d` comes back `1w` — and the value it
/// stands for does not, which is what a round trip is about.
fn render_span(secs: i64) -> String {
    const UNITS: [(char, i64); 4] = [
        ('w', 7 * 24 * 60 * 60),
        ('d', 24 * 60 * 60),
        ('h', 60 * 60),
        ('m', 60),
    ];
    for (unit, size) in UNITS {
        if secs != 0 && secs % size == 0 {
            return format!("{}{unit}", secs / size);
        }
    }
    format!("{secs}s")
}

fn render_window(closed: ClosedWindow) -> String {
    match closed {
        ClosedWindow::All => "all".to_string(),
        ClosedWindow::None => "none".to_string(),
        ClosedWindow::Count(n) => n.to_string(),
        ClosedWindow::Span(secs) => render_span(secs),
    }
}

/// The unreserved set, plus `/` because branch names are full of it and
/// a query string is where it is legal unescaped. Everything else —
/// `&`, `=`, `,`, `:`, `%`, `#`, space, and every non-ASCII byte — is
/// percent-encoded, which is what makes a label carrying one survive.
fn encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The other half. `+` decodes to a space too — forms write it that way
/// and a hand-typed query might — and a `%` that begins no escape is
/// carried through as itself rather than refused: a value is open.
fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'+' => {
                out.push(b' ');
                at += 1;
            }
            b'%' if at + 2 < bytes.len() => match (nibble(bytes[at + 1]), nibble(bytes[at + 2])) {
                (Some(high), Some(low)) => {
                    out.push(high << 4 | low);
                    at += 3;
                }
                _ => {
                    out.push(b'%');
                    at += 1;
                }
            },
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// ---- the fold -------------------------------------------------------

/// The closed window, over flat rows: closed flights sorted newest-first
/// and cut, live flights untouched. Returns the survivors and how many
/// the window hid.
fn window(flights: Vec<FlightView>, closed: ClosedWindow, now: i64) -> (Vec<FlightView>, usize) {
    let (mut done, mut rows): (Vec<FlightView>, Vec<FlightView>) =
        flights.into_iter().partition(model::closed_row);
    done.sort_by_key(|view| std::cmp::Reverse(closed_at(view)));
    let before = done.len();
    match closed {
        ClosedWindow::All => {}
        ClosedWindow::None => done.clear(),
        ClosedWindow::Count(n) => done.truncate(n),
        ClosedWindow::Span(secs) => done.retain(|view| now - closed_at(view) <= secs),
    }
    let hidden = before - done.len();
    rows.extend(done);
    (rows, hidden)
}

/// Whether one filter holds of one row. A field the row does not carry
/// — no assignee, no branch, never moved — fails `is` and passes `not`,
/// which is what "the value is not one of these" has to mean.
fn holds(filter: &Filter, view: &FlightView, now: i64) -> bool {
    match filter.field {
        Field::Status => one(filter, Some(view.status.as_str())),
        Field::Priority => one(filter, Some(view.priority.as_str())),
        Field::Assignee => one(filter, view.assignee.as_deref()),
        Field::Skill => one(filter, view.skill.as_deref()),
        Field::Bay => one(filter, view.bay.as_deref()),
        Field::Procedure => one(filter, view.procedure.as_deref()),
        Field::Branch => one(filter, view.branch.as_deref()),
        Field::Label => many(filter, &view.labels),
        Field::Stale => one(filter, Some(spelled(view.stale))),
        Field::ChangedSinceReady => one(filter, Some(spelled(view.changed_since_ready))),
        Field::Held => one(filter, Some(spelled(view.held))),
        Field::Subject => contains(filter, &view.subject),
        Field::Body => contains(filter, &view.body),
        Field::Filed => moment_holds(filter, Some(view.filed_at), now),
        Field::Moved => moment_holds(filter, view.status_at, now),
        Field::Changed => moment_holds(filter, view.last_change, now),
        // A column carries no predicate; the constructor refuses one,
        // and a hand-built filter naming one matches nothing.
        Field::Ref | Field::Age | Field::Comments | Field::Progress => false,
    }
}

fn spelled(flag: bool) -> &'static str {
    if flag { "true" } else { "false" }
}

fn one(filter: &Filter, held: Option<&str>) -> bool {
    let Value::Words(words) = &filter.value else {
        return false;
    };
    let hit = held.is_some_and(|held| words.iter().any(|word| word == held));
    match filter.op {
        Op::Is => hit,
        Op::IsNot => !hit,
        _ => false,
    }
}

fn many(filter: &Filter, held: &[String]) -> bool {
    let Value::Words(words) = &filter.value else {
        return false;
    };
    let hit = held.iter().any(|one| words.contains(one));
    match filter.op {
        Op::Is => hit,
        Op::IsNot => !hit,
        _ => false,
    }
}

fn contains(filter: &Filter, held: &str) -> bool {
    let Value::Text(text) = &filter.value else {
        return false;
    };
    filter.op == Op::Contains && held.to_lowercase().contains(&text.to_lowercase())
}

fn moment_holds(filter: &Filter, held: Option<i64>, now: i64) -> bool {
    let Value::When(when) = &filter.value else {
        return false;
    };
    let Some(held) = held else {
        return false;
    };
    let at = match when {
        When::Ago(secs) => now - secs,
        When::At(at) => *at,
    };
    match filter.op {
        Op::After => held > at,
        Op::Before => held < at,
        _ => false,
    }
}

/// The closed vocabularies a grouping enumerates up front, so an empty
/// column can still be asked for. A field with none takes its columns
/// from the rows themselves, where an empty one cannot arise.
fn vocabulary(field: Field) -> &'static [&'static str] {
    match field {
        Field::Status => &[
            "triage",
            "waiting",
            "ready",
            "in_progress",
            "held",
            "closed",
        ],
        Field::Priority => &["urgent", "high", "medium", "low", "none"],
        _ => &[],
    }
}

/// The board's own sections, which is what grouping by status means:
/// `done` and `canceled` are one closed column, and a status this
/// binary has never heard of names no column at all.
fn section(status: &str) -> Option<&'static str> {
    match status {
        "triage" => Some("triage"),
        "waiting" => Some("waiting"),
        "ready" => Some("ready"),
        "in_progress" => Some("in_progress"),
        "held" => Some("held"),
        "done" | "canceled" => Some("closed"),
        _ => None,
    }
}

/// Which columns a row belongs in. Usually one; `label` is every label
/// it carries, and none at all drops the row — which only `status` does,
/// for a word that names no section.
fn keys(field: Field, view: &FlightView) -> Vec<Option<String>> {
    match field {
        Field::Status => section(&view.status)
            .map(|name| Some(name.to_string()))
            .into_iter()
            .collect(),
        Field::Priority => vec![Some(view.priority.clone())],
        Field::Assignee => vec![view.assignee.clone()],
        Field::Skill => vec![view.skill.clone()],
        Field::Bay => vec![view.bay.clone()],
        Field::Label => {
            if view.labels.is_empty() {
                vec![None]
            } else {
                view.labels.iter().cloned().map(Some).collect()
            }
        }
        _ => vec![None],
    }
}

/// Deal rows into columns. The vocabulary's columns come first and in
/// its order; anything the rows themselves named follows alphabetically,
/// with the keyless column last.
fn grouped(field: Field, rows: Vec<FlightView>, keep_empty: bool) -> Vec<Group> {
    let seeded = vocabulary(field);
    let mut buckets: Vec<(Option<String>, Vec<FlightView>)> = seeded
        .iter()
        .map(|key| (Some((*key).to_string()), Vec::new()))
        .collect();
    for view in rows {
        for key in keys(field, &view) {
            match buckets.iter_mut().find(|(held, _)| *held == key) {
                Some((_, bucket)) => bucket.push(view.clone()),
                None => buckets.push((key, vec![view.clone()])),
            }
        }
    }
    buckets[seeded.len()..].sort_by(|(a, _), (b, _)| match (a, b) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    buckets
        .into_iter()
        .filter(|(_, rows)| keep_empty || !rows.is_empty())
        .map(|(key, rows)| Group {
            key,
            count: rows.len(),
            rows,
            subgroups: Vec::new(),
        })
        .collect()
}

/// Sort one column's rows. The sort is stable and rows arrive in filed
/// order, so equal rows keep it.
fn sort(rows: &mut [FlightView], order: Order, newest_first: bool) {
    if newest_first {
        rows.sort_by_key(|view| std::cmp::Reverse(closed_at(view)));
        return;
    }
    rows.sort_by(|a, b| {
        let ordering = compare(order.field, a, b);
        if order.descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
}

fn compare(field: Field, a: &FlightView, b: &FlightView) -> Ordering {
    match field {
        // The board's own tie-break, kept: priority, then oldest first.
        Field::Priority => rank(&a.priority)
            .cmp(&rank(&b.priority))
            .then(a.filed_at.cmp(&b.filed_at)),
        Field::Filed => a.filed_at.cmp(&b.filed_at),
        Field::Moved => absent_last(a.status_at, b.status_at),
        Field::Changed => absent_last(a.last_change, b.last_change),
        Field::Subject => a.subject.cmp(&b.subject),
        Field::Assignee => absent_last(a.assignee.as_deref(), b.assignee.as_deref()),
        Field::Status => lifecycle(&a.status)
            .cmp(&lifecycle(&b.status))
            .then(rank(&a.priority).cmp(&rank(&b.priority)))
            .then(a.filed_at.cmp(&b.filed_at)),
        _ => Ordering::Equal,
    }
}

fn absent_last<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// The status vocabulary in lifecycle order. A word this binary has
/// never heard of sorts after every one it knows, the same way an
/// unknown priority sorts after `none`.
fn lifecycle(status: &str) -> u8 {
    match status {
        "triage" => 0,
        "waiting" => 1,
        "ready" => 2,
        "in_progress" => 3,
        "held" => 4,
        "done" => 5,
        "canceled" => 6,
        _ => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::super::model::{ClosedWindow, enrich, rows};
    use super::super::reads::{Reads, Verdicts};
    use super::*;
    use crate::ff::BranchList;
    use crate::log::{Event, EventId, Kind};

    /// The tests' clock: far enough past every fixture time that an age
    /// is whatever the fixture says it is.
    const NOW: i64 = 1_000_000;
    const DAY: i64 = 24 * 60 * 60;

    fn event(id: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind,
        }
    }

    /// One filing, with the stored fields every axis here reads.
    fn filed(
        id: &str,
        time: i64,
        status: &str,
        priority: &str,
        assignee: Option<&str>,
        labels: &[&str],
    ) -> Event {
        event(
            id,
            time,
            Kind::Filed {
                procedure: None,
                subject: format!("subject of {id}"),
                body: String::new(),
                status: status.to_string(),
                assignee: assignee.map(str::to_string),
                priority: priority.to_string(),
                labels: labels.iter().map(|label| label.to_string()).collect(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        )
    }

    fn moved(id: &str, time: i64, flight: &str, to: &str) -> Event {
        event(
            id,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: to.to_string(),
                reason: None,
            },
        )
    }

    /// A repository that knows nothing: every axis under test is a
    /// stored field, so the reads stay out of the way.
    fn reads() -> Reads {
        Reads {
            ops: Vec::new(),
            branches: BranchList {
                named: Vec::new(),
                anonymous: Vec::new(),
            },
            current_branch: None,
            worktrees: Vec::new(),
            orphans: Vec::new(),
        }
    }

    fn flights(events: &[Event]) -> Vec<FlightView> {
        rows(fold(events), &reads(), &Verdicts::default(), NOW, 0).flights
    }

    fn ids(views: &[FlightView]) -> Vec<&str> {
        views.iter().map(|view| view.id.as_str()).collect()
    }

    /// The folded answer flattened to what a render reads off it:
    /// each column's key and the ids in it, in order.
    fn columns(folded: &Folded) -> Vec<(Option<&str>, Vec<&str>)> {
        folded
            .groups
            .iter()
            .map(|group| (group.key.as_deref(), ids(&group.rows)))
            .collect()
    }

    #[test]
    fn a_query_round_trips_through_its_own_codec() {
        // The card's own example, plus the two flags, spelled the way a
        // person would type it into the address bar.
        let raw = "status=ready,in_progress&priority=high&label=infra&subject=contains:parser\
                   &filed=after:3d&group=assignee&sub=priority&order=-changed&closed=10d\
                   &empty=true&mode=board&show=ref,status,assignee,label,age";
        let query = Query::parse(raw).expect("the example parses");

        assert_eq!(query.filters.len(), 5);
        assert_eq!(query.filters[0].field, Field::Status);
        assert_eq!(query.filters[0].op, Op::Is);
        assert_eq!(
            query.filters[0].value,
            Value::Words(vec!["ready".to_string(), "in_progress".to_string()])
        );
        assert_eq!(query.group, Some(Field::Assignee));
        assert_eq!(query.subgroup, Some(Field::Priority));
        assert_eq!(
            query.order,
            Order {
                field: Field::Changed,
                descending: true
            }
        );
        assert_eq!(query.closed, ClosedWindow::Span(10 * DAY));
        assert!(query.empty_groups);
        assert_eq!(query.mode, Mode::Board);
        assert_eq!(query.show.len(), 5);

        assert_eq!(query.render(), raw, "rendering is the parse's inverse");
        assert_eq!(
            Query::parse(&query.render()).expect("the render parses"),
            query
        );

        // The default is the empty string in both directions, so an
        // unfiltered board's link carries nothing at all.
        assert_eq!(Query::default().render(), "");
        assert_eq!(Query::parse("").expect("empty"), Query::default());
        assert_eq!(
            Query::parse("?").expect("bare question mark"),
            Query::default()
        );

        // A span's spelling normalizes to the largest unit that divides
        // it; the value it stands for is what round-trips.
        let week = Query::parse("closed=7d").expect("a week");
        assert_eq!(week.render(), "closed=1w");
        assert_eq!(Query::parse(&week.render()).expect("re-parses"), week);
    }

    #[test]
    fn a_label_carrying_a_comma_survives_the_round_trip() {
        // Every structural character at once: the separator, the
        // operator's colon, the pair's equals, the param's ampersand,
        // the escape itself, and a space.
        let awkward = "needs, maybe: a & b = 50% off/on";
        let query = Query {
            filters: vec![
                Filter::new(
                    Field::Label,
                    Op::Is,
                    Value::Words(vec![awkward.to_string(), "plain".to_string()]),
                )
                .expect("a label takes is"),
            ],
            ..Query::default()
        };
        let rendered = query.render();
        assert!(
            !rendered.contains("needs, maybe"),
            "the comma is encoded, not written raw: {rendered}"
        );
        assert_eq!(Query::parse(&rendered).expect("round trip"), query);

        // And the filter still matches the label it was written for.
        let mut view = flights(&[filed("pi.1", 10, "triage", "none", None, &[awkward])]);
        assert!(holds(&query.filters[0], &view.remove(0), NOW));
    }

    #[test]
    fn a_relative_date_stays_relative_through_a_round_trip() {
        let query = Query::parse("filed=after:3d&changed=before:2w").expect("two moments");
        assert_eq!(
            query.filters[0].value,
            Value::When(When::Ago(3 * DAY)),
            "a span stays a span — resolving it at parse would freeze a saved view"
        );
        assert_eq!(query.filters[1].value, Value::When(When::Ago(14 * DAY)));
        assert_eq!(query.render(), "filed=after:3d&changed=before:2w");

        // The absolute form is the other spelling, and it round-trips
        // as itself.
        let query = Query::parse("filed=after:@1700000000").expect("an epoch");
        assert_eq!(query.filters[0].value, Value::When(When::At(1_700_000_000)));
        assert_eq!(query.render(), "filed=after:@1700000000");

        // And the relative one actually means the last three days.
        let views = flights(&[
            filed("pi.1", NOW - DAY, "triage", "none", None, &[]),
            filed("pi.2", NOW - 10 * DAY, "triage", "none", None, &[]),
        ]);
        let filter = &Query::parse("filed=after:3d").expect("recent").filters[0];
        assert!(holds(filter, &views[0], NOW));
        assert!(!holds(filter, &views[1], NOW));
    }

    #[test]
    fn an_unknown_field_refuses_and_names_it() {
        let err = Query::parse("stauts=ready").expect_err("a typo refuses");
        assert_eq!(err.id(), "usage/unknown-field");
        assert!(err.to_string().contains("`stauts`"), "{err}");
        assert!(
            err.to_string().contains("status"),
            "and names what is: {err}"
        );
        assert_eq!(err.exits(), ["ff tower"]);

        // Every axis is closed the same way, each with its own id.
        for (raw, id) in [
            ("group=subject", "usage/not-groupable"),
            ("order=label", "usage/not-orderable"),
            ("show=body", "usage/not-showable"),
            ("comments=3", "usage/not-filterable"),
            ("status=isnt:ready", "usage/unknown-operator"),
            ("filed=after:soon", "usage/bad-time"),
            ("closed=soon", "usage/bad-window"),
            ("empty=maybe", "usage/bad-flag"),
            ("mode=kanban", "usage/bad-mode"),
            ("status", "usage/bad-query"),
        ] {
            let err = Query::parse(raw).expect_err(raw);
            assert_eq!(err.id(), id, "{raw}: {err}");
        }
    }

    #[test]
    fn a_field_refuses_an_operator_it_has_no_meaning_for() {
        for (raw, field, op) in [
            ("subject=before:3d", "subject", "before"),
            ("status=contains:read", "status", "contains"),
            ("label=after:3d", "label", "after"),
            ("filed=today", "filed", "is"),
        ] {
            let err = Query::parse(raw).expect_err(raw);
            assert_eq!(err.id(), "usage/bad-operator", "{raw}");
            let message = err.to_string();
            assert!(message.contains(field) && message.contains(op), "{message}");
        }

        // The checked constructor is the same refusal, for a query
        // assembled in memory rather than parsed.
        let err = Filter::new(Field::Status, Op::Contains, Value::Text("re".to_string()))
            .expect_err("status takes no contains");
        assert_eq!(err.id(), "usage/bad-operator");
        assert!(Query::default().check().is_ok());
    }

    #[test]
    fn an_unknown_value_parses_and_matches_nothing() {
        // A view saved by a newer tower must stay parseable here: field
        // names refuse, values never do.
        let query = Query::parse("status=parked").expect("an unheard-of status parses");
        let folded = query.fold(
            flights(&[
                filed("pi.1", 10, "triage", "none", None, &[]),
                filed("pi.2", 20, "ready", "none", None, &[]),
            ]),
            NOW,
        );
        assert!(
            folded.groups.is_empty(),
            "nothing matched, so no column stands"
        );
        assert_eq!(folded.filtered, 2);

        // And the negation of an unknown value keeps everything.
        let query = Query::parse("status=not:parked").expect("parses");
        let folded = query.fold(
            flights(&[filed("pi.1", 10, "triage", "none", None, &[])]),
            NOW,
        );
        assert_eq!(folded.filtered, 0);
    }

    #[test]
    fn the_two_counts_never_count_one_flight_twice() {
        // Four closed with a window of one, and three live of which the
        // filter keeps one.
        let events = [
            filed("pi.1", 10, "triage", "none", None, &[]),
            filed("pi.2", 20, "triage", "none", None, &[]),
            filed("pi.3", 30, "ready", "none", None, &[]),
            filed("pi.4", 40, "triage", "none", None, &[]),
            filed("pi.5", 50, "triage", "none", None, &[]),
            filed("pi.6", 60, "triage", "none", None, &[]),
            filed("pi.7", 70, "triage", "none", None, &[]),
            moved("pi.8", NOW - 400, "pi.4", "done"),
            moved("pi.9", NOW - 300, "pi.5", "done"),
            moved("pi.10", NOW - 200, "pi.6", "canceled"),
            moved("pi.11", NOW - 100, "pi.7", "done"),
        ];
        let all = flights(&events);
        assert_eq!(all.len(), 7);

        let query = Query::parse("status=ready&closed=1").expect("parses");
        let folded = query.fold(all, NOW);
        let landed: usize = folded.groups.iter().map(|group| group.count).sum();

        assert_eq!(folded.hidden, 3, "four closed, one window");
        assert_eq!(
            folded.filtered, 3,
            "the survivor of the window plus two live rows the filter rejected"
        );
        assert_eq!(landed, 1);
        assert_eq!(folded.hidden + folded.filtered + landed, 7);
    }

    #[test]
    fn grouping_by_labels_puts_a_flight_in_every_column_it_carries() {
        let folded = Query::parse("group=label").expect("parses").fold(
            flights(&[
                filed("pi.1", 10, "triage", "none", None, &["infra", "docs"]),
                filed("pi.2", 20, "triage", "none", None, &["docs"]),
                filed("pi.3", 30, "triage", "none", None, &[]),
            ]),
            NOW,
        );
        assert_eq!(
            columns(&folded),
            [
                (Some("docs"), vec!["pi.1", "pi.2"]),
                (Some("infra"), vec!["pi.1"]),
                (None, vec!["pi.3"]),
            ],
            "the labeled columns alphabetically, the unlabeled column last"
        );
        // A flight in two columns is one flight: the counts stay honest
        // because nothing was hidden or filtered.
        assert_eq!(folded.hidden, 0);
        assert_eq!(folded.filtered, 0);
    }

    #[test]
    fn an_empty_group_is_dropped_unless_the_query_asks_for_it() {
        let views = flights(&[filed("pi.1", 10, "triage", "none", None, &[])]);

        let folded = Query::default().fold(views.clone(), NOW);
        assert_eq!(columns(&folded), [(Some("triage"), vec!["pi.1"])]);

        let folded = Query::parse("empty=true").expect("parses").fold(views, NOW);
        let keys: Vec<Option<&str>> = folded
            .groups
            .iter()
            .map(|group| group.key.as_deref())
            .collect();
        assert_eq!(
            keys,
            [
                Some("triage"),
                Some("waiting"),
                Some("ready"),
                Some("in_progress"),
                Some("held"),
                Some("closed"),
            ],
            "the whole vocabulary, in lifecycle order"
        );
        assert_eq!(folded.groups[1].count, 0);
    }

    #[test]
    fn an_unknown_priority_still_sorts_after_none() {
        let folded = Query::parse("group=&order=priority").expect("parses").fold(
            flights(&[
                filed("pi.1", 10, "triage", "blocker", None, &[]),
                filed("pi.2", 20, "triage", "none", None, &[]),
                filed("pi.3", 30, "triage", "urgent", None, &[]),
            ]),
            NOW,
        );
        assert_eq!(
            columns(&folded),
            [(None, vec!["pi.3", "pi.2", "pi.1"])],
            "urgent, none, then the word this binary has never heard of"
        );

        // And grouping by it keeps the vocabulary's ladder, with the
        // unknown column after every known one.
        let folded = Query::parse("group=priority").expect("parses").fold(
            flights(&[
                filed("pi.1", 10, "triage", "blocker", None, &[]),
                filed("pi.2", 20, "triage", "none", None, &[]),
                filed("pi.3", 30, "triage", "urgent", None, &[]),
            ]),
            NOW,
        );
        assert_eq!(
            columns(&folded),
            [
                (Some("urgent"), vec!["pi.3"]),
                (Some("none"), vec!["pi.2"]),
                (Some("blocker"), vec!["pi.1"]),
            ]
        );
    }

    #[test]
    fn the_default_query_folds_to_todays_board() {
        // A board with something in every section, an unroutable status
        // that belongs in none, and four closed flights against the
        // compiled-in window of three.
        let events = [
            filed("pi.1", 10, "triage", "low", None, &[]),
            filed("pi.2", 20, "triage", "urgent", None, &[]),
            filed("pi.3", 30, "waiting", "none", None, &[]),
            filed("pi.4", 40, "ready", "high", Some("me"), &[]),
            filed("pi.5", 50, "ready", "medium", Some("agent"), &[]),
            filed("pi.6", 60, "in_progress", "none", Some("agent"), &[]),
            filed("pi.7", 70, "held", "none", None, &[]),
            filed("pi.8", 80, "triage", "none", None, &[]),
            filed("pi.9", 90, "triage", "none", None, &[]),
            filed("pi.10", 100, "triage", "none", None, &[]),
            filed("pi.11", 110, "triage", "none", None, &[]),
            moved("pi.12", 120, "pi.8", "parked"),
            moved("pi.13", NOW - 4_000, "pi.9", "done"),
            moved("pi.14", NOW - 3_000, "pi.10", "canceled"),
            moved("pi.15", NOW - 2_000, "pi.11", "done"),
            filed("pi.16", 160, "done", "none", None, &[]),
        ];
        let board = enrich(
            fold(&events),
            &reads(),
            &Verdicts::default(),
            NOW,
            0,
            ClosedWindow::default(),
        );
        let folded = Query::default().fold(flights(&events), NOW);

        assert_eq!(
            columns(&folded),
            [
                (Some("triage"), ids(&board.triage)),
                (Some("waiting"), ids(&board.waiting)),
                (Some("ready"), ids(&board.ready)),
                (Some("in_progress"), ids(&board.in_progress)),
                (Some("held"), ids(&board.held)),
                (Some("closed"), ids(&board.closed)),
            ],
            "the default query is today's board, section for section and row for row"
        );
        assert_eq!(
            ids(&board.closed),
            ["pi.11", "pi.10", "pi.9"],
            "the closed column keeps the window's own newest-first order"
        );
        assert_eq!(
            folded.hidden, 1,
            "one closed flight beyond the compiled-in window of three"
        );
        assert_eq!(folded.filtered, 0);
        assert!(
            !folded
                .groups
                .iter()
                .any(|group| group.rows.iter().any(|view| view.id == "pi.8")),
            "a status this binary has never heard of routes nowhere here either"
        );
    }
}
