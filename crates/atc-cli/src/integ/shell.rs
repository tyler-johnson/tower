//! The shells: bash, zsh, fish, powershell.
//!
//! One slug per shell, because the rc file and its syntax differ per
//! shell, but one trigger source, because every one of them installs
//! lines that call the same `atc trigger shell`. A terminal is a
//! session: the lines mint a session id once per interactive shell and
//! export it as `ATC_SHELL_SESSION` with the shell's pid as
//! `ATC_SHELL_PID`, renew the lease before every prompt, and release it
//! when the shell exits. The variable is the last row of core's session
//! table on purpose — a terminal's environment is inherited by every
//! agent launched from it, and the agent's own row must win — so an
//! agent started from a wired terminal is its own session.
//!
//! Marked-line editing, fufu's discipline: install appends lines carrying
//! the tower marker, uninstall removes exactly the marked lines, `-u`
//! replaces them with the current text. A hand-written line naming the
//! trigger is detected, reported, and never touched. The marker differs
//! from fufu's, so the two tools' lines share one rc file and each
//! filters only its own. Every path is env-resolved (HOME, ZDOTDIR,
//! XDG_CONFIG_HOME, SHELL) so tests stay hermetic. The one exception is
//! PowerShell's profile on Windows, which lives under the Documents
//! known folder rather than under any variable; `ATC_DOCUMENTS_DIR`
//! stands in for the known-folder lookup so the suite never writes a
//! real profile.

use std::path::{Path, PathBuf};

use super::settings::{Class, Event, Need};
use super::{Change, InstallOptions, Integration, Mechanism, Presence, Status, Wiring};
use crate::error::CliError;

/// The marker every written line carries.
const MARKER: &str = "# tower — added by `atc hook`";

/// The command the lines call.
const TRIGGER: &str = "atc trigger shell";

pub const SHELLS: [&str; 4] = ["bash", "zsh", "fish", "powershell"];

/// The console host's profile file name, the same under PowerShell 7 and
/// Windows PowerShell 5.1; only the directory differs.
const PROFILE: &str = "Microsoft.PowerShell_profile.ps1";

pub struct Shell {
    pub slug: &'static str,
}

/// The events for the record: what `atc trigger shell` does bare and
/// under `--end`. `class_of` is overridden below, since a shell has no
/// payload to name an event in.
const EVENTS: [Event; 2] = [
    Event {
        name: "prompt",
        matcher: None,
        class: Class::Activity,
        need: Need::Required,
    },
    Event {
        name: "end",
        matcher: None,
        class: Class::End,
        need: Need::Required,
    },
];

fn is_marked(line: &str) -> bool {
    line.contains(MARKER)
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The line ending a file already uses, so a rewrite keeps it. A profile
/// written by a Windows editor is CRLF, and a rewrite that rejoined it
/// with `\n` would flip every line of it on the first `atc unhook`.
fn line_ending(contents: &str) -> &'static str {
    if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// `$PROFILE` for the console host, one file for both PowerShells.
///
/// On Windows that is `<Documents>\PowerShell\...` for PowerShell 7 and
/// `<Documents>\WindowsPowerShell\...` for 5.1; the 7 file is the one
/// wired unless only the 5.1 file exists, and it is created when neither
/// does. `documents` is the known folder (OneDrive can redirect it away
/// from `<home>\Documents`, and PowerShell follows the redirect), with
/// `<home>\Documents` as the fallback when the lookup failed. Elsewhere it
/// is `$XDG_CONFIG_HOME/powershell/...`, or `~/.config/powershell/...`.
fn powershell_profile(
    windows: bool,
    home: &Path,
    documents: Option<&Path>,
    xdg: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    if !windows {
        let config = xdg.map_or_else(|| home.join(".config"), Path::to_path_buf);
        return config.join("powershell").join(PROFILE);
    }
    let documents = documents.map_or_else(|| home.join("Documents"), Path::to_path_buf);
    let seven = documents.join("PowerShell").join(PROFILE);
    let five = documents.join("WindowsPowerShell").join(PROFILE);
    if exists(&seven) || !exists(&five) {
        seven
    } else {
        five
    }
}

/// The Documents known folder, the way PowerShell itself resolves
/// `$PROFILE`: through the shell API, so a OneDrive redirect lands on the
/// file PowerShell reads rather than on a `<home>\Documents` it never
/// opens. `ATC_DOCUMENTS_DIR` wins when set, which is how the test suite
/// keeps a real profile out of reach. `None` when the lookup fails.
#[cfg(windows)]
fn documents_dir() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Documents, SHGetKnownFolderPath};

    if let Some(dir) = env_path("ATC_DOCUMENTS_DIR") {
        return Some(dir);
    }
    let mut wide: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: the folder id is a static GUID, a null token means the calling
    // user, and on success the API hands back a NUL-terminated buffer the
    // caller owns and releases with CoTaskMemFree — which happens below,
    // after the copy, on every path that got one.
    let hr =
        unsafe { SHGetKnownFolderPath(&FOLDERID_Documents, 0, std::ptr::null_mut(), &mut wide) };
    if hr < 0 || wide.is_null() {
        return None;
    }
    let mut len = 0;
    // SAFETY: the buffer is NUL-terminated per the API contract, and the
    // reads stop at the terminator.
    while unsafe { *wide.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` counts initialized u16s before the terminator.
    let dir = PathBuf::from(std::ffi::OsString::from_wide(unsafe {
        std::slice::from_raw_parts(wide, len)
    }));
    // SAFETY: the buffer was allocated by the shell for us to free, once.
    unsafe { CoTaskMemFree(wide.cast()) };
    Some(dir)
}

#[cfg(not(windows))]
fn documents_dir() -> Option<PathBuf> {
    None
}

fn rc_file(shell: &str) -> Result<PathBuf, CliError> {
    Ok(match shell {
        "bash" => super::home()?.join(".bashrc"),
        "zsh" => match env_path("ZDOTDIR") {
            Some(zdot) => zdot.join(".zshrc"),
            None => super::home()?.join(".zshrc"),
        },
        "fish" => {
            let config = match env_path("XDG_CONFIG_HOME") {
                Some(xdg) => xdg,
                None => super::home()?.join(".config"),
            };
            config.join("fish").join("config.fish")
        }
        "powershell" => powershell_profile(
            cfg!(windows),
            &super::home()?,
            documents_dir().as_deref(),
            env_path("XDG_CONFIG_HOME").as_deref(),
            |p| p.is_file(),
        ),
        other => unreachable!("{other} is not a shell slug"),
    })
}

/// The un-marked bodies of the lines; the caller appends the marker to
/// each.
///
/// Three rules every shell's lines obey. Mint only when no session is
/// inherited, so a nested shell, a subshell, and a script from the
/// prompt stay in their terminal's session. Install the release only
/// when this process owns the session — `ATC_SHELL_PID` is the shell's
/// own pid — so a nested interactive shell never releases its parent's
/// lease, and `exec bash` re-entering under the same pid still gets its
/// trap. Run the trigger with stdin from `/dev/null` and the release with
/// every stream closed: an interactive bash fed by a pipe hands its stdin
/// to `PROMPT_COMMAND`, and a hung-up pty makes any write fail.
///
/// Bash's EXIT trap fires only for the shell that set it, and the
/// `||`/`&&` chain leaves a re-source at status 0. Zsh runs EXIT traps in
/// subshells and `zshexit` only when the main shell exits, so the release
/// is a `zshexit_functions` entry; the `(I)` guards keep a re-source from
/// registering twice. Fish's shell-exit event is `fish_exit`, its pid is
/// `$fish_pid`, and redefining a function on re-source replaces it.
/// PowerShell wraps `prompt`, guarded so a second dot-source does not
/// wrap the wrapper, and registers `PowerShell.Exiting` for the release;
/// pwsh loads the profile for `-Command` and `-File` too, so the mint is
/// guarded against a scripted invocation. Bare `atc`, not the exe path:
/// a shell has `PATH`.
fn lines(shell: &str) -> Vec<String> {
    match shell {
        "bash" => vec![
            format!(
                r#"if [[ $- == *i* ]]; then [ -n "$ATC_SHELL_SESSION" ] || export ATC_SHELL_SESSION="$(atc session --mint)" ATC_SHELL_PID=$$; [ "$ATC_SHELL_PID" = "$$" ] && trap '{TRIGGER} --end </dev/null >/dev/null 2>&1' EXIT; fi"#
            ),
            format!(
                r#"[[ $PROMPT_COMMAND == *"{TRIGGER}"* ]] || PROMPT_COMMAND="{TRIGGER} </dev/null;$PROMPT_COMMAND""#
            ),
        ],
        "zsh" => vec![
            r#"if [[ $- == *i* ]] && [ -z "$ATC_SHELL_SESSION" ]; then export ATC_SHELL_SESSION="$(atc session --mint)" ATC_SHELL_PID=$$; fi"#.to_string(),
            format!("_tower_ambient() {{ {TRIGGER} </dev/null }}"),
            "(( ${precmd_functions[(I)_tower_ambient]} )) || precmd_functions+=(_tower_ambient)".to_string(),
            format!(
                "_tower_exit() {{ [[ $ATC_SHELL_PID == $$ ]] && {TRIGGER} --end </dev/null >/dev/null 2>&1 }}"
            ),
            "(( ${zshexit_functions[(I)_tower_exit]} )) || zshexit_functions+=(_tower_exit)".to_string(),
        ],
        "fish" => vec![
            r#"if status is-interactive; and test -z "$ATC_SHELL_SESSION"; set -gx ATC_SHELL_SESSION (atc session --mint); set -gx ATC_SHELL_PID $fish_pid; end"#.to_string(),
            format!("function _tower_ambient --on-event fish_prompt; {TRIGGER} </dev/null; end"),
            format!(
                r#"function _tower_exit --on-event fish_exit; test "$ATC_SHELL_PID" = "$fish_pid"; and {TRIGGER} --end </dev/null >/dev/null 2>&1; end"#
            ),
        ],
        "powershell" => vec![
            format!(
                "if (-not $env:ATC_SHELL_SESSION -and -not ([Environment]::GetCommandLineArgs() -match '^-(c|Command|f|File|e|ec|EncodedCommand)$')) {{ $env:ATC_SHELL_SESSION = (atc session --mint); $env:ATC_SHELL_PID = $PID; Register-EngineEvent PowerShell.Exiting -Action {{ {TRIGGER} --end | Out-Null }} | Out-Null }}"
            ),
            format!(
                "if (-not (Test-Path Function:_tower_prompt)) {{ $function:global:_tower_prompt = $function:prompt; function global:prompt {{ {TRIGGER} | Out-Null; _tower_prompt }} }}"
            ),
        ],
        _ => Vec::new(),
    }
}

/// The marked lines as they are written, joined with `eol`.
fn marked(shell: &str, eol: &str) -> String {
    lines(shell)
        .iter()
        .map(|line| format!("{line}  {MARKER}{eol}"))
        .collect()
}

/// `Wired` requires a marked line naming either the trigger command or a
/// `_tower_` function — zsh's wiring is several marked lines and not all
/// of them mention the command literally, so either alternative marks
/// the whole piece wired. An unmarked line naming the trigger is
/// hand-written: reported, never touched.
fn wiring(contents: &str, rc: &Path) -> Wiring {
    for line in contents.lines() {
        if is_marked(line) && (line.contains(TRIGGER) || line.contains("_tower_")) {
            return Wiring::Wired {
                mechanism: Mechanism::Rc,
                at: rc.to_path_buf(),
            };
        }
    }
    for line in contents.lines() {
        if !is_marked(line) && line.contains(TRIGGER) {
            return Wiring::HandWritten {
                at: rc.to_path_buf(),
            };
        }
    }
    Wiring::NotWired
}

/// The shell `$SHELL` names — but only when it is one of `SHELLS`, so an
/// exotic login shell is absent rather than guessed. PowerShell's binary
/// is `pwsh`, and its slug is not.
pub fn default_shell() -> Option<&'static str> {
    let shell = std::env::var("SHELL").ok()?;
    let name = Path::new(&shell).file_name()?.to_str()?;
    if name == "pwsh" {
        return Some("powershell");
    }
    SHELLS.into_iter().find(|s| *s == name)
}

/// Write the rc file whole through a sibling temp file and a rename, so
/// a shell sourcing it mid-write reads the old text or the new and never
/// half of either. The temp name is explicit: `with_extension` on
/// `.bashrc` would replace the whole name.
fn write(rc: &Path, contents: &str) -> Result<(), CliError> {
    if let Some(parent) = rc.parent() {
        std::fs::create_dir_all(parent).map_err(|err| super::failed(parent, err))?;
    }
    let name = rc
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = rc.with_file_name(format!("{name}.atc-tmp"));
    std::fs::write(&tmp, contents).map_err(|err| super::failed(&tmp, err))?;
    std::fs::rename(&tmp, rc).map_err(|err| {
        let _ = std::fs::remove_file(&tmp);
        super::failed(rc, err)
    })
}

impl Shell {
    fn rc(&self) -> Result<PathBuf, CliError> {
        rc_file(self.slug)
    }

    fn wiring(&self) -> Wiring {
        let Ok(rc) = self.rc() else {
            return Wiring::Unavailable("HOME is not set".into());
        };
        let contents = std::fs::read_to_string(&rc).unwrap_or_default();
        wiring(&contents, &rc)
    }
}

impl Integration for Shell {
    fn slug(&self) -> &'static str {
        self.slug
    }

    /// Every shell feeds one source: the rc lines they install differ in
    /// syntax and call the same command.
    fn source(&self) -> &'static str {
        "shell"
    }

    fn events(&self) -> &'static [Event] {
        &EVENTS
    }

    /// A shell names no event on stdin: bare is a prompt, activity; the
    /// `--end` flag names the end. Never a boundary.
    fn class_of(&self, name: Option<&str>) -> Option<Class> {
        match name {
            None | Some("") => Some(Class::Activity),
            Some(name) => EVENTS
                .iter()
                .find(|event| event.name == name)
                .map(|event| event.class),
        }
    }

    fn carries_payload(&self) -> bool {
        false
    }

    fn detect(&self) -> Presence {
        // The rc file is the evidence when there is one. A shell that is
        // the login shell but has never been configured still counts —
        // that is precisely the shell worth offering to wire. On Windows
        // PowerShell is that shell unconditionally: 5.1 ships with the
        // OS, and there is no `$SHELL` to consult.
        match self.rc() {
            Ok(rc) if rc.is_file() => Presence::Present { evidence: rc },
            Ok(rc) if default_shell() == Some(self.slug) => Presence::Present { evidence: rc },
            Ok(rc) if cfg!(windows) && self.slug == "powershell" => {
                Presence::Present { evidence: rc }
            }
            _ => Presence::Absent,
        }
    }

    fn status(&self) -> Status {
        Status {
            slug: self.slug,
            presence: self.detect(),
            wiring: self.wiring(),
            note: None,
            skill: None,
            stale: false,
        }
    }

    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let rc = self.rc()?;
        let contents = std::fs::read_to_string(&rc).unwrap_or_default();
        let eol = line_ending(&contents);
        match wiring(&contents, &rc) {
            Wiring::Wired { .. } => {
                return Ok(Change::unchanged(format!(
                    "already wired in {}",
                    rc.display()
                )));
            }
            Wiring::HandWritten { .. } => {
                return Ok(Change::unchanged(format!(
                    "{} already calls {TRIGGER} by hand — leaving it alone",
                    rc.display()
                )));
            }
            _ => {}
        }
        let mut updated = contents;
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push_str(eol);
        }
        updated.push_str(&marked(self.slug, eol));
        write(&rc, &updated)?;
        let mut change = Change::changed(format!("wired into {}", rc.display()));
        change
            .lines
            .push("restart the shell (or source the file) to activate it".into());
        Ok(change)
    }

    /// `-u`: the marked lines replaced by the current text, in one
    /// write, and no write at all when the bytes already match.
    fn repair(&self) -> Result<Change, CliError> {
        let rc = self.rc()?;
        let contents = std::fs::read_to_string(&rc).unwrap_or_default();
        let eol = line_ending(&contents);
        let kept: Vec<&str> = contents.lines().filter(|line| !is_marked(line)).collect();
        let mut updated = kept.join(eol);
        if !updated.is_empty() {
            updated.push_str(eol);
        }
        updated.push_str(&marked(self.slug, eol));
        if updated == contents {
            return Ok(Change::unchanged(format!(
                "already wired in {}",
                rc.display()
            )));
        }
        write(&rc, &updated)?;
        Ok(Change::changed(format!(
            "rewrote the session lines in {}",
            rc.display()
        )))
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let rc = self.rc()?;
        let Ok(contents) = std::fs::read_to_string(&rc) else {
            return Ok(Change::unchanged(format!(
                "nothing wired ({} not found)",
                rc.display()
            )));
        };
        match wiring(&contents, &rc) {
            Wiring::Wired { .. } => {}
            by_hand => {
                let mut change = Change::unchanged(format!("nothing wired in {}", rc.display()));
                if matches!(by_hand, Wiring::HandWritten { .. }) {
                    change.lines.push(format!(
                        "the {TRIGGER} line in {} was written by hand — not touching it",
                        rc.display()
                    ));
                }
                return Ok(change);
            }
        }
        // A hand-written line is never marked, so it survives this filter
        // with no special case.
        let eol = line_ending(&contents);
        let kept: Vec<&str> = contents.lines().filter(|line| !is_marked(line)).collect();
        let mut updated = kept.join(eol);
        if contents.ends_with('\n') && !updated.is_empty() {
            updated.push_str(eol);
        }
        write(&rc, &updated)?;
        Ok(Change::changed(format!(
            "removed the session lines from {}",
            rc.display()
        )))
    }

    /// A shell has no context to inject a notice into: the text is
    /// returned as it is, and the trigger never prints it.
    fn envelope(&self, text: &str) -> String {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RC: &str = "/home/u/.bashrc";

    fn rc() -> &'static Path {
        Path::new(RC)
    }

    /// A marked line naming the trigger, or a `_tower_` function, is
    /// wired; an unmarked line naming the trigger is hand-written; and a
    /// line fufu marked — `ff trigger shell` under fufu's marker — is
    /// neither, because it is fufu's.
    #[test]
    fn wiring_is_the_marker_and_the_command() {
        for shell in SHELLS {
            let text = marked(shell, "\n");
            assert!(
                matches!(wiring(&text, rc()), Wiring::Wired { .. }),
                "{shell}: {text}"
            );
            for line in text.lines() {
                assert!(is_marked(line), "{shell}: every line is marked: {line}");
            }
        }
        assert!(matches!(
            wiring(
                &format!("_tower_ambient() {{ {TRIGGER} }}  {MARKER}\n"),
                rc()
            ),
            Wiring::Wired { .. }
        ));
        assert_eq!(
            wiring(
                &format!("PROMPT_COMMAND=\"{TRIGGER};$PROMPT_COMMAND\"\n"),
                rc()
            ),
            Wiring::HandWritten { at: RC.into() }
        );
        let fufu = "[[ $PROMPT_COMMAND == *\"ff trigger shell\"* ]] || PROMPT_COMMAND=\"ff trigger shell;$PROMPT_COMMAND\"  # fufu — added by `ff hook`\n";
        assert_eq!(wiring(fufu, rc()), Wiring::NotWired);
        assert_eq!(wiring("# mine\n", rc()), Wiring::NotWired);
        // Marked wins over hand-written when both are there.
        let both = format!("{TRIGGER}\n{}", marked("bash", "\n"));
        assert!(matches!(wiring(&both, rc()), Wiring::Wired { .. }));
    }

    /// The PowerShell profile is one file: PowerShell 7's when it exists
    /// or when neither does, 5.1's only when it is the sole profile on
    /// disk, and under the Documents folder PowerShell resolves rather
    /// than the one under home.
    #[test]
    fn the_powershell_profile_is_one_file() {
        let home = Path::new("C:\\Users\\u");
        let docs = home.join("Documents");
        let seven = docs.join("PowerShell").join(PROFILE);
        let five = docs.join("WindowsPowerShell").join(PROFILE);
        let profile = |exists: &dyn Fn(&Path) -> bool| {
            powershell_profile(true, home, Some(&docs), None, exists)
        };
        assert_eq!(profile(&|p| p == seven), seven, "7 exists");
        assert_eq!(profile(&|p| p == five), five, "only 5.1 exists");
        assert_eq!(profile(&|p| p == seven || p == five), seven, "both");
        assert_eq!(profile(&|_| false), seven, "neither: 7 is created");

        let redirected = home.join("OneDrive").join("Documents");
        assert_eq!(
            powershell_profile(true, home, Some(&redirected), None, |_| false),
            redirected.join("PowerShell").join(PROFILE),
            "the known folder wins over <home>\\Documents"
        );
        assert_eq!(
            powershell_profile(true, home, None, None, |_| false),
            seven,
            "a failed lookup falls back to <home>\\Documents"
        );

        let home = Path::new("/home/u");
        assert_eq!(
            powershell_profile(false, home, None, None, |_| false),
            Path::new("/home/u/.config/powershell").join(PROFILE)
        );
        assert_eq!(
            powershell_profile(false, home, None, Some(Path::new("/xdg")), |_| false),
            Path::new("/xdg/powershell").join(PROFILE),
            "XDG_CONFIG_HOME is honored, and Documents is not consulted"
        );
    }

    /// A CRLF file stays CRLF through the rewrites: the lines are joined
    /// with the ending the file already uses.
    #[test]
    fn a_crlf_file_keeps_its_line_endings() {
        let file = format!("# mine\r\n{}", marked("bash", "\r\n"));
        assert!(!file.contains("\n\n"), "{file:?}");
        assert!(file.ends_with("\r\n"));
        let eol = line_ending(&file);
        assert_eq!(eol, "\r\n");
        let kept: Vec<&str> = file.lines().filter(|l| !is_marked(l)).collect();
        assert_eq!(format!("{}{eol}", kept.join(eol)), "# mine\r\n");
        assert_eq!(line_ending("a\nb\n"), "\n");
    }

    /// The login shell by its basename, `pwsh` under its slug, and an
    /// exotic shell as none.
    #[test]
    fn the_default_shell_is_the_login_shell_by_slug() {
        let var = "SHELL";
        let saved = std::env::var_os(var);
        for (value, expected) in [
            ("/bin/bash", Some("bash")),
            ("/usr/bin/zsh", Some("zsh")),
            ("/opt/homebrew/bin/fish", Some("fish")),
            ("/usr/bin/pwsh", Some("powershell")),
            ("/bin/tcsh", None),
        ] {
            unsafe { std::env::set_var(var, value) };
            assert_eq!(default_shell(), expected, "{value}");
        }
        match saved {
            Some(value) => unsafe { std::env::set_var(var, value) },
            None => unsafe { std::env::remove_var(var) },
        }
    }

    /// The written lines: the three rules every shell obeys are visible
    /// in the text — the mint guarded on an inherited session, the
    /// release guarded on the pid, the trigger fed from /dev/null.
    #[test]
    fn every_shell_mints_once_and_releases_only_its_own() {
        for shell in SHELLS {
            let text = lines(shell).join("\n");
            assert!(text.contains("atc session --mint"), "{shell} mints: {text}");
            assert!(
                text.contains("ATC_SHELL_SESSION") && text.contains("ATC_SHELL_PID"),
                "{shell} exports both: {text}"
            );
            assert!(
                text.contains(&format!("{TRIGGER} --end")),
                "{shell} releases: {text}"
            );
        }
        for shell in ["bash", "zsh", "fish"] {
            let text = lines(shell).join("\n");
            assert!(
                text.contains(&format!("{TRIGGER} </dev/null")),
                "{shell}: the trigger reads no stdin: {text}"
            );
        }
        assert!(lines("bash")[0].contains("[ \"$ATC_SHELL_PID\" = \"$$\" ] && trap"));
        assert!(lines("zsh")[3].contains("[[ $ATC_SHELL_PID == $$ ]] &&"));
        assert!(lines("fish")[2].contains("test \"$ATC_SHELL_PID\" = \"$fish_pid\"; and"));
        assert!(lines("powershell")[0].contains("GetCommandLineArgs()"));
    }
}
