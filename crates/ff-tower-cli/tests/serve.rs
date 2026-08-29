//! `ff tower serve` against the real binary: the two lanes and their four
//! sources each, the socket, the embedded app, and the fallback.
//!
//! What this file deliberately does not prove is anything about the
//! board: every assertion here is about the verb, the socket, and the
//! envelope, so routes landing underneath change nothing above. What the
//! read API answers is serve_api.rs's to prove. The compiled default port
//! is not bound either: 7420 is where a person dogfooding tower already
//! has a server, and a suite that fought them for it would fail for the
//! wrong reason. That the default is 7420 is core's own test.
//!
//! The shape no other suite in this crate has is a child that does not
//! exit. [`Server`] spawns one with `--json`, reads the startup envelope
//! for the address it actually bound, and kills it on drop. Every spawn
//! points `HOME` and `XDG_CONFIG_HOME` into the fixture and clears
//! `TOWER_HOST` and `TOWER_PORT`, so neither the developer's git config
//! nor their environment can decide where a test binds.
//!
//! The only non-loopback address any test names is `0.0.0.0`: it binds on
//! every platform CI runs, and a wildcard bind is reachable through the
//! loopback, which is what makes a wide bind testable at all. `127.0.0.2`
//! is local on Linux and not on macOS without an alias, and `::1` is left
//! alone for the same kind of reason.

use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

mod support;
use support::{Server, command, free_port, free_ports, http, refusal};

/// A spawn that runs to completion. Only for the refusals: a `serve` that
/// bound would never return.
fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    command(repo).args(args).output().expect("spawn ff-tower")
}

#[test]
fn a_port_that_does_not_parse_is_the_usage_envelope() {
    let repo = Repo::new();
    let out = ff_tower(repo.path(), &["serve", "--json", "--port", "banana"]);
    let envelope = refusal(&out, 2, "usage/bad-port");
    let message = envelope["error"]["message"].as_str().expect("a message");
    // The lane is named, because the lane is what has to be fixed.
    assert!(message.contains("--port"), "{message}");
    assert!(message.contains("banana"), "{message}");
}

#[test]
fn the_environment_and_the_config_refuse_the_same_way() {
    let repo = Repo::new();

    let out = command(repo.path())
        .args(["serve", "--json"])
        .env("TOWER_PORT", "70000")
        .output()
        .expect("spawn ff-tower");
    let message = refusal(&out, 2, "usage/bad-port")["error"]["message"].to_string();
    assert!(message.contains("TOWER_PORT"), "{message}");

    // The registry refuses an unusable port on the way in, so the only
    // way to have one stored is to have stored it with git.
    let out = ff_tower(repo.path(), &["config", "--json", "servePort", "70000"]);
    refusal(&out, 2, "usage/bad-value");

    repo.git(&["config", "tower.servePort", "banana"]);
    let out = ff_tower(repo.path(), &["serve", "--json"]);
    let message = refusal(&out, 2, "usage/bad-port")["error"]["message"].to_string();
    assert!(message.contains("tower.servePort"), "{message}");
}

#[test]
fn the_flag_beats_the_environment_beats_the_config() {
    let repo = Repo::new();
    let ports = free_ports(3);
    let (flag, env, config) = (ports[0], ports[1], ports[2]);
    repo.git(&["config", "tower.servePort", &config.to_string()]);

    let server = Server::start(
        repo.path(),
        &["--port", &flag.to_string()],
        &[("TOWER_PORT", &env.to_string())],
    );
    assert_eq!(server.port(), flag);
    drop(server);

    let server = Server::start(repo.path(), &[], &[("TOWER_PORT", &env.to_string())]);
    assert_eq!(server.port(), env);
    drop(server);

    let server = Server::start(repo.path(), &[], &[]);
    assert_eq!(server.port(), config);
}

#[test]
fn the_startup_envelope_names_the_address_it_bound() {
    let repo = Repo::new();
    let port = free_port();
    let server = Server::start(repo.path(), &["--port", &port.to_string()], &[]);

    let envelope = &server.startup;
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("serve"));
    assert_eq!(envelope["data"]["host"], serde_json::json!("127.0.0.1"));
    assert_eq!(envelope["data"]["port"], serde_json::json!(port));
    assert_eq!(
        envelope["data"]["address"],
        serde_json::json!(format!("127.0.0.1:{port}"))
    );
    assert_eq!(
        envelope["data"]["url"],
        serde_json::json!(format!("http://127.0.0.1:{port}/"))
    );

    // One envelope at startup, and then it keeps serving — the half of
    // the promise a `--json` caller finds out about last.
    assert_eq!(http(&server.addr, "/").0, 200);
}

#[test]
fn the_root_is_the_app_and_unknown_paths_fall_back_to_it() {
    let repo = Repo::new();
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);

    // The shell is the SvelteKit build, embedded at compile time.
    let (status, head, index) = http(&server.addr, "/");
    assert_eq!(status, 200);
    assert!(index.contains("_app/"), "{index}");
    assert!(head.contains("content-type: text/html"), "{head}");

    // The named landmine: a wire id carries a dot, and lookup is exact
    // match rather than extension sniffing, so an open flight's path
    // answers the shell byte for byte instead of 404ing as a file of
    // unknown type.
    let (status, _, body) = http(&server.addr, "/f/pi-2118.94");
    assert_eq!(status, 200);
    assert_eq!(body, index);

    // The unmounted half survives the app landing: a path nothing
    // answers for under `/api/` says it is not there, in plain text.
    assert_eq!(http(&server.addr, "/api/nothing").0, 404);

    // A real file by its one stable unhashed name, revalidated always.
    let (status, head, _) = http(&server.addr, "/_app/version.json");
    assert_eq!(status, 200);
    assert!(head.contains("content-type: application/json"), "{head}");

    // A hashed asset out of the shell, cacheable forever.
    let asset = index
        .split('"')
        .find(|value| value.starts_with("/_app/immutable/"))
        .expect("an immutable asset in the shell");
    let (status, head, _) = http(&server.addr, asset);
    assert_eq!(status, 200, "{asset}");
    assert!(head.contains("immutable"), "{head}");
}

#[test]
fn a_second_server_on_one_port_is_refused_by_the_socket() {
    let repo = Repo::new();
    let port = free_port().to_string();
    let _first = Server::start(repo.path(), &["--port", &port], &[]);

    let out = ff_tower(repo.path(), &["serve", "--json", "--port", &port]);
    let envelope = refusal(&out, 1, "serve/address-in-use");
    let message = envelope["error"]["message"].as_str().expect("a message");
    assert!(message.contains(&port), "{message}");
    assert_eq!(
        envelope["error"]["exits"][0],
        serde_json::json!("ff tower serve --port <n>")
    );
}

#[test]
fn outside_a_repository_it_refuses_before_it_binds() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let port = free_port();
    // Both flags on purpose: they skip the config lane, so the refusal
    // comes from the server validating the repository at startup rather
    // than from a config read on the way to it.
    let out = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args([
            "serve",
            "--json",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("xdg"))
        .env_remove("FF_REPO")
        .env_remove("TOWER_HOST")
        .env_remove("TOWER_PORT")
        .env_remove("GIT_CONFIG_GLOBAL")
        .output()
        .expect("spawn ff-tower");
    refusal(&out, 1, "repo/error");

    // At startup and not on the first request: nothing ever bound.
    assert!(
        TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "the port was bound before the repository was checked"
    );
}

#[test]
fn a_host_that_does_not_parse_is_the_usage_envelope() {
    let repo = Repo::new();
    let out = ff_tower(repo.path(), &["serve", "--json", "--host", "banana"]);
    let envelope = refusal(&out, 2, "usage/bad-host");
    let message = envelope["error"]["message"].as_str().expect("a message");
    assert!(message.contains("--host"), "{message}");
    assert!(message.contains("banana"), "{message}");
    assert_eq!(
        envelope["error"]["exits"][0],
        serde_json::json!("ff tower serve --host <addr>")
    );
}

#[test]
fn localhost_is_refused_and_the_refusal_names_the_spelling_that_works() {
    let repo = Repo::new();
    // A name is not an address: nothing resolves DNS on the way to a
    // bind, so the one word a person is most likely to try is refused —
    // and told what to type instead.
    let out = ff_tower(repo.path(), &["serve", "--json", "--host", "localhost"]);
    let message = refusal(&out, 2, "usage/bad-host")["error"]["message"].to_string();
    assert!(message.contains("localhost"), "{message}");
    assert!(message.contains("127.0.0.1"), "{message}");
}

#[test]
fn the_environment_and_the_config_refuse_a_host_the_same_way() {
    let repo = Repo::new();

    let out = command(repo.path())
        .args(["serve", "--json"])
        .env("TOWER_HOST", "localhost")
        .output()
        .expect("spawn ff-tower");
    let message = refusal(&out, 2, "usage/bad-host")["error"]["message"].to_string();
    assert!(message.contains("TOWER_HOST"), "{message}");

    // The registry refuses an unusable address on the way in, so the only
    // way to have one stored is to have stored it with git.
    let out = ff_tower(repo.path(), &["config", "--json", "serveHost", "localhost"]);
    refusal(&out, 2, "usage/bad-value");

    repo.git(&["config", "tower.serveHost", "banana"]);
    let out = ff_tower(repo.path(), &["serve", "--json"]);
    let message = refusal(&out, 2, "usage/bad-host")["error"]["message"].to_string();
    assert!(message.contains("tower.serveHost"), "{message}");
}

#[test]
fn the_host_flag_beats_the_environment_beats_the_config() {
    let repo = Repo::new();
    let ports = free_ports(3);
    repo.git(&["config", "tower.serveHost", "127.0.0.1"]);

    let server = Server::start(
        repo.path(),
        &["--host", "0.0.0.0", "--port", &ports[0].to_string()],
        &[("TOWER_HOST", "127.0.0.1")],
    );
    assert_eq!(server.host(), "0.0.0.0");
    drop(server);

    let server = Server::start(
        repo.path(),
        &["--port", &ports[1].to_string()],
        &[("TOWER_HOST", "0.0.0.0")],
    );
    assert_eq!(server.host(), "0.0.0.0");
    drop(server);

    let server = Server::start(repo.path(), &["--port", &ports[2].to_string()], &[]);
    assert_eq!(server.host(), "127.0.0.1");
    drop(server);

    // And the config lane over the compiled default.
    repo.git(&["config", "tower.serveHost", "0.0.0.0"]);
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);
    assert_eq!(server.host(), "0.0.0.0");
}

#[test]
fn a_wide_bind_is_reachable_through_the_loopback() {
    let repo = Repo::new();
    let port = free_port();
    let server = Server::start(
        repo.path(),
        &["--host", "0.0.0.0", "--port", &port.to_string()],
        &[],
    );

    assert_eq!(server.startup["data"]["host"], serde_json::json!("0.0.0.0"));
    assert_eq!(
        server.startup["data"]["address"],
        serde_json::json!(format!("0.0.0.0:{port}"))
    );

    // The wildcard includes the loopback, which is the only interface a
    // test can name and reach on every platform.
    assert_eq!(http(&format!("127.0.0.1:{port}"), "/").0, 200);
}

#[test]
fn a_bind_beyond_the_loopback_says_so_once_on_stderr() {
    let repo = Repo::new();

    let server = Server::watching_stderr(
        repo.path(),
        &["--host", "0.0.0.0", "--port", &free_port().to_string()],
        &[],
    );
    let said = server.stderr_said();
    assert!(said.contains("no authentication"), "{said}");
    // stderr and not stdout, so a --json caller still gets exactly one
    // envelope and a person still gets the sentence.
    assert_eq!(said.lines().count(), 1, "{said}");

    let server = Server::watching_stderr(
        repo.path(),
        &["--host", "127.0.0.1", "--port", &free_port().to_string()],
        &[],
    );
    let quiet = server.stderr();
    assert!(quiet.is_empty(), "the loopback said something: {quiet}");
}
