//! The rc lines under a real shell: a fixture `HOME` wired by `atc hook
//! <shell>` and a `PATH` directory holding the test binary as `atc`,
//! then an interactive shell fed a script on stdin. What the lines
//! promise: one session per interactive shell, inherited by its
//! children and minted again by none of them; the lease created at the
//! first prompt, listed under `this session`, and gone at exit — on
//! `exit`, on EOF, and after `exec` — released by the shell that owns
//! it and never by a nested one; a scripted shell minting nothing; and
//! a piped script running to its end, because the trigger reads no
//! stdin.
//!
//! Unix only, and each test skips when its shell is not installed:
//! bash is on every runner, zsh and fish where they are.
#![cfg(unix)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use atc_testsupport::{Repo, scrub};

fn root(repo: &Path) -> &Path {
    repo.parent().expect("the fixture nests the repository")
}

/// Whether the shell is on this machine's PATH.
fn installed(shell: &str) -> bool {
    Command::new(shell)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// A fixture: the repository, its root as `HOME`, the binary linked as
/// `atc` under `HOME/bin`, and the shell's rc file wired.
struct Fixture {
    repo: Repo,
    shell: &'static str,
}

impl Fixture {
    fn new(shell: &'static str) -> Fixture {
        let repo = Repo::new();
        repo.pin_writer("pi");
        let fixture = Fixture { repo, shell };
        let bin = fixture.home().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_atc"), bin.join("atc")).unwrap();
        let out = fixture.atc(&["hook", shell]).output().unwrap();
        assert!(
            out.status.success(),
            "hook {shell}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        fixture
    }

    fn home(&self) -> &Path {
        root(self.repo.path())
    }

    fn leases(&self) -> PathBuf {
        self.home().join(".local/state/atc/leases")
    }

    fn lease_files(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.leases()) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            // The heartbeat's sweep marker is not a lease, and neither
            // is a `.lock` sidecar.
            .filter(|name| name != "sweep" && !name.ends_with(".lock"))
            .collect();
        names.sort();
        names
    }

    /// The environment every spawn gets: the fixture's HOME, the
    /// binary first on PATH, the shell's own rc variable, and none of
    /// the developer's session variables.
    fn env(&self, command: &mut Command) {
        let home = self.home();
        let path = format!(
            "{}:{}",
            home.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        command
            .current_dir(self.repo.path())
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env("PATH", path)
            .env("XDG_CONFIG_HOME", home.join("xdg"))
            .env("ZDOTDIR", home)
            .env("TERM", "dumb")
            .env_remove("GIT_CONFIG_GLOBAL")
            .env_remove("PROMPT_COMMAND")
            .env("ATC_FF", "/nonexistent");
        scrub(command);
    }

    fn atc(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
        command.args(args);
        self.env(&mut command);
        command
    }

    /// An interactive shell fed the script on stdin; stdout comes back
    /// whole, stderr — the prompts — is dropped.
    fn interactive(&self, script: &str) -> Output {
        let mut command = Command::new(self.shell);
        command.arg("-i");
        self.env(&mut command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|err| panic!("spawn {} -i: {err}", self.shell));
        child
            .stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    /// A scripted shell — `<shell> -c <script>` — from outside any
    /// interactive one.
    fn scripted(&self, script: &str) -> Output {
        let mut command = Command::new(self.shell);
        command.args(["-c", script]);
        self.env(&mut command);
        command.stdin(Stdio::null()).output().unwrap()
    }
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The JSON envelopes among the lines, in order.
fn envelopes(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap_or_else(|err| panic!("{err}: {line}")))
        .collect()
}

/// The script a full session runs, in each shell's syntax: who am I,
/// what does a child see, who else is here, then exit.
fn session_script(shell: &str) -> String {
    let child = match shell {
        "fish" => "sh -c 'echo child=$ATC_SHELL_SESSION'",
        _ => "bash -c 'echo child=$ATC_SHELL_SESSION'",
    };
    format!("atc whoami --json\n{child}\natc session --json\necho pid=$ATC_SHELL_PID\nexit\n")
}

/// The whole promise under one shell: the session with source `shell`
/// and the shell's own pid alive, the id inherited by a child, the lease
/// listed as this session, and no lease after `exit`.
fn a_terminal_is_a_session(shell: &'static str) {
    if !installed(shell) {
        eprintln!("{shell} is not installed; skipped");
        return;
    }
    let fixture = Fixture::new(shell);
    let out = fixture.interactive(&session_script(shell));
    let text = stdout(&out);
    let envelopes = envelopes(&text);
    assert_eq!(envelopes.len(), 2, "{text}");
    let whoami = &envelopes[0]["data"];
    assert_eq!(whoami["session_source"], "shell", "{shell}: {whoami}");
    let session = whoami["session"].as_str().unwrap().to_string();
    assert_eq!(session.len(), 36, "a v7 id: {session}");
    let pid_line = text
        .lines()
        .find(|line| line.starts_with("pid="))
        .unwrap_or_else(|| panic!("{shell}: {text}"));
    let pid: u64 = pid_line["pid=".len()..].parse().unwrap();
    if cfg!(target_os = "linux") {
        assert_eq!(whoami["pid"], pid, "{shell}: the shell's own pid: {whoami}");
        assert_eq!(whoami["lease"]["pid_alive"], true, "{shell}: {whoami}");
    }
    assert_eq!(whoami["lease"]["fresh"], true, "{shell}: {whoami}");
    assert!(
        text.contains(&format!("child={session}")),
        "{shell}: the child inherits the id: {text}"
    );
    let sessions = envelopes[1]["data"]["sessions"].as_array().unwrap();
    let own: Vec<&serde_json::Value> = sessions.iter().filter(|row| row["this"] == true).collect();
    assert_eq!(own.len(), 1, "{shell}: {sessions:?}");
    assert_eq!(own[0]["session"], session);
    assert_eq!(sessions.len(), 1, "{shell}: one session, the terminal's");
    assert!(
        fixture.lease_files().is_empty(),
        "{shell}: exit releases the lease: {:?}",
        fixture.lease_files()
    );
}

#[test]
fn bash_a_terminal_is_a_session() {
    a_terminal_is_a_session("bash");
}

#[test]
fn zsh_a_terminal_is_a_session() {
    a_terminal_is_a_session("zsh");
}

#[test]
fn fish_a_terminal_is_a_session() {
    a_terminal_is_a_session("fish");
}

/// A nested interactive shell mints nothing — it is in its parent's
/// session — and its exit releases nothing; the parent's exit does.
#[test]
fn a_nested_bash_mints_nothing_and_releases_nothing() {
    if !installed("bash") {
        return;
    }
    let fixture = Fixture::new("bash");
    let script = "atc whoami --json\n\
                  bash -i <<'INNER'\n\
                  echo nested=$ATC_SHELL_SESSION\n\
                  echo nested_pid=$ATC_SHELL_PID\n\
                  trap -p EXIT\n\
                  exit\n\
                  INNER\n\
                  atc session --json\n\
                  echo pid=$$\n\
                  exit\n";
    let text = stdout(&fixture.interactive(script));
    let envelopes = envelopes(&text);
    assert_eq!(envelopes.len(), 2, "{text}");
    let session = envelopes[0]["data"]["session"].as_str().unwrap();
    assert!(text.contains(&format!("nested={session}")), "{text}");
    let pid = text
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .unwrap();
    assert!(text.contains(&format!("nested_pid={pid}")), "{text}");
    assert!(
        !text.contains("trap -- "),
        "the nested shell installs no release trap: {text}"
    );
    let sessions = envelopes[1]["data"]["sessions"].as_array().unwrap();
    assert_eq!(
        sessions.len(),
        1,
        "the nested exit released nothing: {sessions:?}"
    );
    assert_eq!(sessions[0]["session"], session);
    assert!(fixture.lease_files().is_empty(), "the parent's exit did");
}

/// `exec bash` keeps the session — the same pid, the variable inherited
/// — and the re-entered shell still releases on exit.
#[test]
fn exec_bash_keeps_the_session_and_still_releases() {
    if !installed("bash") {
        return;
    }
    let fixture = Fixture::new("bash");
    let script = "echo before=$ATC_SHELL_SESSION\n\
                  exec bash -i\n\
                  echo after=$ATC_SHELL_SESSION\n\
                  echo pid=$$ own=$ATC_SHELL_PID\n\
                  trap -p EXIT\n\
                  atc session --json\n\
                  exit\n";
    let text = stdout(&fixture.interactive(script));
    let before = text
        .lines()
        .find_map(|line| line.strip_prefix("before="))
        .unwrap();
    let after = text
        .lines()
        .find_map(|line| line.strip_prefix("after="))
        .unwrap();
    assert_eq!(before, after, "{text}");
    assert_eq!(before.len(), 36);
    let pids = text
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .unwrap();
    let (pid, own) = pids.split_once(" own=").unwrap();
    assert_eq!(pid, own, "exec keeps the pid: {text}");
    assert!(
        text.contains("trap -- 'atc trigger shell --end"),
        "the re-entered shell owns the session and gets the trap: {text}"
    );
    assert_eq!(envelopes(&text).len(), 1);
    assert!(fixture.lease_files().is_empty(), "released at exit");
}

/// EOF on stdin ends the shell the way a closed terminal does, and the
/// lease goes with it.
#[test]
fn eof_releases_the_lease() {
    if !installed("bash") {
        return;
    }
    let fixture = Fixture::new("bash");
    let text = stdout(&fixture.interactive("atc session --json\n"));
    let sessions = envelopes(&text)[0]["data"]["sessions"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(sessions, 1, "{text}");
    assert!(fixture.lease_files().is_empty());
}

/// A scripted bash outside any terminal has no session: `-c` is not
/// interactive, so the lines mint nothing, and sourcing the rc file by
/// hand from one mints nothing either.
#[test]
fn a_scripted_bash_mints_nothing() {
    if !installed("bash") {
        return;
    }
    let fixture = Fixture::new("bash");
    let text = stdout(&fixture.scripted("echo session=[$ATC_SHELL_SESSION]"));
    assert_eq!(text, "session=[]\n");
    let text = stdout(
        &fixture
            .scripted("source ~/.bashrc; echo session=[$ATC_SHELL_SESSION] pid=[$ATC_SHELL_PID]"),
    );
    assert_eq!(text, "session=[] pid=[]\n");
    assert!(fixture.lease_files().is_empty());
}

/// A script piped into an interactive bash runs to its end: the trigger
/// runs before every prompt with stdin from /dev/null, so it eats none
/// of the script.
#[test]
fn a_piped_script_runs_to_its_end() {
    if !installed("bash") {
        return;
    }
    let fixture = Fixture::new("bash");
    let script: String = (1..=20)
        .map(|n| format!("echo line{n}\n"))
        .chain(std::iter::once("echo the-end\nexit\n".to_string()))
        .collect();
    let text = stdout(&fixture.interactive(&script));
    let expected: String = (1..=20)
        .map(|n| format!("line{n}\n"))
        .chain(std::iter::once("the-end\n".to_string()))
        .collect();
    assert_eq!(text, expected);
}
