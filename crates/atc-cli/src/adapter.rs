//! PATH dispatch and bounded questions, carried from fufu at a44093cc.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::CliError;

pub const BUDGET: Duration = Duration::from_secs(1);

pub fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_')
}

pub fn resolve(name: &str) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path).filter(|dir| !dir.as_os_str().is_empty()) {
        #[cfg(unix)]
        let names = [format!("atc-{name}")];
        #[cfg(not(unix))]
        let names = [format!("atc-{name}.exe"), format!("atc-{name}")];
        for name in names {
            let path = dir.join(name);
            if executable(&path) {
                // A relative PATH entry must still name the same binary when an ask changes cwd.
                return std::path::absolute(path).ok();
            }
        }
    }
    None
}

fn executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// Every child gets the same context. An absent repository clears a stale inherited value.
pub fn context(cmd: &mut Command, repo: Option<&Path>, session: Option<&str>) {
    cmd.env("ATC_CONTRACT", atc_core::machine::CONTRACT.to_string());
    if let Some(repo) = repo {
        let real = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
        let real = real.to_string_lossy();
        let real = real.strip_prefix(r"\\?\").unwrap_or(&real);
        cmd.env("ATC_REPO", real.replace('\\', "/"));
    } else {
        cmd.env_remove("ATC_REPO");
    }
    if let Some(session) = session {
        cmd.env("ATC_SESSION", session);
    }
}

pub fn dispatch(path: &Path, argv: &[OsString]) -> ! {
    let cwd = std::env::current_dir().ok();
    let repo = cwd.as_deref().and_then(atc_core::lease::repo_root);
    let session = atc_core::lease::session_key("");
    let mut cmd = Command::new(path);
    cmd.args(argv)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    context(&mut cmd, repo.as_deref(), session.as_deref());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        eprintln!("atc: {}: {err}", path.display());
        std::process::exit(if err.kind() == std::io::ErrorKind::NotFound {
            127
        } else {
            126
        });
    }
    #[cfg(not(unix))]
    match cmd.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(err) => {
            eprintln!("atc: {}: {err}", path.display());
            std::process::exit(126);
        }
    }
}

/// Drain stdout independently of waiting for the child; a grandchild holding the pipe cannot extend the deadline.
pub fn time_boxed(cmd: &mut Command, budget: Duration) -> Result<Vec<u8>, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|err| format!("it would not run: {err}"))?;
    let mut pipe = child.stdout.take().expect("piped stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut said = Vec::new();
        let _ = pipe.read_to_end(&mut said);
        let _ = tx.send(said);
    });
    let deadline = Instant::now() + budget;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!("it exited with {status}"));
                }
                break;
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("it did not answer inside the time box".into());
            }
        }
    }
    rx.recv_timeout(
        deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(20)),
    )
    .map_err(|_| "it exited without closing its stdout".into())
}

pub fn ask(
    name: &str,
    verb: &str,
    rest: &[&str],
    cwd: &Path,
    repo: Option<&Path>,
    session: Option<&str>,
) -> Option<Vec<u8>> {
    let path = resolve(name)?;
    let mut cmd = Command::new(path);
    cmd.arg(verb).args(rest).current_dir(cwd);
    context(&mut cmd, repo, session);
    match time_boxed(&mut cmd, BUDGET) {
        Ok(said) => Some(said),
        Err(why) => {
            if std::env::var_os("ATC_DEBUG").is_some() {
                eprintln!("atc[debug]: atc-{name} {verb} said nothing: {why}");
            }
            None
        }
    }
}

pub fn delegate(name: &str, verb: &str, rest: &[&str]) -> Result<Vec<u8>, CliError> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let session = atc_core::lease::session_key("");
    ask(name, verb, rest, &cwd, None, session.as_deref()).ok_or_else(|| CliError::coded(
        "adapter/delegate-failed",
        format!("atc-{name} did not answer: it may have left PATH since it was declared, refused to start, exited nonzero, or run past the time box tower gives it"),
        vec!["atc doctor".into(), "atc adapter".into()],
    ))
}
