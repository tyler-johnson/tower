//! A procedure: the named recipe for one shape of work, as data.
//!
//! DESIGN.md's *Procedures*, loaded. A definition is a name, optional
//! match rules, and the flights it stamps out — each with the same
//! fields any flight carries, pre-filled — no conditions and no loops,
//! because principle 13 puts every conditional in the skill an
//! agent-assigned flight points at. What is here is a parser, a
//! validator, and two layers to read them from.
//!
//! **The definition is read once, at file time.** Nothing in this module
//! is consulted by the fold or by a render: `file` reads a definition,
//! copies each flight's fields into the log, and everything downstream
//! works off the log alone. Editing a definition therefore never disturbs
//! a flight already in the air, which is the property that makes forking
//! one mid-week safe.
//!
//! Two closures are deliberate. `deny_unknown_fields` on every wire
//! struct means a typo'd key is a refusal rather than a silently ignored
//! line — the discipline that keeps a config format from drifting into a
//! language; an old `[[part]]` file refuses loudly by path for the same
//! reason. And `assignee`/`done` are closed enums *here* while staying
//! free strings in the log: a known kind whose body does not parse is an
//! error by design (`log/event.rs`), so one closed enum on the wire
//! would let a newer tower's value take an older tower's whole board
//! down rather than one flight. The refusal belongs where a person is
//! editing a file.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

pub use crate::model::Assignee;

pub type Result<T> = std::result::Result<T, Error>;

/// One procedure, loaded and validated.
///
/// `source` is the loader's, never the file's — a definition cannot claim
/// which layer it came from.
#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    pub name: String,
    /// What the flight's subject resolves against later; `branch` on
    /// `review`. Nothing derives from it yet.
    pub subject: Option<String>,
    /// Intake rules, run by the lazy pass over what sits in Triage;
    /// first match wins, and the routing event stores which rule fired.
    pub matches: Vec<Match>,
    /// Declaration order — the order `file` mints the flights in.
    pub flights: Vec<FlightDef>,
    pub source: Source,
}

impl Definition {
    /// The terminal flights when every one of them is an agent's, and
    /// `None` otherwise. Derived, never stored: nothing on the wire says
    /// this, and the surfaces that warn re-derive it from the flights.
    ///
    /// Terminal: nothing waits on it. DESIGN.md:338 — a procedure should
    /// end with you, and the warning fires only when no terminal flight
    /// does, because one human close is the boundary the rule is about.
    pub fn no_human_end(&self) -> Option<Vec<&str>> {
        let waited_on: HashSet<&str> = self
            .flights
            .iter()
            .flat_map(|flight| flight.after.iter().map(String::as_str))
            .collect();
        let terminal: Vec<&FlightDef> = self
            .flights
            .iter()
            .filter(|flight| !waited_on.contains(flight.id.as_str()))
            .collect();
        let all_agents = !terminal.is_empty()
            && terminal
                .iter()
                .all(|flight| flight.assignee != Assignee::Me);
        all_agents.then(|| terminal.iter().map(|flight| flight.id.as_str()).collect())
    }
}

/// One intake rule: a name — what the routing event records as having
/// fired — and the predicates, which all AND. `source`/`event` are
/// adapter provenance, which no folded flight carries, so a rule keyed
/// on them stays honestly inert until adapters exist; the field
/// predicates match what a person files with fields already on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Match {
    pub name: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    /// Membership in the flight's label set.
    #[serde(default)]
    pub label: Option<String>,
    /// Equality on the stored field, and the same for the two below.
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
}

impl Match {
    /// Whether any predicate is declared — a rule without one would be
    /// meaningless, and the validator refuses it.
    pub fn has_predicates(&self) -> bool {
        self.source.is_some()
            || self.event.is_some()
            || self.label.is_some()
            || self.priority.is_some()
            || self.skill.is_some()
            || self.assignee.is_some()
    }
}

/// One flight a procedure stamps out — the same fields any flight
/// carries, pre-filled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightDef {
    pub id: String,
    pub assignee: Assignee,
    /// The skill an agent-assigned flight is flown with — the seam that
    /// keeps structure in data and judgment in prose.
    #[serde(default)]
    pub skill: Option<String>,
    /// The flights this one waits on, by id. The DAG is these edges, and
    /// concurrency is the absence of a declaration.
    #[serde(default)]
    pub after: Vec<String>,
    #[serde(default)]
    pub done: Done,
    /// Whether filing should build a tree ahead of whoever flies this.
    #[serde(default)]
    pub bay: Option<Bay>,
    /// The priority the flight is born with; the field's default when
    /// unsaid. Free here because it is free on the flight.
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

/// What finishing a part means. Closed on purpose: four values cannot
/// grow into an expression language. `asserted` is the only one anything
/// derives today; the other three wait for the verbs that can see them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Done {
    #[default]
    Asserted,
    Committed,
    Promoted,
    Landed,
}

/// What a part asks of the pool. One value today: warm a bay for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bay {
    Warm,
}

impl Done {
    pub fn name(&self) -> &'static str {
        match self {
            Done::Asserted => "asserted",
            Done::Committed => "committed",
            Done::Promoted => "promoted",
            Done::Landed => "landed",
        }
    }
}

impl Bay {
    pub fn name(&self) -> &'static str {
        match self {
            Bay::Warm => "warm",
        }
    }
}

/// Which layer a definition came from, and the file it came from. Both
/// layers are directories, so every definition has a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    User(PathBuf),
    Repo(PathBuf),
}

impl Source {
    /// The layer's name — the word a listing prints.
    pub fn layer(&self) -> &'static str {
        match self {
            Source::User(_) => "user",
            Source::Repo(_) => "repo",
        }
    }

    /// The file it was read from.
    pub fn path(&self) -> &Path {
        match self {
            Source::User(path) | Source::Repo(path) => path,
        }
    }

    /// What a refusal names the definition by.
    fn describe(&self) -> String {
        self.path().display().to_string()
    }
}

/// `{"layer": …, "path": …}` — flat, the same two keys on every emitting
/// surface.
impl Serialize for Source {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("Source", 2)?;
        out.serialize_field("layer", self.layer())?;
        out.serialize_field("path", &self.path().display().to_string())?;
        out.end()
    }
}

/// What is installed, keyed by name — the more specific layer's
/// definition, whole.
#[derive(Debug, Default)]
pub struct Registry {
    by_name: BTreeMap<String, Definition>,
}

impl Registry {
    /// The definition under this name, when one is installed.
    pub fn get(&self, name: &str) -> Option<&Definition> {
        self.by_name.get(name)
    }

    /// The definition under this name, or the refusal that lists what
    /// is installed — for the surfaces where a missing name is an error
    /// rather than a branch.
    pub fn require(&self, name: &str) -> Result<&Definition> {
        self.get(name).ok_or_else(|| Error::NotFound {
            name: name.to_string(),
            installed: self.names().join(", "),
        })
    }

    /// Every installed definition, by name.
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.by_name.values()
    }

    /// The installed names, sorted — what a `not-found` refusal lists.
    pub fn names(&self) -> Vec<&str> {
        self.by_name.keys().map(String::as_str).collect()
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Layered in: the same name replaces wholesale, never field by
    /// field. Half a definition from one layer and half from another
    /// would be a shape nobody wrote down. `pub(crate)` for the pass's
    /// unit tests, which need a registry over hand-loaded definitions.
    pub(crate) fn insert(&mut self, definition: Definition) {
        self.by_name.insert(definition.name.clone(), definition);
    }
}

/// The registry as one payload — what a bare `procedures` envelope
/// carries as `data`. Named structs rather than ad-hoc maps so the key
/// and the field order are the same on every emitting surface.
#[derive(Serialize)]
pub struct Listing<'a> {
    pub procedures: Vec<&'a Definition>,
}

/// The detail payload: one definition, whole.
#[derive(Serialize)]
pub struct One<'a> {
    pub procedure: &'a Definition,
}

/// The two layers, lowest first: user, then repository. Nothing sits
/// under them — the engine ships empty, and the worked examples a person
/// copies in live in the repository's `docs/`.
///
/// A missing directory is an empty layer, not an error. A file inside a
/// directory being read that does not parse *is* an error, named by path:
/// a definition you cannot see is worse than a refusal.
pub fn registry(repo_root: Option<&Path>) -> Result<Registry> {
    layered(user_dir().as_deref(), repo_root.map(repo_dir).as_deref())
}

/// The same, over two directories named outright rather than resolved
/// from the environment. `registry` is this with the environment's
/// answers filled in; a caller that already knows where to look — a test
/// with its own tempdirs, most of all — says so instead of setting a
/// process-global variable.
pub fn layered(user: Option<&Path>, repo: Option<&Path>) -> Result<Registry> {
    let mut installed = Registry::default();
    if let Some(dir) = user {
        read_dir(&mut installed, dir, Source::User)?;
    }
    if let Some(dir) = repo {
        read_dir(&mut installed, dir, Source::Repo)?;
    }
    Ok(installed)
}

/// Where your own definitions live: `$XDG_CONFIG_HOME/tower/procedures`,
/// or `~/.config/tower/procedures` when that variable is unset. `None`
/// when neither variable says where home is.
pub fn user_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(Path::new(&xdg).join("tower").join("procedures"));
    }
    let home = std::env::var_os("HOME").filter(|value| !value.is_empty())?;
    Some(
        Path::new(&home)
            .join(".config")
            .join("tower")
            .join("procedures"),
    )
}

/// Where the team's live, under the main worktree — never the invoking
/// one. The anchor is `tower.bays`'s, for its reason: every bay must see
/// the same definitions.
pub fn repo_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".tower").join("procedures")
}

/// Every `*.toml` in one directory, in file-name order so a layer reads
/// the same way twice.
fn read_dir(
    installed: &mut Registry,
    dir: &Path,
    source: impl Fn(PathBuf) -> Source,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(Error::Invalid {
                at: dir.display().to_string(),
                detail: err.to_string(),
            });
        }
    };

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| Error::Invalid {
            at: dir.display().to_string(),
            detail: err.to_string(),
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            paths.push(path);
        }
    }
    paths.sort();

    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|err| Error::Invalid {
            at: path.display().to_string(),
            detail: err.to_string(),
        })?;
        installed.insert(load(&text, source(path))?);
    }
    Ok(())
}

/// Parse one definition and validate it. The name is the file's, not the
/// file name's — layering is keyed on what the definition calls itself.
pub fn load(text: &str, source: Source) -> Result<Definition> {
    let at = source.describe();
    let wire: Wire = toml::from_str(text).map_err(|err| Error::Invalid {
        at: at.clone(),
        detail: one_line(&err.to_string()),
    })?;
    let definition = Definition {
        name: wire.name,
        subject: wire.subject,
        matches: wire.matches,
        flights: wire.flights,
        source,
    };
    validate(&definition, &at)?;
    Ok(definition)
}

/// The TOML file's shape, exactly. `[[match]]` and `[[flight]]` are
/// TOML's array-of-tables spelling of the two plural fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    name: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default, rename = "match")]
    matches: Vec<Match>,
    #[serde(default, rename = "flight")]
    flights: Vec<FlightDef>,
}

/// Six refusals, in the order it is cheapest to be sure of them: no
/// flights, a rule with no predicates, two rules under one name, a
/// duplicate id, an `after` naming nothing, and a cycle. The human end is
/// not among them: [`Definition::no_human_end`] is a warning the surfaces
/// raise, because the file is personal.
fn validate(definition: &Definition, at: &str) -> Result<()> {
    let name = definition.name.clone();
    let at = at.to_string();

    if definition.flights.is_empty() {
        return Err(Error::NoParts { name, at });
    }

    let mut rules: HashSet<&str> = HashSet::new();
    for rule in &definition.matches {
        if !rule.has_predicates() {
            return Err(Error::EmptyRule {
                name,
                at,
                rule: rule.name.clone(),
            });
        }
        if !rules.insert(rule.name.as_str()) {
            return Err(Error::DuplicateRule {
                name,
                at,
                rule: rule.name.clone(),
            });
        }
    }

    let mut seen: HashSet<&str> = HashSet::new();
    for flight in &definition.flights {
        if !seen.insert(flight.id.as_str()) {
            return Err(Error::DuplicatePart {
                name,
                at,
                part: flight.id.clone(),
            });
        }
    }

    for flight in &definition.flights {
        for after in &flight.after {
            if !seen.contains(after.as_str()) {
                return Err(Error::UnknownAfter {
                    name,
                    at,
                    part: flight.id.clone(),
                    after: after.clone(),
                });
            }
        }
    }

    if let Some(part) = cycle(&definition.flights) {
        return Err(Error::Cyclic { name, at, part });
    }

    Ok(())
}

/// The id of a flight a back edge closes on, if `after` is not a DAG.
/// Depth-first with the classic three colors; the flights are few, and
/// the walk runs in declaration order so the id it names is
/// deterministic.
fn cycle(parts: &[FlightDef]) -> Option<String> {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let edges: HashMap<&str, &[String]> = parts
        .iter()
        .map(|part| (part.id.as_str(), part.after.as_slice()))
        .collect();
    let mut color: HashMap<&str, Color> = parts
        .iter()
        .map(|part| (part.id.as_str(), Color::White))
        .collect();

    // An explicit stack rather than recursion: a hand-written definition
    // is small, but nothing here should be able to blow one.
    for part in parts {
        if color[part.id.as_str()] != Color::White {
            continue;
        }
        let mut stack: Vec<(&str, usize)> = vec![(part.id.as_str(), 0)];
        color.insert(part.id.as_str(), Color::Gray);
        while let Some((id, step)) = stack.pop() {
            let after = edges[id];
            if step < after.len() {
                stack.push((id, step + 1));
                let next = after[step].as_str();
                match color[next] {
                    Color::Gray => return Some(next.to_string()),
                    Color::White => {
                        color.insert(next, Color::Gray);
                        stack.push((next, 0));
                    }
                    Color::Black => {}
                }
            } else {
                color.insert(id, Color::Black);
            }
        }
    }
    None
}

/// toml's own message, on one line. Its Display draws a source snippet
/// with a gutter, which reads well in a compiler and badly in a one-line
/// refusal; the gutter goes and the sentences stay.
fn one_line(detail: &str) -> String {
    let kept: Vec<&str> = detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('|'))
        .filter(|line| {
            !line
                .split_once(" | ")
                .is_some_and(|(head, _)| head.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    if kept.is_empty() {
        detail.trim().to_string()
    } else {
        kept.join(": ")
    }
}

/// What a definition can be refused for. Every variant has a stable id,
/// so a CLI can code it without re-deriving the reason.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// TOML that does not parse, a key nothing declares, or a directory
    /// that cannot be read. Carries the path and toml's own words.
    #[error("{at}: {detail}")]
    Invalid { at: String, detail: String },

    /// A procedure with no flights has nothing to file.
    #[error("procedure `{name}` ({at}) declares no flights")]
    NoParts { name: String, at: String },

    /// A rule with no predicates could never say what it matches.
    #[error("procedure `{name}` ({at}): rule `{rule}` declares no predicates")]
    EmptyRule {
        name: String,
        at: String,
        rule: String,
    },

    /// Two rules under one name: the routing event could not say which
    /// fired.
    #[error("procedure `{name}` ({at}) declares rule `{rule}` twice")]
    DuplicateRule {
        name: String,
        at: String,
        rule: String,
    },

    /// Two flights under one id: `after` could not say which it meant.
    #[error("procedure `{name}` ({at}) declares flight `{part}` twice")]
    DuplicatePart {
        name: String,
        at: String,
        part: String,
    },

    /// An edge to a flight that does not exist — a typo, every time.
    #[error("procedure `{name}` ({at}): flight `{part}` waits on `{after}`, which is not a flight")]
    UnknownAfter {
        name: String,
        at: String,
        part: String,
        after: String,
    },

    /// `after` closed on itself, so no flight could ever start.
    #[error("procedure `{name}` ({at}): `after` closes a cycle through flight `{part}`")]
    Cyclic {
        name: String,
        at: String,
        part: String,
    },

    /// A name the registry does not carry — the one refusal here about
    /// the asking rather than the definition. Raised by
    /// [`Registry::require`], never by the loader.
    #[error("no procedure `{name}` — {}", installed_note(installed))]
    NotFound { name: String, installed: String },
}

/// The tail of a `not-found` refusal. Empty is the normal state of a box
/// nobody has authored a definition on yet, and `installed: ` with
/// nothing after it reads like a bug rather than an answer.
fn installed_note(installed: &str) -> String {
    if installed.is_empty() {
        "nothing installed".to_string()
    } else {
        format!("installed: {installed}")
    }
}

impl Error {
    /// The stable id, tower's `category/kebab-case`.
    pub fn id(&self) -> &'static str {
        match self {
            Error::Invalid { .. } => "procedure/invalid",
            Error::NoParts { .. } => "procedure/no-parts",
            Error::EmptyRule { .. } => "procedure/empty-rule",
            Error::DuplicateRule { .. } => "procedure/duplicate-rule",
            Error::DuplicatePart { .. } => "procedure/duplicate-part",
            Error::UnknownAfter { .. } => "procedure/unknown-after",
            Error::Cyclic { .. } => "procedure/cyclic",
            Error::NotFound { .. } => "procedure/not-found",
        }
    }

    /// Commands that lead out of it. One answer for every variant: the
    /// list is where a bad definition and a missing name both get
    /// diagnosed.
    pub fn exits(&self) -> Vec<String> {
        vec!["ff tower procedures".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source for a hand-written definition: the loader wants one, and
    /// a refusal names the definition by its path, so the tests read the
    /// same way a real file's would.
    fn at(name: &str) -> Source {
        Source::Repo(PathBuf::from(name))
    }

    #[test]
    fn the_registry_starts_empty_and_says_so() {
        // Principle 12, asserted where it is cheapest: nothing sits under
        // the two layers, so a box with neither directory has nothing
        // installed at all.
        let installed = layered(None, None).expect("no layers is not an error");
        assert!(installed.is_empty());
        assert_eq!(installed.len(), 0);
        assert!(installed.names().is_empty());

        let err = installed
            .require("review")
            .expect_err("nothing is installed");
        assert_eq!(err.id(), "procedure/not-found");
        assert_eq!(err.to_string(), "no procedure `review` — nothing installed");
        assert_eq!(err.exits(), ["ff tower procedures"]);
    }

    #[test]
    fn require_answers_or_refuses_with_the_installed_list() {
        let dir = tempfile::TempDir::new().unwrap();
        let user = dir.path().join("procedures");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(
            user.join("review.toml"),
            "name = \"review\"\n[[flight]]\nid = \"verdict\"\nassignee = \"me\"\n",
        )
        .unwrap();
        std::fs::write(
            user.join("ticket.toml"),
            "name = \"ticket\"\n[[flight]]\nid = \"work\"\nassignee = \"me\"\n",
        )
        .unwrap();

        let installed = layered(Some(&user), None).expect("loads");
        assert_eq!(
            installed.require("review").expect("installed").name,
            "review"
        );

        let err = installed.require("nope").expect_err("not installed");
        assert_eq!(err.id(), "procedure/not-found");
        assert_eq!(
            err.to_string(),
            "no procedure `nope` — installed: review, ticket"
        );
        assert_eq!(err.exits(), ["ff tower procedures"]);
    }

    #[test]
    fn done_defaults_to_asserted_and_the_rest_parse() {
        let definition = load(
            r#"
name = "shapes"
[[flight]]
id       = "first"
assignee = "agent"
[[flight]]
id       = "last"
assignee = "me"
after    = ["first"]
done     = "landed"
"#,
            at("shapes.toml"),
        )
        .expect("loads");
        assert_eq!(definition.flights[0].done, Done::Asserted);
        assert_eq!(definition.flights[1].done, Done::Landed);
    }

    #[test]
    fn a_word_the_done_enum_does_not_carry_is_refused_by_path() {
        // The enum is closed here and open in the log: a completion word
        // a newer tower minted must not take an older tower's board
        // down, but the refusal belongs where a person is editing a file.
        let err = load(
            r#"
name = "shapes"
[[flight]]
id       = "only"
assignee = "me"
done     = "shipped"
"#,
            at("shapes.toml"),
        )
        .expect_err("an unrecognized word refuses");
        assert_eq!(err.id(), "procedure/invalid");
        let message = err.to_string();
        assert!(message.contains("shapes.toml"), "names the path: {message}");
        assert!(!message.contains('\n'), "one line: {message}");
    }

    #[test]
    fn a_rule_parses_with_field_predicates_and_a_bare_one_refuses() {
        let definition = load(
            r#"
name = "chores"
[[match]]
name  = "chore-label"
label = "chore"
[[match]]
name     = "handoffs"
priority = "high"
assignee = "agent"
[[flight]]
id       = "work"
assignee = "me"
"#,
            at("chores.toml"),
        )
        .expect("loads");
        assert_eq!(definition.matches.len(), 2);
        assert_eq!(definition.matches[0].name, "chore-label");
        assert_eq!(definition.matches[0].label.as_deref(), Some("chore"));
        assert!(definition.matches[0].source.is_none());
        assert_eq!(definition.matches[1].priority.as_deref(), Some("high"));
        assert_eq!(definition.matches[1].assignee.as_deref(), Some("agent"));

        // A nameless rule refuses through serde — the `[[part]]`
        // precedent: the loader is where an old grammar fails loudly.
        let err = load(
            r#"
name = "old"
[[match]]
source = "github"
event  = "review_requested"
[[flight]]
id       = "work"
assignee = "me"
"#,
            at("old.toml"),
        )
        .expect_err("a nameless rule refuses");
        assert_eq!(err.id(), "procedure/invalid");

        let err = load(
            r#"
name = "vacuous"
[[match]]
name = "everything"
[[flight]]
id       = "work"
assignee = "me"
"#,
            at("vacuous.toml"),
        )
        .expect_err("a rule with no predicates refuses");
        assert_eq!(err.id(), "procedure/empty-rule");
        assert_eq!(
            err.to_string(),
            "procedure `vacuous` (vacuous.toml): rule `everything` declares no predicates"
        );

        let err = load(
            r#"
name = "twice"
[[match]]
name  = "same"
label = "chore"
[[match]]
name  = "same"
label = "ops"
[[flight]]
id       = "work"
assignee = "me"
"#,
            at("twice.toml"),
        )
        .expect_err("two rules under one name refuse");
        assert_eq!(err.id(), "procedure/duplicate-rule");
        assert_eq!(
            err.to_string(),
            "procedure `twice` (twice.toml) declares rule `same` twice"
        );
    }

    #[test]
    fn an_unknown_key_is_refused() {
        let err = load(
            r#"
name = "typo"
[[flight]]
id       = "only"
assignee = "me"
skil     = "review"
"#,
            at("typo.toml"),
        )
        .expect_err("a typo'd key is a refusal");
        assert_eq!(err.id(), "procedure/invalid");
        assert!(!err.to_string().contains('\n'), "one line: {err}");
    }

    #[test]
    fn an_old_part_grammar_file_refuses_loudly() {
        // The pre-stored-model grammar: `[[part]]` with `crew`. The
        // `deny_unknown_fields` closure is what makes the refusal loud
        // rather than a definition silently missing its flights.
        let err = load(
            r#"
name = "old"
[[part]]
id   = "work"
crew = "you"
"#,
            at("old.toml"),
        )
        .expect_err("the old grammar refuses");
        assert_eq!(err.id(), "procedure/invalid");
    }
}
