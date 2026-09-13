//! The declaring seam against real executable probes, isolated from the user's registry and clients.
#![cfg(unix)]

use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Fixture {
    home: tempfile::TempDir,
    repo: atc_testsupport::Repo,
    bin: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        Self {
            home,
            repo: atc_testsupport::Repo::new(),
            bin,
        }
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_atc"));
        atc_testsupport::scrub(&mut cmd);
        let mut paths = vec![self.bin.clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        cmd.args(args)
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("XDG_CONFIG_HOME", self.home.path().join("xdg"))
            .env("XDG_CACHE_HOME", self.home.path().join("cache"))
            .env("APPDATA", self.home.path().join("xdg"))
            .env("LOCALAPPDATA", self.home.path().join("cache"))
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("ATC_FF", "/nonexistent")
            .env("ATC_CODEX", "/nonexistent")
            .env("ATC_COPILOT", "/nonexistent")
            .env("ATC_CURSOR", "/nonexistent")
            .env("ATC_OPENCODE", "/nonexistent")
            .env("ATC_NONINTERACTIVE", "1")
            .env("PROBE_LOG", self.home.path().join("calls"));
        cmd
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?}: {}\n{}",
            text(&out),
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
    fn script(&self, name: &str, body: &str) {
        let path = self.bin.join(format!("atc-{name}"));
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn probe(&self, manifest: Value, files: Value) {
        let manifest = json!({"atc": 1, "cmd": "probe --atc-manifest", "data": manifest});
        let skill = json!({"atc": 1, "cmd": "probe --atc-skill", "data": {"files": files}});
        self.script(
            "probe",
            &format!(
                r#"
printf '%s\n' "$*" >> "$PROBE_LOG"
case "$1" in
 --atc-manifest) test "$#" = 1 && test "$ATC_NONINTERACTIVE" = 1 && printf '%s\n' '{manifest}' ;;
 --atc-skill) test "$#" = 2 && printf '%s\n' '{skill}' ;;
 help) printf 'probe help\n' ;;
 explain) printf 'probe explain %s\n' "$2" ;;
 briefing) printf 'probe dynamic notice\n' ;;
 *) printf 'dispatch'; printf '<%s>' "$@"; printf '\n'; exit 7 ;;
esac
"#
            ),
        );
    }
    fn declare(&self) {
        self.ok(&["adapter", "probe"]);
    }
}
fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn data(out: &Output) -> Value {
    serde_json::from_slice::<Value>(&out.stdout).unwrap()["data"].clone()
}
fn manifest() -> Value {
    json!({"name":"probe", "version":"1.0.0", "contract":1, "verbs":[{"name":"go", "read_only":true}], "undoable":false})
}
fn files(body: &str) -> Value {
    json!([{"path":"SKILL.md", "content":body}])
}

#[test]
fn declaration_round_trip_and_help_explain_delegation() {
    let f = Fixture::new();
    assert_eq!(data(&f.ok(&["adapter", "--json"]))["declared"], json!([]));
    f.probe(manifest(), files("manual"));
    assert_eq!(
        f.run(&["probe", "go"]).status.code(),
        Some(7),
        "dispatch needs no declaration"
    );
    assert!(!f.run(&["help", "probe"]).status.success());
    f.declare();
    let list = data(&f.ok(&["adapter", "--json"]));
    assert_eq!(list["declared"][0]["manifest"]["name"], "probe");
    assert_eq!(
        list["declared"][0]["resolved"],
        json!(f.bin.join("atc-probe"))
    );
    assert_eq!(text(&f.ok(&["help", "probe"])), "probe help\n");
    assert_eq!(
        text(&f.ok(&["explain", "probe/usage/x"])),
        "probe explain usage/x\n"
    );
    let mut next = manifest();
    next["version"] = json!("2.0.0");
    f.probe(next, files("manual"));
    let updated = data(&f.ok(&["adapter", "probe", "--json"]));
    assert_eq!(updated["replaced"], "1.0.0");
    f.ok(&["adapter", "-d", "probe"]);
    assert_eq!(data(&f.ok(&["adapter", "--json"]))["declared"], json!([]));
    assert_eq!(f.run(&["probe"]).status.code(), Some(7));
    let out = f.run(&["adapter", "-d", "probe", "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"]["id"],
        "adapter/not-declared"
    );
}

#[test]
fn dispatch_preserves_os_arguments_streams_exit_and_context() {
    use std::io::Write;
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    f.script("probe", r#"printf '%s\n' "$ATC_CONTRACT|$ATC_REPO|$ATC_SESSION"; printf '%s\n' "$@"; cat; printf 'child stderr\n' >&2; exit 23"#);
    let mut cmd = f.command(&["--json", "probe", "--json", "--help", "a b", "", "--"]);
    cmd.arg(std::ffi::OsString::from_vec(vec![0xff, b'x']))
        .env("ATC_REPO", "/stale")
        .env("ATC_SESSION", "worker");
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"input\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(23));
    let mut expected = format!(
        "1|{}|worker\n--json\n--help\na b\n\n--\n",
        f.repo.path().canonicalize().unwrap().display()
    )
    .into_bytes();
    expected.extend_from_slice(b"\xffx\ninput\n");
    assert_eq!(out.stdout, expected);
    assert_eq!(out.stderr, b"child stderr\n");
    let out = f
        .command(&["probe"])
        .current_dir(f.home.path())
        .env("ATC_REPO", "/stale")
        .output()
        .unwrap();
    assert!(text(&out).starts_with("1||"), "{}", text(&out));
}

#[test]
fn builtin_aliases_and_invalid_tower_flags_cannot_launch_a_probe() {
    let f = Fixture::new();
    for name in ["version", "show", "file", "probe"] {
        f.script(name, "printf 'wrong binary'; exit 99");
    }
    for args in [
        &["version"][..],
        &["show", "--help"],
        &["file", "--bad"],
        &["--bad", "probe"],
        &["-v", "probe"],
        &["--closed", "7d", "probe"],
    ] {
        let out = f.run(args);
        assert_ne!(out.status.code(), Some(99), "{args:?}");
        assert!(!text(&out).contains("wrong binary"));
    }
}

#[test]
fn handshake_refusals_do_not_replace_the_record() {
    let f = Fixture::new();
    f.probe(manifest(), files("manual"));
    f.declare();
    let file = data(&f.ok(&["adapter", "--json"]))["file"]
        .as_str()
        .unwrap()
        .to_string();
    let before = std::fs::read(&file).unwrap();
    for (field, value, id) in [
        ("contract", json!(99), "adapter/unsupported-contract"),
        ("name", json!("other"), "adapter/name-mismatch"),
        ("verbs", json!([]), "adapter/bad-manifest"),
    ] {
        let mut bad = manifest();
        bad[field] = value;
        f.probe(bad, files("manual"));
        let out = f.run(&["adapter", "probe", "--json"]);
        let err = serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"].clone();
        assert_eq!(err["id"], id);
        if field == "contract" {
            assert_eq!(
                err["message"],
                "atc-probe speaks contract 99, and this tower speaks 1"
            );
        }
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
    for body in [
        "exit 3",
        "printf 'banner\\n{}\\n'",
        "printf '{}'",
        "printf '{\"atc\":1,\"error\":{\"id\":\"probe/x\"}}'",
    ] {
        f.script("probe", body);
        let out = f.run(&["adapter", "probe", "--json"]);
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"]["id"],
            "adapter/handshake-failed"
        );
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
}

#[test]
fn notice_lines_and_refresh_without_wired_clients() {
    let f = Fixture::new();
    let baseline = text(&f.ok(&["trigger"]));
    let mut m = manifest();
    m["briefing"] = json!("probe static notice");
    f.probe(m, files("manual"));
    f.declare();
    assert!(text(&f.ok(&["trigger"])).ends_with("probe static notice\n"));
    let mut m = manifest();
    m["briefing"] = json!(true);
    m["version"] = json!("2.0.0");
    f.probe(m, files("manual"));
    f.ok(&["hook", "-u", "--json"]);
    assert_eq!(
        data(&f.ok(&["adapter", "--json"]))["declared"][0]["manifest"]["version"],
        "2.0.0"
    );
    assert!(text(&f.ok(&["briefing"])).ends_with("probe dynamic notice\n"));
    for line in ["x".repeat(241), "one\ntwo".into(), "".into()] {
        let mut m = manifest();
        m["briefing"] = json!(line);
        f.probe(m, files("manual"));
        f.declare();
        assert_eq!(text(&f.ok(&["trigger"])), baseline);
    }
    let mut m = manifest();
    m["briefing"] = json!("é".repeat(240));
    f.probe(m, files("manual"));
    f.declare();
    assert!(text(&f.ok(&["trigger"])).contains(&"é".repeat(240)));
}

#[test]
fn failed_and_hung_dynamic_queries_are_bounded_and_help_is_loud() {
    let f = Fixture::new();
    let mut m = manifest();
    m["briefing"] = json!(true);
    f.probe(m, files("manual"));
    f.declare();
    for body in [
        "printf 'bad line'; exit 1",
        "exec sleep 3",
        "sleep 3 & exit 0",
    ] {
        f.script("probe", body);
        let start = std::time::Instant::now();
        let out = f.ok(&["trigger"]);
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
        assert!(!text(&out).contains("bad line"));
    }
    std::fs::remove_file(f.bin.join("atc-probe")).unwrap();
    let out = f.run(&["--json", "help", "probe"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"]["id"],
        "adapter/delegate-failed"
    );
}

#[test]
fn skills_install_refresh_and_remove_across_client_roots() {
    let f = Fixture::new();
    let mut m = manifest();
    m["skills"] = json!(["probe"]);
    f.probe(
        m.clone(),
        json!([{"path":"SKILL.md","content":"old manual"},{"path":"docs/old.md","content":"old"}]),
    );
    f.declare();
    f.ok(&[
        "hook", "claude", "codex", "cursor", "copilot", "opencode", "--json",
    ]);
    let roots = skill_roots(f.home.path());
    assert_eq!(roots.len(), 5, "{roots:?}");
    for root in &roots {
        assert_eq!(
            std::fs::read_to_string(root.join("probe/SKILL.md")).unwrap(),
            "old manual"
        );
    }
    // A rebuilt adapter may retain its version while dropping a file. Refresh still writes the exact new bundle.
    f.probe(m.clone(), files("old manual"));
    f.ok(&["hook", "-u", "--json"]);
    for root in &roots {
        assert!(!root.join("probe/docs/old.md").exists());
    }
    m["version"] = json!("2.0.0");
    f.probe(m, files("new manual"));
    f.ok(&["hook", "-u", "--json"]);
    for root in &roots {
        assert_eq!(
            std::fs::read_to_string(root.join("probe/SKILL.md")).unwrap(),
            "new manual"
        );
        assert!(!root.join("probe/docs/old.md").exists());
    }
    f.ok(&["adapter", "-d", "probe"]);
    f.ok(&["hook", "-u"]);
    for root in &roots {
        assert!(!root.join("probe").exists());
        assert!(root.join("tower/SKILL.md").exists());
    }
}

fn skill_roots(home: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, roots: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, roots);
            } else if entry.file_name() == ".atc-adapter-skills.json" {
                roots.push(dir.to_path_buf());
            }
        }
    }
    let mut roots = Vec::new();
    walk(home, &mut roots);
    roots
}

#[test]
fn doctor_detects_manifest_and_path_drift() {
    let f = Fixture::new();
    f.probe(manifest(), files("manual"));
    f.declare();
    let out = f.run(&["doctor", "--json"]);
    let rows = data(&out)["rows"].as_array().unwrap().clone();
    assert!(
        rows.iter()
            .any(|row| row["check"] == "adapter/probe" && row["level"] == "ok"),
        "{rows:?}"
    );
    let mut m = manifest();
    m["version"] = json!("2.0.0");
    f.probe(m, files("manual"));
    let out = f.run(&["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        data(&out)["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["check"] == "adapter/probe"
                && row["message"].as_str().unwrap().contains("differs"))
    );
    std::fs::remove_file(f.bin.join("atc-probe")).unwrap();
    assert!(
        data(&f.run(&["doctor", "--json"]))["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["message"].as_str().unwrap().contains("not on PATH"))
    );
}

#[test]
fn bad_skills_preserve_installed_files_and_unhook_preserves_foreign_skills() {
    let f = Fixture::new();
    let mut m = manifest();
    m["skills"] = json!(["probe"]);
    f.probe(m.clone(), files("good manual"));
    f.declare();
    f.ok(&["hook", "opencode"]);
    let root = skill_roots(f.home.path()).pop().unwrap();
    let foreign = root.join("my-skill");
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("SKILL.md"), "mine").unwrap();
    f.probe(m, json!([{"path":"../escape","content":"bad"}]));
    let out = f.ok(&["hook", "-u", "--json"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a skill tower can read"));
    assert_eq!(
        std::fs::read_to_string(root.join("probe/SKILL.md")).unwrap(),
        "good manual"
    );
    assert!(!root.join("escape").exists());
    f.ok(&["adapter", "-d", "probe"]);
    f.ok(&["unhook", "opencode"]);
    assert!(!root.join("probe").exists());
    assert_eq!(
        std::fs::read_to_string(foreign.join("SKILL.md")).unwrap(),
        "mine"
    );
}

#[test]
fn install_does_not_overwrite_a_foreign_skill_of_the_same_name() {
    let f = Fixture::new();
    // First install reveals this client's actual skill root without relying on a platform spelling.
    let mut m = manifest();
    m["skills"] = json!(["probe"]);
    f.probe(m, files("adapter manual"));
    f.declare();
    f.ok(&["hook", "opencode"]);
    let root = skill_roots(f.home.path()).pop().unwrap();
    std::fs::remove_file(root.join(".atc-adapter-skills.json")).unwrap();
    std::fs::write(root.join("probe/SKILL.md"), "my own manual").unwrap();
    let out = f.ok(&["hook", "-u"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("was not installed by tower"));
    assert_eq!(
        std::fs::read_to_string(root.join("probe/SKILL.md")).unwrap(),
        "my own manual"
    );
}

#[test]
fn update_walk_continues_past_source_tower_and_failed_adapter_then_refreshes() {
    let f = Fixture::new();
    let mut m = manifest();
    m["update"] = json!({"install":"https://example.invalid/probe.sh", "bin":f.bin});
    let mut broken = m.clone();
    broken["name"] = json!("broken");
    broken["update"]["install"] = json!("https://example.invalid/broken.sh");
    f.script(
        "broken",
        &format!("printf '%s\\n' '{}'", json!({"atc":1,"data":broken})),
    );
    f.ok(&["adapter", "broken"]);
    f.probe(m, files("manual"));
    f.declare();
    let curl = f.bin.join("curl");
    std::fs::write(&curl, "#!/bin/sh\ncase \"$2\" in *broken*) printf 'exit 9\\n';; *) printf 'printf installed >> \"$INSTALL_LOG\"\\n';; esac\n").unwrap();
    std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let marker = f.home.path().join("installed");
    let preview = data(&f.ok(&["update", "--json"]));
    assert_eq!(preview["adapters"].as_array().unwrap().len(), 2);
    assert!(
        !marker.exists(),
        "noninteractive update only prints recipes"
    );
    let before = std::fs::read_to_string(f.home.path().join("calls"))
        .unwrap()
        .matches("--atc-manifest")
        .count();
    let out = f
        .command(&["update", "-y", "--json"])
        .env("INSTALL_LOG", &marker)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = serde_json::from_slice::<Value>(&out.stdout).unwrap()["error"].clone();
    assert!(err["message"].as_str().unwrap().contains("atc-broken"));
    assert!(err["message"].as_str().unwrap().contains("source"));
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "installed");
    assert!(
        std::fs::read_to_string(f.home.path().join("calls"))
            .unwrap()
            .matches("--atc-manifest")
            .count()
            > before
    );
}
