//! The serve suites' shared harness: a spawned binary, a server child
//! that does not exit, and a raw one-request HTTP client.
//!
//! A directory module rather than a file so it is not itself a test
//! crate; each suite that says `mod support;` compiles its own copy, and
//! the allow below is for the helpers that copy does not reach.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The binary, addressed at a fixture: `FF_REPO` is the production
/// handshake, `HOME` and `XDG_CONFIG_HOME` point into the fixture so
/// neither the developer's git config nor their environment can reach a
/// spawn, and the serve lanes' variables are cleared for the same
/// reason.
pub fn command(repo: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ff-tower"));
    command
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", root(repo).join("xdg"))
        .env("HOME", root(repo))
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", root(repo))
        .env_remove("GIT_CONFIG_GLOBAL")
        .env_remove("TOWER_HOST")
        .env_remove("TOWER_PORT");
    command
}

/// A spawn that runs to completion — a verb, never a bound `serve`.
pub fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    command(repo).args(args).output().expect("spawn ff-tower")
}

/// The fixture tempdir holding `repo/` — the suite's `HOME`.
pub fn root(repo: &Path) -> std::path::PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .to_path_buf()
}

pub fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("an envelope")
}

/// Assert a refusal: the exit code, and the envelope's error id.
pub fn refusal(output: &Output, code: i32, id: &str) -> serde_json::Value {
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
pub struct Server {
    child: Child,
    pub addr: String,
    pub startup: serde_json::Value,
}

impl Server {
    /// Spawn one and wait for the line it prints once the socket is
    /// bound. `--json` rides every start: the envelope is the only honest
    /// way to learn the address, and it is worth asserting anyway.
    pub fn start(repo: &Path, args: &[&str], env: &[(&str, &str)]) -> Server {
        Server::spawn(repo, args, env, Stdio::inherit())
    }

    /// The same spawn with stderr held, for the one case that asserts on
    /// what the verb says there rather than on stdout.
    pub fn watching_stderr(repo: &Path, args: &[&str], env: &[(&str, &str)]) -> Server {
        Server::spawn(repo, args, env, Stdio::piped())
    }

    fn spawn(repo: &Path, args: &[&str], env: &[(&str, &str)], stderr: Stdio) -> Server {
        let mut command = command(repo);
        command
            .arg("serve")
            .arg("--json")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(stderr);
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

    pub fn port(&self) -> u16 {
        let port = self.startup["data"]["port"].as_u64().expect("a port");
        u16::try_from(port).expect("a port fits a u16")
    }

    pub fn host(&self) -> String {
        self.startup["data"]["host"]
            .as_str()
            .expect("a host")
            .to_string()
    }

    /// Everything the child wrote to stderr, read after the kill: the
    /// pipe reaches EOF only once the process is gone. For proving
    /// silence only — a kill races anything the child was about to say,
    /// so a positive assertion belongs on [`Server::stderr_said`].
    pub fn stderr(mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut text = String::new();
        self.child
            .stderr
            .take()
            .expect("piped stderr")
            .read_to_string(&mut text)
            .expect("read stderr");
        text
    }

    /// Everything the child wrote to stderr, with the first line awaited
    /// before the kill. The kill-then-drain above can land between the
    /// child's stdout envelope and its stderr sentence and lose the
    /// sentence; blocking on one line first cannot.
    pub fn stderr_said(mut self) -> String {
        let mut stderr = self.child.stderr.take().expect("piped stderr");
        let mut reader = BufReader::new(&mut stderr);
        let mut said = String::new();
        reader.read_line(&mut said).expect("a stderr line");
        let _ = self.child.kill();
        let _ = self.child.wait();
        reader.read_to_string(&mut said).expect("read stderr");
        said
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
pub fn free_ports(n: usize) -> Vec<u16> {
    let held: Vec<TcpListener> = (0..n)
        .map(|_| TcpListener::bind("127.0.0.1:0").expect("an ephemeral port"))
        .collect();
    held.iter()
        .map(|listener| listener.local_addr().expect("a local address").port())
        .collect()
}

pub fn free_port() -> u16 {
    free_ports(1)[0]
}

/// One GET over a raw socket: status, the raw header block, the body.
pub fn http(addr: &str, path: &str) -> (u16, String, String) {
    request(addr, "GET", path)
}

/// One POST over the same raw socket, the body carried with its length
/// and the JSON content type.
pub fn post(addr: &str, path: &str, body: &str) -> (u16, String, String) {
    exchange(addr, "POST", path, Some(body))
}

/// One HTTP/1.1 request over a raw socket, answering with the status
/// code, the header block, and the body. The suites need a client for a
/// handful of requests, and `Connection: close` ends the response at EOF
/// — so nothing here has to understand keep-alive or chunking.
pub fn request(addr: &str, method: &str, path: &str) -> (u16, String, String) {
    exchange(addr, method, path, None)
}

fn exchange(addr: &str, method: &str, path: &str, body: Option<&str>) -> (u16, String, String) {
    let mut stream = connect(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("a read timeout");
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if let Some(body) = body {
        head.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .expect("write the request");
    if let Some(body) = body {
        stream.write_all(body.as_bytes()).expect("write the body");
    }

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
    (status, head.to_string(), body.to_string())
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

/// One SSE subscription, held open. The request goes as HTTP/1.0 on
/// purpose: hyper answers a 1.0 request without chunked framing — the
/// body is close-delimited instead — so the reader stays line-wise and
/// needs no chunk decoder. `exchange` cannot serve here: it reads to
/// EOF, and a stream's EOF is the server shutting down.
pub struct Sse {
    reader: BufReader<TcpStream>,
}

/// Subscribe: send the GET, assert the 200 and the `text/event-stream`
/// content type, consume the header block, and hand back the stream.
pub fn sse(addr: &str, path: &str) -> Sse {
    let mut stream = connect(addr);
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("a read timeout");
    stream
        .write_all(format!("GET {path} HTTP/1.0\r\nHost: {addr}\r\n\r\n").as_bytes())
        .expect("write the request");

    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status).expect("a status line");
    assert!(status.contains(" 200 "), "not a 200: {status}");
    let mut event_stream = false;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("a header line");
        if line == "\r\n" || line == "\n" {
            break;
        }
        if line.to_lowercase().contains("content-type") {
            assert!(line.to_lowercase().contains("text/event-stream"), "{line}");
            event_stream = true;
        }
    }
    assert!(event_stream, "no content type on the feed");
    Sse { reader }
}

impl Sse {
    /// Block for the next event's `data:` payload — lines joined with
    /// newlines, keep-alive comments skipped — with the socket's 10s
    /// read timeout as the deadline.
    pub fn next_data(&mut self) -> String {
        let mut data: Vec<String> = Vec::new();
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).expect("an event line");
            assert!(read > 0, "the stream ended mid-event");
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if !data.is_empty() {
                    return data.join("\n");
                }
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                data.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
            }
        }
    }
}
