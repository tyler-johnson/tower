//! Exercise the Unix installer offline, including replacing a live executable and refreshing through the new path.
#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use sha2::{Digest, Sha256};

struct Fixture {
    root: tempfile::TempDir,
    install: PathBuf,
    payload: Vec<u8>,
}

fn executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Fixture {
    fn new(bad_checksum: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("install directory");
        let release = root.path().join("release");
        let bin = root.path().join("bin");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::create_dir_all(&release).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let payload = r#"#!/bin/sh
printf '%s\n' "$0" "$*" > "$HOOK_RECORD"
if read -r line; then
    printf 'stdin was not closed\n' >> "$HOOK_RECORD"
fi
exit "${HOOK_EXIT:-0}"
"#;
        executable(&release.join("atc"), payload);
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            os => os,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            arch => arch,
        };
        let archive = format!("atc_0.2.0_{os}_{arch}.tar.gz");
        let output = Command::new("tar")
            .args(["-czf", &archive, "atc"])
            .current_dir(&release)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let hash = if bad_checksum {
            "0".repeat(64)
        } else {
            format!(
                "{:x}",
                Sha256::digest(std::fs::read(release.join(&archive)).unwrap())
            )
        };
        std::fs::write(
            release.join("checksums.txt"),
            format!("{hash}  {archive}\n"),
        )
        .unwrap();
        executable(
            &bin.join("curl"),
            r#"#!/bin/sh
test "$1" = '-fsSL' && test "$2" = '-o' || exit 1
cp "$RELEASE_FIXTURE/${4##*/}" "$3"
"#,
        );
        Self {
            root,
            install,
            payload: payload.as_bytes().to_vec(),
        }
    }

    fn run(&self, hook_exit: i32) -> Output {
        let mut command = Command::new("/bin/sh");
        atc_testsupport::scrub(&mut command);
        command
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh"))
            .current_dir(self.root.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.path().join("bin").display()),
            )
            .env("HOME", self.root.path())
            .env("TMPDIR", self.root.path())
            .env("ATC_VERSION", "v0.2.0")
            .env("ATC_INSTALL_DIR", &self.install)
            .env("RELEASE_FIXTURE", self.root.path().join("release"))
            .env("HOOK_RECORD", self.root.path().join("hook-record"))
            .env("HOOK_EXIT", hook_exit.to_string())
            .output()
            .unwrap()
    }

    fn assert_installed(&self, output: &Output) {
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            std::fs::read(self.install.join("atc")).unwrap(),
            self.payload
        );
        assert!(!self.install.join("atc.new").exists());
        assert_eq!(
            std::fs::read_to_string(self.root.path().join("hook-record")).unwrap(),
            format!("{}\nhook -u\n", self.install.join("atc").display())
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("atc hook"), "{stdout}");
        assert!(!stdout.contains("fufu"), "{stdout}");
    }
}

#[test]
fn fresh_install_refreshes_through_its_path_with_stdin_closed() {
    let fixture = Fixture::new(false);
    fixture.assert_installed(&fixture.run(0));
}

#[test]
fn a_busy_binary_is_replaced_and_hook_failure_does_not_fail_installation() {
    let fixture = Fixture::new(false);
    let target = fixture.install.join("atc");
    std::fs::copy("/bin/sh", &target).unwrap();
    let mut busy = Command::new(&target)
        .args(["-c", "printf 'ready\n'; read -r line"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ready = String::new();
    BufReader::new(busy.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    assert_eq!(ready, "ready\n");
    let output = fixture.run(17);
    // Closing stdin lets our fixture shell exit normally, even if the installer failed.
    drop(busy.stdin.take());
    busy.wait().unwrap();
    fixture.assert_installed(&output);
}

#[test]
fn a_bad_checksum_preserves_the_old_binary_and_never_refreshes() {
    let fixture = Fixture::new(true);
    let target = fixture.install.join("atc");
    std::fs::write(&target, "original binary").unwrap();
    let output = fixture.run(0);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("checksum mismatch"));
    assert_eq!(std::fs::read_to_string(target).unwrap(), "original binary");
    assert!(!fixture.root.path().join("hook-record").exists());
    assert!(!fixture.install.join("atc.new").exists());
}
