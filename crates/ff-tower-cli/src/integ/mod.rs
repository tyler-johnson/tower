//! The plugin body: what will land under `~/.claude/skills/tower/` — a
//! `.claude-plugin/plugin.json` manifest and the two commands the plugin
//! name namespaces as `/tower:plan` (attended) and `/tower:loop`
//! (unattended). #39 wires the verbs that write and remove it; nothing
//! here touches disk.
//!
//! The commands live as markdown next to the Rust, embedded whole. The
//! embedded constants are the staleness fingerprint — byte drift on disk
//! reads as "an older tower wrote it" — so the files carry no version or
//! hash of their own, and the manifest is built from Cargo metadata
//! rather than checked in.

/// The `/tower:plan` command: load the board with a human present.
pub const PLAN: &str = include_str!("plan.md");

/// The `/tower:loop` command: drain the board unattended.
pub const LOOP: &str = include_str!("loop.md");

/// The plugin manifest. The name is what namespaces the commands, and the
/// version is the binary's — the manifest a different tower would write
/// differently is the drift signal.
pub fn manifest() -> serde_json::Value {
    serde_json::json!({
        "name": "tower",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "tower keeps people and agents from colliding on one repository",
        "homepage": env!("CARGO_PKG_REPOSITORY"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn front_matter<'a>(name: &str, text: &'a str) -> Vec<&'a str> {
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("---"), "{name}: front matter first");
        lines.take_while(|line| *line != "---").collect()
    }

    #[test]
    fn each_command_leads_with_its_front_matter() {
        for (name, text) in [("plan", PLAN), ("loop", LOOP)] {
            let head = front_matter(name, text);
            assert!(
                head.iter().any(|line| line.starts_with("description: ")),
                "{name}: the description is what the command picker shows: {head:?}"
            );
            assert!(
                !head.iter().any(|line| line.starts_with("name:")),
                "{name}: a command is named by its filename, not a key: {head:?}"
            );
        }
        assert!(
            front_matter("plan", PLAN)
                .iter()
                .any(|line| line.starts_with("argument-hint: ")),
            "plan takes its goal on the command line"
        );
    }

    #[test]
    fn each_body_carries_its_heading() {
        assert!(PLAN.contains("\n# plan\n"));
        assert!(LOOP.contains("\n# loop\n"));
    }

    #[test]
    fn the_manifest_names_the_plugin() {
        let manifest = manifest();
        assert_eq!(manifest["name"], "tower");
        for key in ["version", "description", "homepage"] {
            assert!(
                !manifest[key].as_str().unwrap_or_default().is_empty(),
                "{key} is empty"
            );
        }
    }

    #[test]
    fn the_load_bearing_verbs_survive_rewording() {
        for verb in ["ff tower next", "ff tower skills", "hold"] {
            assert!(LOOP.contains(verb), "loop lost `{verb}`");
        }
        assert!(
            PLAN.contains("ff tower procedures"),
            "plan lost the shelf check"
        );
    }
}
