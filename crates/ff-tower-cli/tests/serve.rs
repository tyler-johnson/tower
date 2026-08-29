//! `ff tower serve` against the real binary: the port's four lanes, the
//! socket, and what the two mounted routes answer.
//!
//! What this file deliberately does not prove is anything about the
//! board. The server serves one placeholder page — there is no read API,
//! no change feed, and no frontend yet — so every assertion here is about
//! the verb, the socket, and the envelope, and they should keep passing
//! unchanged when routes land underneath them. The compiled default port
//! is not bound either: 7420 is where a person dogfooding tower already
//! has a server, and a suite that fought them for it would fail for the
//! wrong reason. That the default is 7420 is core's own test.
//!
//! The shape no other suite in this crate has is a child that does not
//! exit. [`Server`] spawns one with `--json`, reads the startup envelope
//! for the address it actually bound, and kills it on drop. Every spawn
//! points `HOME` and `XDG_CONFIG_HOME` into the fixture and clears
//! `TOWER_PORT`, so neither the developer's git config nor their
//! environment can decide which port a test binds.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use ff_tower_testsupport::Repo;

fn command(repo: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ff-tower"));
    command
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", root(repo).join("xdg"))
        .env("HOME", root(repo))
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", root(repo))
        .env_remove("GIT_CONFIG_GLOBAL")
        .env_remove("TOWER_PORT");
    command
}

/// A spawn that runs to completion. Only for the refusals: a `serve` that
/// bound would never return.
fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    command(repo).args(args).output().expect("spawn ff-tower")
}

/// The fixture tempdir holding `repo/` — the suite's `HOME`.
fn root(repo: &Path) -> std::path::PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .to_path_buf()
}

fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("an envelope")
}

/// Assert a refusal: the exit code, and the envelope's error id.
fn refusal(output: &Output, code: i32, id: &str) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let envelope = envelope(output);
    assert_eq!(envelope["error"]["id"], serde_json::json!(id));
    envelope
}

/// A running server, killed on drop.
struct Server {
    child: Child,
    addr: String,
    startup: serde_json::Value,
}

impl Server {
    /// Spawn one and wait for the line it prints once the socket is
    /// bound. `--json` rides every start: the envelope is the only honest
    /// way to learn the address, and it is worth asserting anyway.
    fn start(repo: &Path, args: &[&str], env: &[(&str, &str)]) -> Server {
        let mut command = command(repo);
        command
            .arg("serve")
            .arg("--json")
            .args(args)
            .stdout(Stdio::piped());
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("spawn ff-tower serve");

        // Borrowed rather than taken, so the pipe outlives this line: a
        // closed read end would turn any later write in the child into a
        // broken pipe.
        let mut line = String::new();
        {
            let stdout = child.stdout.as_mut().expect("piped stdout");
            BufReader::new(stdout)
                .read_line(&mut line)
                .expect("the startup envelope");
        }
        let startup: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|err| panic!("not an envelope: {err}\n{line}"));
        let addr = startup["data"]["address"]
            .as_str()
            .unwrap_or_else(|| panic!("no bound address in {startup}"))
            .to_string();
        Server {
            child,
            addr,
            startup,
        }
    }

    fn port(&self) -> u16 {
        let port = self.startup["data"]["port"].as_u64().expect("a port");
        u16::try_from(port).expect("a port fits a u16")
    }
}

impl Drop for Server {
    /// Killed rather than signalled: Ctrl-C is a person's gesture and
    /// `std` has no way to send one, so what the suite proves about
    /// shutdown is that nothing is left behind — the port frees, which
    /// the next `bind` in the same test would notice.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Ports that were free a moment ago, held all at once so they come back
/// distinct and released together. A race with the rest of the machine is
/// possible and has never been worth the machinery to avoid; `--port 0`
/// is the raceless alternative, and the precedence tests cannot use it
/// because they have to name the number they expect.
fn free_ports(n: usize) -> Vec<u16> {
    let held: Vec<TcpListener> = (0..n)
        .map(|_| TcpListener::bind("127.0.0.1:0").expect("an ephemeral port"))
        .collect();
    held.iter()
        .map(|listener| listener.local_addr().expect("a local address").port())
        .collect()
}

fn free_port() -> u16 {
    free_ports(1)[0]
}

/// One HTTP/1.1 request over a raw socket, answering with the status code
/// and the body. The suite needs a client for two requests, and
/// `Connection: close` ends the response at EOF — so nothing here has to
/// understand keep-alive or chunking.
fn http(addr: &str, path: &str) -> (u16, String) {
    let mut stream = connect(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("a read timeout");
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .expect("write the request");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read the response");
    let (head, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("not an HTTP response: {response}"));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line: {head}"));
    (status, body.to_string())
}

/// The socket is bound before the envelope is printed, so a connection
/// cannot be refused — but the accept loop starts a moment after, and a
/// loaded machine deserves a second try rather than a flake.
fn connect(addr: &str) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match TcpStream::connect(addr) {
            Ok(stream) => return stream,
            Err(err) if Instant::now() >= deadline => panic!("{addr} never answered: {err}"),
            Err(_) => std::thread::sleep(Duration::from_millis(25)),
        }
    }
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
fn the_root_is_the_placeholder_and_anything_else_is_not_found() {
    let repo = Repo::new();
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);

    let (status, body) = http(&server.addr, "/");
    assert_eq!(status, 200);
    assert!(body.contains("<title>tower</title>"), "{body}");

    // The unmounted half: `/api` is where the read and verb APIs land,
    // and until they do it should say it is not there rather than answer
    // blank.
    assert_eq!(http(&server.addr, "/api/board").0, 404);
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
    // `--port` on purpose: it skips the config lane, so the refusal comes
    // from the server validating the repository at startup rather than
    // from a config read on the way to it.
    let out = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(["serve", "--json", "--port", &port.to_string()])
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("xdg"))
        .env_remove("FF_REPO")
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
