//! The plugin body: the `tower` skill, the manual a session loads on its
//! own, shipped through `--ff-skill tower` today; and the two commands
//! the plugin name namespaces as `/tower:plan` (attended) and
//! `/tower:loop` (unattended), which #107 ships as `tower-plan` and
//! `tower-loop`. Nothing here touches disk.
//!
//! All three live as markdown next to the Rust, embedded whole. The
//! embedded constants are the staleness fingerprint — byte drift on disk
//! reads as "an older tower wrote it" — so the files carry no version or
//! hash of their own.

/// The `/tower:plan` command: load the board with a human present.
pub const PLAN: &str = include_str!("plan.md");

/// The `/tower:loop` command: drain the board unattended.
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

    /// The opposite of the commands' rule: a skill is named by its
    /// `name:` key, so fufu installs it under that name.
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
    fn the_manual_teaches_no_retired_verb() {
        for retired in [
            "ff tower requeue",
            "ff tower promote",
            "ff tower sync",
            "ff tower log",
            "-p <name>",
        ] {
            assert!(!SKILL.contains(retired), "the manual teaches `{retired}`");
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
