//! What the binary answers `--ff-skill` with: one skill, `tower`, the
//! manual a session loads on its own. Nothing here touches disk.
//!
//! It lives as markdown next to the Rust, embedded whole. The embedded
//! constant is the staleness fingerprint — byte drift on disk reads as
//! "an older tower wrote it" — so the file carries no version or hash
//! of its own.

/// The `tower` skill: the advanced manual, on fufu's model.
pub const SKILL: &str = include_str!("skill.md");

#[cfg(test)]
mod tests {
    use super::*;

    fn front_matter<'a>(name: &str, text: &'a str) -> Vec<&'a str> {
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("---"), "{name}: front matter first");
        lines.take_while(|line| *line != "---").collect()
    }

    #[test]
    fn the_manual_leads_with_its_front_matter() {
        let head = front_matter("skill", SKILL);
        assert!(
            head.contains(&"name: tower"),
            "the skill is named by its key: {head:?}"
        );
        assert!(
            head.iter().any(|line| line.starts_with("description: ")),
            "the description is what the model loads it by: {head:?}"
        );
        assert!(SKILL.contains("\n# tower\n"));
    }

    /// fufu's cap on its own page, so the manual stays a page.
    #[test]
    fn the_manual_fits_the_budget() {
        assert!(SKILL.len() <= 16_000, "{} bytes", SKILL.len());
    }

    /// The drift the worked examples carry, kept out of the manual: no
    /// `requeue`, `promote`, `sync`, or `log` verb exists, and `-p` is
    /// priority rather than a procedure.
    #[test]
    fn no_skill_teaches_a_retired_verb() {
        for retired in [
            "atc requeue",
            "atc promote",
            "atc sync",
            "atc log",
            "-p <name>",
        ] {
            assert!(!SKILL.contains(retired), "the manual teaches `{retired}`");
        }
    }
}
