//! What the binary answers `--ff-skill` with: three skills. `tower` is
//! the manual a session loads on its own; `tower-plan` loads the board
//! with a person present, and `tower-loop` drains it unattended. The
//! two intents land inside fufu's plugin, so they are typed
//! `/fufu:tower-plan` and `/fufu:tower-loop`. Nothing here touches disk.
//!
//! All three live as markdown next to the Rust, embedded whole. The
//! embedded constants are the staleness fingerprint — byte drift on disk
//! reads as "an older tower wrote it" — so the files carry no version or
//! hash of their own.

/// The `tower-plan` skill, `/fufu:tower-plan`: load the board with a
/// human present.
pub const PLAN: &str = include_str!("plan.md");

/// The `tower-loop` skill, `/fufu:tower-loop`: drain the board
/// unattended.
pub const LOOP: &str = include_str!("loop.md");

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

    /// A skill is named by its `name:` key, so fufu installs it under
    /// that name; `disable-model-invocation` keeps both intents behind
    /// a typed slash command rather than a description match.
    #[test]
    fn each_skill_leads_with_its_front_matter() {
        for (name, text, key) in [
            ("plan", PLAN, "name: tower-plan"),
            ("loop", LOOP, "name: tower-loop"),
        ] {
            let head = front_matter(name, text);
            assert!(
                head.contains(&key),
                "{name}: the skill is named by its key: {head:?}"
            );
            assert!(
                head.iter().any(|line| line.starts_with("description: ")),
                "{name}: the description is what the skill picker shows: {head:?}"
            );
            assert!(
                head.contains(&"disable-model-invocation: true"),
                "{name}: an intent is typed, never loaded on a match: {head:?}"
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

    /// The drift the worked examples carry, kept out of all three: no
    /// `requeue`, `promote`, `sync`, or `log` verb exists, and `-p` is
    /// priority rather than a procedure.
    #[test]
    fn no_skill_teaches_a_retired_verb() {
        for (name, text) in [("skill", SKILL), ("plan", PLAN), ("loop", LOOP)] {
            for retired in [
                "ff tower requeue",
                "ff tower promote",
                "ff tower sync",
                "ff tower log",
                "-p <name>",
            ] {
                assert!(!text.contains(retired), "{name} teaches `{retired}`");
            }
        }
    }

    #[test]
    fn the_load_bearing_verbs_survive_rewording() {
        for verb in [
            "ff tower next",
            "ff tower skills",
            "hold",
            "ff tower status",
        ] {
            assert!(LOOP.contains(verb), "loop lost `{verb}`");
        }
        assert!(
            PLAN.contains("ff tower procedures"),
            "plan lost the shelf check"
        );
    }
}
