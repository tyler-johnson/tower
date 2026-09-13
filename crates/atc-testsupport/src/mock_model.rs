//! A scripted model server, for driving a real agent client through one
//! tool call without a model.
//!
//! The live suites (`live_<client>.rs` in `atc-cli`) point a client's
//! base URL here and read back what it sent: the proof that the notice
//! reached the prompt is the request body, and the proof that the shell
//! knows its pilot is the tool output the client carried back on the
//! next turn. Nothing here judges prose; the server answers every model
//! request by a fixed rule and records every one.
//!
//! Three dialects, told apart by the path the client posts to: the
//! OpenAI Responses API (`/responses`, Codex), OpenAI chat completions
//! (`/chat/completions`, Qwen Code), and Anthropic messages
//! (`/messages`, Claude Code — SSE when the body asks to stream, one
//! JSON object when it does not, since Claude Code retries a failed
//! stream unstreamed). Each dialect's first answer is a call to its shell
//! tool running the command the server was started with, and its second
//! is the text `done`.
//!
//! The rule is stateless per request on purpose: a body that does not
//! yet carry the command's output gets the tool call, and a body that
//! does gets the text. A client's retry of the same request, or an
//! extra request after the turn (Qwen Code extracts memories on its way
//! out), cannot desynchronize a counter that does not exist. The marker
//! is `"cmd":"whoami"` — the head of `atc whoami --json`'s envelope,
//! which is only in a body once the tool ran — matched in the escaped
//! form a JSON string carries as well as bare. A body carrying three
//! tool outputs and still no envelope gets the text too: the command
//! is failing in the client's shell, and a turn that ends lets the
//! suite's assertions say so, where one more tool call would loop
//! until the deadline.
//!
//! Dependency-free like the rest of the crate: `std::net` on a thread,
//! JSON as string literals, and one escaper for the command, whose
//! backslashes on Windows are the only thing that needs escaping.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// One recorded POST: the path as requested, the raw header block, and
/// the body as sent.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub path: String,
    pub headers: String,
    pub body: String,
}

/// The server. Bound on start, served until dropped.
pub struct MockModel {
    addr: SocketAddr,
    recorded: Arc<Mutex<Vec<Recorded>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// The marker of a body that carries the tool's output, in the two
/// forms it can take: escaped inside a JSON string, which is how every
/// client today carries a tool result, and bare, for one that nests the
/// output as an object.
const MARKERS: [&str; 2] = [r#"\"cmd\":\"whoami\""#, r#""cmd":"whoami""#];

/// A tool output in each dialect's body: the Responses item, the
/// Anthropic block, the chat message role.
const TOOL_OUTPUTS: [&str; 3] = [
    r#""type":"function_call_output""#,
    r#""type":"tool_result""#,
    r#""role":"tool""#,
];

/// How many failed attempts end the turn without the envelope.
const ATTEMPTS: usize = 3;

impl MockModel {
    /// Bind `127.0.0.1:0` and serve until dropped. `command` is the shell
    /// line the first turn asks the client to run.
    pub fn start(command: &str) -> MockModel {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the mock model");
        let addr = listener.local_addr().expect("a local address");
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let script = Arc::new(Script::new(command));
        let thread = {
            let recorded = Arc::clone(&recorded);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let recorded = Arc::clone(&recorded);
                    let script = Arc::clone(&script);
                    // One thread per connection: a client holds a
                    // keep-alive connection open across turns, and may
                    // open a second while the first idles.
                    std::thread::spawn(move || serve(stream, &recorded, &script));
                }
            })
        };
        MockModel {
            addr,
            recorded,
            stop,
            thread: Some(thread),
        }
    }

    /// `http://127.0.0.1:<port>`, with no path: each client adds its own
    /// `/v1` or not.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Every POST so far, in arrival order.
    pub fn requests(&self) -> Vec<Recorded> {
        self.recorded.lock().expect("the record").clone()
    }
}

impl Drop for MockModel {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // `accept` blocks; one connection wakes it to see the flag.
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The three dialects' answers, built once from the command.
struct Script {
    responses_call: String,
    responses_done: String,
    chat_call: String,
    chat_done: String,
    messages_call: String,
    messages_done: String,
    messages_call_json: String,
    messages_done_json: String,
}

impl Script {
    fn new(command: &str) -> Script {
        Script {
            responses_call: responses_sse(&function_call_item(command)),
            responses_done: responses_sse(ASSISTANT_MESSAGE_ITEM),
            chat_call: chat_sse(
                &format!(
                    r#"{{"role":"assistant","content":null,"tool_calls":[{{"index":0,"id":"call_1","type":"function","function":{{"name":"run_shell_command","arguments":{}}}}}]}}"#,
                    json_string(&format!(r#"{{"command":{}}}"#, json_string(command)))
                ),
                "tool_calls",
            ),
            chat_done: chat_sse(r#"{"role":"assistant","content":"done"}"#, "stop"),
            messages_call: messages_sse(Some(command)),
            messages_done: messages_sse(None),
            messages_call_json: messages_json(&tool_use_block(command), "tool_use"),
            messages_done_json: messages_json(TEXT_BLOCK, "end_turn"),
        }
    }

    /// The answer for one POST: the body and its content type, or
    /// `None` for a path no dialect claims.
    fn answer(&self, path: &str, body: &str) -> Option<(&str, &str)> {
        let outputs: usize = TOOL_OUTPUTS
            .iter()
            .map(|marker| body.matches(marker).count())
            .sum();
        let done = MARKERS.iter().any(|marker| body.contains(marker)) || outputs >= ATTEMPTS;
        let path = path.split('?').next().unwrap_or(path);
        if path.ends_with("/responses") {
            return Some((
                if done {
                    &self.responses_done
                } else {
                    &self.responses_call
                },
                SSE,
            ));
        }
        if path.ends_with("/chat/completions") {
            return Some((
                if done {
                    &self.chat_done
                } else {
                    &self.chat_call
                },
                SSE,
            ));
        }
        if path.contains("/messages") {
            return Some(if body.contains(r#""stream":true"#) {
                (
                    if done {
                        &self.messages_done
                    } else {
                        &self.messages_call
                    },
                    SSE,
                )
            } else {
                (
                    if done {
                        &self.messages_done_json
                    } else {
                        &self.messages_call_json
                    },
                    JSON,
                )
            });
        }
        None
    }
}

const SSE: &str = "text/event-stream";
const JSON: &str = "application/json";

/// The token counts Codex insists on in `response.completed`.
const USAGE: &str = r#"{"input_tokens":1,"output_tokens":1,"total_tokens":2,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}"#;

const ASSISTANT_MESSAGE_ITEM: &str = r#"{"type":"message","id":"msg_1","role":"assistant","status":"completed","content":[{"type":"output_text","text":"done","annotations":[]}]}"#;

const TEXT_BLOCK: &str = r#"{"type":"text","text":"done"}"#;

fn function_call_item(command: &str) -> String {
    format!(
        r#"{{"type":"function_call","id":"fc_1","call_id":"call_1","name":"exec_command","arguments":{}}}"#,
        json_string(&format!(r#"{{"cmd":{}}}"#, json_string(command)))
    )
}

fn tool_use_block(command: &str) -> String {
    format!(
        r#"{{"type":"tool_use","id":"toolu_1","name":"Bash","input":{{"command":{}}}}}"#,
        json_string(command)
    )
}

/// One Responses API stream carrying one output item: created, the
/// item done, completed.
fn responses_sse(item: &str) -> String {
    let created =
        r#"{"id":"resp_1","object":"response","created_at":0,"status":"in_progress","output":[]}"#;
    let completed = format!(
        r#"{{"id":"resp_1","object":"response","created_at":0,"status":"completed","output":[{item}],"usage":{USAGE}}}"#
    );
    sse(&[
        format!(r#"{{"type":"response.created","response":{created}}}"#),
        format!(r#"{{"type":"response.output_item.done","output_index":0,"item":{item}}}"#),
        format!(r#"{{"type":"response.completed","response":{completed}}}"#),
    ])
}

/// One chat completions stream: a single chunk carrying the whole
/// delta, then the terminator.
fn chat_sse(delta: &str, finish: &str) -> String {
    format!(
        "data: {{\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"mock\",\"choices\":[{{\"index\":0,\"delta\":{delta},\"finish_reason\":\"{finish}\"}}]}}\n\ndata: [DONE]\n\n"
    )
}

/// One Anthropic messages stream carrying one content block: a `Bash`
/// call running `command`, or the text `done` without one. A tool block
/// starts empty and its input arrives as one `input_json_delta`, the
/// way the real API sends it.
fn messages_sse(command: Option<&str>) -> String {
    let (start, delta, stop) = match command {
        Some(command) => (
            r#"{"type":"tool_use","id":"toolu_1","name":"Bash","input":{}}"#.to_string(),
            format!(
                r#"{{"type":"input_json_delta","partial_json":{}}}"#,
                json_string(&format!(r#"{{"command":{}}}"#, json_string(command)))
            ),
            "tool_use",
        ),
        None => (
            r#"{"type":"text","text":""}"#.to_string(),
            r#"{"type":"text_delta","text":"done"}"#.to_string(),
            "end_turn",
        ),
    };
    sse(&[
        r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","model":"mock","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}}}"#.to_string(),
        format!(r#"{{"type":"content_block_start","index":0,"content_block":{start}}}"#),
        format!(r#"{{"type":"content_block_delta","index":0,"delta":{delta}}}"#),
        r#"{"type":"content_block_stop","index":0}"#.to_string(),
        format!(
            r#"{{"type":"message_delta","delta":{{"stop_reason":"{stop}","stop_sequence":null}},"usage":{{"output_tokens":1}}}}"#
        ),
        r#"{"type":"message_stop"}"#.to_string(),
    ])
}

/// Named events, one per object, the name read off the object's `type`.
fn sse(events: &[String]) -> String {
    events
        .iter()
        .map(|event| {
            let name = event
                .split("\"type\":\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .expect("an event type");
            format!("event: {name}\ndata: {event}\n\n")
        })
        .collect()
}

/// The unstreamed Anthropic answer: one message object.
fn messages_json(block: &str, stop: &str) -> String {
    format!(
        r#"{{"id":"msg_1","type":"message","role":"assistant","model":"mock","content":[{block}],"stop_reason":"{stop}","stop_sequence":null,"usage":{{"input_tokens":1,"output_tokens":1}}}}"#
    )
}

/// `text` as a JSON string literal, quotes included.
pub fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// One connection: requests in sequence until the client closes it.
fn serve(stream: TcpStream, recorded: &Mutex<Vec<Recorded>>, script: &Script) {
    // An idle keep-alive connection is the client's to close; the
    // timeout is for one a dead client left behind.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(300)));
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();

        let mut headers = String::new();
        let mut length: Option<usize> = None;
        loop {
            let mut header = String::new();
            match reader.read_line(&mut header) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            if header == "\r\n" || header == "\n" {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().ok();
            }
            headers.push_str(&header);
        }

        let (status, content_type, body) = match (method.as_str(), length) {
            ("POST", Some(length)) => {
                let mut raw = vec![0; length];
                if reader.read_exact(&mut raw).is_err() {
                    return;
                }
                let body = String::from_utf8_lossy(&raw).into_owned();
                let answer = script.answer(&path, &body);
                recorded.lock().expect("the record").push(Recorded {
                    path: path.clone(),
                    headers: headers.clone(),
                    body,
                });
                match answer {
                    Some((body, content_type)) => ("200 OK", content_type, body.to_string()),
                    None => ("404 Not Found", JSON, "{}".to_string()),
                }
            }
            // A body framed any other way — chunked — is not read: it
            // is recorded empty, so the suite's body assertions name it.
            ("POST", None) => {
                recorded.lock().expect("the record").push(Recorded {
                    path: path.clone(),
                    headers: headers.clone(),
                    body: String::new(),
                });
                ("411 Length Required", JSON, "{}".to_string())
            }
            _ => ("404 Not Found", JSON, "{}".to_string()),
        };
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            body.len()
        );
        let stream = reader.get_mut();
        if stream.write_all(head.as_bytes()).is_err() || stream.write_all(body.as_bytes()).is_err()
        {
            return;
        }
        let _ = stream.flush();
        if status.starts_with("411") {
            // The unread body would be parsed as the next request line.
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One POST over a raw socket, `Connection: close` so the response
    /// is read to EOF.
    fn post(model: &MockModel, path: &str, body: &str) -> (u16, String, String) {
        let mut stream = TcpStream::connect(model.addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body.as_bytes()).unwrap();
        // The server keeps the connection; the client's half-close ends
        // the read below.
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").expect("a response");
        let status: u16 = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse().ok())
            .expect("a status");
        (status, head.to_string(), body.to_string())
    }

    const CMD: &str = r#"C:\tools\atc whoami --json"#;

    #[test]
    fn responses_gets_the_tool_call_then_the_text() {
        let model = MockModel::start(CMD);
        let (status, head, body) = post(&model, "/v1/responses", r#"{"input":[],"stream":true}"#);
        assert_eq!(status, 200);
        assert!(head.contains("text/event-stream"), "{head}");
        assert!(body.starts_with("event: response.created\n"), "{body}");
        assert!(body.contains(r#""name":"exec_command""#), "{body}");
        assert!(
            body.contains(r#""arguments":"{\"cmd\":\"C:\\\\tools\\\\atc whoami --json\"}""#),
            "{body}"
        );
        assert!(body.contains("event: response.completed\n"), "{body}");
        assert!(body.contains(r#""total_tokens":2"#), "{body}");

        let (_, _, body) = post(
            &model,
            "/v1/responses",
            r#"{"input":[{"type":"function_call_output","output":"{\"atc\":1,\"cmd\":\"whoami\"}"}]}"#,
        );
        assert!(body.contains(r#""text":"done""#), "{body}");
        assert!(!body.contains("exec_command"), "{body}");
    }

    #[test]
    fn chat_completions_gets_the_tool_call_then_the_text() {
        let model = MockModel::start("atc whoami --json");
        let (status, head, body) = post(&model, "/v1/chat/completions", r#"{"messages":[]}"#);
        assert_eq!(status, 200);
        assert!(head.contains("text/event-stream"), "{head}");
        assert!(body.contains(r#""name":"run_shell_command""#), "{body}");
        assert!(
            body.contains(r#""arguments":"{\"command\":\"atc whoami --json\"}""#),
            "{body}"
        );
        assert!(body.contains(r#""finish_reason":"tool_calls""#), "{body}");
        assert!(body.ends_with("data: [DONE]\n\n"), "{body}");

        let (_, _, body) = post(
            &model,
            "/v1/chat/completions",
            r#"{"messages":[{"role":"tool","content":"{\"atc\":1,\"cmd\":\"whoami\"}"}]}"#,
        );
        assert!(body.contains(r#""content":"done""#), "{body}");
        assert!(body.contains(r#""finish_reason":"stop""#), "{body}");
    }

    #[test]
    fn messages_streams_or_not_as_the_body_asks() {
        let model = MockModel::start("atc whoami --json");
        let (status, head, body) = post(&model, "/v1/messages?beta=true", r#"{"stream":true}"#);
        assert_eq!(status, 200);
        assert!(head.contains("text/event-stream"), "{head}");
        assert!(body.starts_with("event: message_start\n"), "{body}");
        assert!(body.contains(r#""name":"Bash","input":{}"#), "{body}");
        assert!(
            body.contains(r#""partial_json":"{\"command\":\"atc whoami --json\"}""#),
            "{body}"
        );
        assert!(body.contains(r#""stop_reason":"tool_use""#), "{body}");

        let (status, head, body) = post(&model, "/v1/messages", r#"{"stream":false}"#);
        assert_eq!(status, 200);
        assert!(head.contains("application/json"), "{head}");
        assert!(body.starts_with(r#"{"id":"msg_1""#), "{body}");
        assert!(
            body.contains(r#""input":{"command":"atc whoami --json"}"#),
            "{body}"
        );

        let (_, _, body) = post(
            &model,
            "/v1/messages",
            r#"{"stream":false,"messages":[{"content":"{\"atc\":1,\"cmd\":\"whoami\"}"}]}"#,
        );
        assert!(body.contains(r#""stop_reason":"end_turn""#), "{body}");
        assert!(body.contains(r#""text":"done""#), "{body}");
    }

    #[test]
    fn requests_are_recorded_in_order_and_unknown_paths_are_404() {
        let model = MockModel::start("atc whoami --json");
        post(&model, "/v1/responses", "first");
        let (status, _, _) = post(&model, "/v1/models", "second");
        assert_eq!(status, 404);
        post(&model, "/v1/messages", "third");
        let recorded = model.requests();
        let bodies: Vec<&str> = recorded.iter().map(|r| r.body.as_str()).collect();
        assert_eq!(bodies, ["first", "second", "third"]);
        assert_eq!(recorded[1].path, "/v1/models");
        assert!(recorded[0].headers.contains("Content-Length: 5"));
    }

    #[test]
    fn a_command_that_keeps_failing_ends_the_turn() {
        let model = MockModel::start("atc whoami --json");
        let failed =
            r#"{"type":"function_call_output","call_id":"call_1","output":"bwrap: no permission"}"#;
        let two = format!(r#"{{"input":[{failed},{failed}]}}"#);
        let (_, _, body) = post(&model, "/v1/responses", &two);
        assert!(
            body.contains("exec_command"),
            "two failures try again: {body}"
        );
        let three = format!(r#"{{"input":[{failed},{failed},{failed}]}}"#);
        let (_, _, body) = post(&model, "/v1/responses", &three);
        assert!(!body.contains("exec_command"), "three end the turn: {body}");
        assert!(body.contains(r#""text":"done""#), "{body}");
    }

    #[test]
    fn json_string_escapes() {
        assert_eq!(json_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(json_string("x\ny"), r#""x\ny""#);
    }
}
