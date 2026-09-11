//! Builds the web app and stages it for embedding.
//!
//! `cargo build` alone yields the full binary: this script runs pnpm — install, then the SvelteKit build — and copies the output into `$OUT_DIR/webapp`, where `assets.rs` embeds it. Node and pnpm are build dependencies of the workspace; the binary needs neither at runtime. The copy exists so the embed macro never reads the mutable source tree: `web/build` belongs to vite, `OUT_DIR` belongs to cargo.
//!
//! The rerun stamps cover the web sources alone — never `web/build`, which this script writes (stamping it would loop the rebuild), and never `node_modules` or `.svelte-kit`. pnpm runs once per profile's OUT_DIR — debug, dogfood, and release each — and `pnpm install` is a fast no-op when the store is current. A stale lockfile fails under `--frozen-lockfile` with pnpm's own readable error; this script never mutates the lockfile.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let web = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../../web");

    for source in [
        "src",
        "package.json",
        "pnpm-lock.yaml",
        "pnpm-workspace.yaml",
        "svelte.config.js",
        "vite.config.ts",
        "tsconfig.json",
    ] {
        println!("cargo::rerun-if-changed={}", web.join(source).display());
    }
    // SvelteKit's optional static/ directory: stamped only while it
    // exists, because a stamp on a missing path reruns every build.
    let statics = web.join("static");
    if statics.exists() {
        println!("cargo::rerun-if-changed={}", statics.display());
    }

    pnpm(&web, &["install", "--frozen-lockfile"]);
    pnpm(&web, &["run", "build"]);

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("webapp");
    if out.exists() {
        fs::remove_dir_all(&out).expect("remove the previous webapp copy");
    }
    copy_dir(&web.join("build"), &out);
}

/// One pnpm command in `web/`, or the build fails. On Windows pnpm is
/// `pnpm.cmd`, which `Command::new("pnpm")` will not resolve, so it
/// goes through `cmd /C`.
fn pnpm(web: &Path, args: &[&str]) {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", "pnpm"]);
        command
    } else {
        Command::new("pnpm")
    };
    match command.args(args).current_dir(web).status() {
        Ok(status) if status.success() => {}
        Ok(status) => panic!(
            "`pnpm {}` failed ({status}) in {}: the web build runs inside cargo, and Node and pnpm are build dependencies of this workspace",
            args.join(" "),
            web.display()
        ),
        Err(error) => panic!(
            "could not run pnpm ({error}): building tower needs Node and pnpm on PATH — build dependencies only, the binary serves the app with neither"
        ),
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the webapp copy");
    for entry in fs::read_dir(from).expect("read the web build") {
        let entry = entry.expect("read a web build entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a web build file");
        }
    }
}
