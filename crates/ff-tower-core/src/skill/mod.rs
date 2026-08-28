//! A skill: the prose an agent-crewed part is flown with, as a file.
//!
//! DESIGN.md's *Skills*, loaded. A skill is instructions the harness
//! executes, not a process tower spawns — policy in markdown the user can
//! fork, layered exactly as procedures are: built-in, then
//! `~/.config/tower/skills/*.md`, then `.tower/skills/*.md` under the
//! main worktree, the same name replacing wholesale. The content is
//! opaque UTF-8 here; the name is the file stem, and the only structure
//! this module reads is an optional front matter block whose
//! `description:` line becomes the listing's summary.
//!
//! The seam is one-way on purpose. A procedure's part names a skill by
//! string; the skill store never reads procedures, and nothing validates
//! the link at load — a procedure naming a skill nothing installs still
//! loads and flies, and `ff tower doctor` is where the unresolved name
//! surfaces (principle 5: observe and complain, never enforce).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use crate::procedure::Source;

pub type Result<T> = std::result::Result<T, Error>;

/// One skill, loaded. `source` is the loader's, never the file's — a
/// skill cannot claim to be built in.
#[derive(Debug, Clone)]
pub struct Skill {
    /// The file stem — layering and lookup key both.
    pub name: String,
    /// The markdown, byte for byte as the file holds it.
    pub text: String,
    pub source: Source,
}

impl Skill {
    /// The one-line summary a listing prints: the front matter's
    /// `description:` when the file opens with a `---` block carrying
    /// one, else the first non-empty body line with a leading `# `
    /// stripped.
    pub fn summary(&self) -> &str {
        if let Some(description) = front_matter_description(&self.text) {
            return description;
        }
        body(&self.text)
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(|line| line.strip_prefix("# ").unwrap_or(line))
            .unwrap_or("")
    }
}

/// The `description:` value inside a leading `---` front matter block,
/// when both exist.
fn front_matter_description(text: &str) -> Option<&str> {
    let (matter, _) = split_front_matter(text)?;
    for line in matter.lines() {
        if let Some(value) = line.trim().strip_prefix("description:") {
            return Some(value.trim());
        }
    }
    None
}

/// A leading `---` front matter block split from what follows it:
/// `(matter, body)`, or `None` when the file does not open with one.
fn split_front_matter(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---")?;
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))?;
    let end = rest.find("\n---")?;
    let matter = &rest[..end];
    let after = &rest[end + "\n---".len()..];
    let body = after.find('\n').map(|nl| &after[nl + 1..]).unwrap_or("");
    Some((matter, body))
}

/// The markdown past the front matter block — the whole text when there
/// is no block.
fn body(text: &str) -> &str {
    match split_front_matter(text) {
        Some((_, body)) => body,
        None => text,
    }
}

/// What is installed, keyed by name — the more specific layer's file,
/// whole.
#[derive(Debug, Default)]
pub struct Registry {
    by_name: BTreeMap<String, Skill>,
}

impl Registry {
    /// The skill under this name, when one is installed.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.by_name.get(name)
    }

    /// Every installed skill, by name.
    pub fn skills(&self) -> impl Iterator<Item = &Skill> {
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

    /// Layered in: the same name replaces wholesale, never line by line.
    /// Half a skill from one layer and half from another would be prose
    /// nobody wrote.
    fn insert(&mut self, skill: Skill) {
        self.by_name.insert(skill.name.clone(), skill);
    }
}

/// The three skills shipped in the binary. Name beside text, because
/// markdown carries no name field — the registry key is the file stem
/// either way.
const BUILT_INS: [(&str, &str); 3] = [
    ("plan", include_str!("builtin/plan.md")),
    ("review", include_str!("builtin/review.md")),
    ("work", include_str!("builtin/work.md")),
];

/// The three layers, lowest first: built-in, then user, then repository.
///
/// A missing directory is an empty layer, not an error. A file inside a
/// directory being read that cannot be read *is* an error, named by
/// path: a skill you cannot see is worse than a refusal.
pub fn registry(repo_root: Option<&Path>) -> Result<Registry> {
    layered(user_dir().as_deref(), repo_root.map(repo_dir).as_deref())
}

/// The same, over two directories named outright rather than resolved
/// from the environment — procedure's split, for the same reason: a
/// caller that already knows where to look, a test with its own tempdirs
/// most of all, says so instead of setting a process-global variable.
pub fn layered(user: Option<&Path>, repo: Option<&Path>) -> Result<Registry> {
    let mut installed = Registry::default();
    for (name, text) in BUILT_INS {
        installed.insert(Skill {
            name: name.to_string(),
            text: text.to_string(),
            source: Source::BuiltIn,
        });
    }
    if let Some(dir) = user {
        read_dir(&mut installed, dir, Source::User)?;
    }
    if let Some(dir) = repo {
        read_dir(&mut installed, dir, Source::Repo)?;
    }
    Ok(installed)
}

/// Where your own skills live: `$XDG_CONFIG_HOME/tower/skills`, or
/// `~/.config/tower/skills` when that variable is unset. `None` when
/// neither variable says where home is.
pub fn user_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(Path::new(&xdg).join("tower").join("skills"));
    }
    let home = std::env::var_os("HOME").filter(|value| !value.is_empty())?;
    Some(
        Path::new(&home)
            .join(".config")
            .join("tower")
            .join("skills"),
    )
}

/// Where the team's live, under the main worktree — never the invoking
/// one, so every bay sees the same skills.
pub fn repo_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".tower").join("skills")
}

/// Every `*.md` in one directory, in file-name order so a layer reads
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
        if path.extension().is_some_and(|ext| ext == "md") {
            paths.push(path);
        }
    }
    paths.sort();

    for path in paths {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| Error::Invalid {
                at: path.display().to_string(),
                detail: "the file name is not UTF-8, so it cannot name a skill".to_string(),
            })?
            .to_string();
        let text = std::fs::read_to_string(&path).map_err(|err| Error::Invalid {
            at: path.display().to_string(),
            detail: err.to_string(),
        })?;
        installed.insert(Skill {
            name,
            text,
            source: source(path),
        });
    }
    Ok(())
}

/// What a skill layer can be refused for — one variant, because content
/// is opaque: only the reading can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A directory or file that cannot be read, or bytes that are not
    /// UTF-8. Carries the path and the system's own words.
    #[error("{at}: {detail}")]
    Invalid { at: String, detail: String },
}

impl Error {
    /// The stable id, tower's `category/kebab-case`.
    pub fn id(&self) -> &'static str {
        match self {
            Error::Invalid { .. } => "skill/invalid",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_ins_load_with_front_matter_and_a_body() {
        // A shipped skill without the front matter a harness redirect
        // depends on would be the worst bug in this module; it fails
        // here instead.
        let installed = layered(None, None).expect("the built-ins load");
        assert_eq!(installed.names(), ["plan", "review", "work"]);
        assert_eq!(installed.len(), 3);
        assert!(!installed.is_empty());

        for skill in installed.skills() {
            assert_eq!(skill.source, Source::BuiltIn, "{}", skill.name);
            assert!(skill.text.starts_with("---\n"), "{}", skill.name);
            let (matter, markdown) = split_front_matter(&skill.text).expect("front matter");
            assert!(
                matter.contains(&format!("name: tower-{}", skill.name)),
                "{}: {matter}",
                skill.name
            );
            assert!(!skill.summary().is_empty(), "{}", skill.name);
            assert!(
                skill.summary() != skill.text,
                "{}: the summary is the description, not the file",
                skill.name
            );
            assert!(
                markdown.contains(&format!("# {}", skill.name)),
                "{}: a body follows the block",
                skill.name
            );
        }
    }

    #[test]
    fn the_summary_is_the_front_matter_description() {
        let installed = layered(None, None).expect("loads");
        let work = installed.get("work").expect("work");
        assert_eq!(
            work.summary(),
            "claim, do, hold or commit, repeat — the loop that pairs with `ff tower next`"
        );
    }

    #[test]
    fn without_front_matter_the_summary_is_the_first_line_unhashed() {
        let skill = Skill {
            name: "bare".to_string(),
            text: "\n# bare\n\nProse.\n".to_string(),
            source: Source::BuiltIn,
        };
        assert_eq!(skill.summary(), "bare");
    }

    #[test]
    fn a_user_file_replaces_the_built_in_wholesale() {
        let dir = tempfile::TempDir::new().unwrap();
        let user = dir.path().join("skills");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join("work.md"), "# mine\n\nMy loop.\n").unwrap();

        let installed = layered(Some(&user), None).expect("loads");
        assert_eq!(installed.names(), ["plan", "review", "work"]);
        let work = installed.get("work").expect("work");
        assert_eq!(work.text, "# mine\n\nMy loop.\n");
        assert_eq!(work.source, Source::User(user.join("work.md")));
        assert_eq!(work.summary(), "mine");
    }

    #[test]
    fn the_repo_layer_beats_the_user_layer() {
        let dir = tempfile::TempDir::new().unwrap();
        let user = dir.path().join("user");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(user.join("work.md"), "the user's\n").unwrap();
        std::fs::write(repo.join("work.md"), "the team's\n").unwrap();

        let installed = layered(Some(&user), Some(&repo)).expect("loads");
        let work = installed.get("work").expect("work");
        assert_eq!(work.text, "the team's\n");
        assert_eq!(work.source, Source::Repo(repo.join("work.md")));
    }

    #[test]
    fn a_missing_directory_is_an_empty_layer_and_a_new_name_joins_the_set() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("triage.md"), "# triage\n").unwrap();
        // The name is the stem; a non-`.md` neighbor is not a skill.
        std::fs::write(repo.join("notes.txt"), "not a skill").unwrap();

        let installed = layered(Some(&dir.path().join("never-made")), Some(&repo)).expect("loads");
        assert_eq!(installed.names(), ["plan", "review", "triage", "work"]);
        assert_eq!(installed.get("triage").expect("triage").summary(), "triage");
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused_by_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("bad.md"), [0xff, 0xfe, 0x00]).unwrap();

        let err = layered(None, Some(&repo)).expect_err("a refusal");
        assert_eq!(err.id(), "skill/invalid");
        assert!(
            err.to_string().contains("bad.md"),
            "the refusal names the path: {err}"
        );
    }
}
