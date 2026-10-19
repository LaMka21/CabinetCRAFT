//! Server riadiaceho kanála so session bez okna.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use cc_control::{engine, ControlError, ControlServer};
use cc_engine::Session;
use serde_json::{json, Value};

fn session() -> Session {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/project.toml");
    Session::open(&p).unwrap()
}

struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Client {
    fn connect(port: u16) -> Client {
        let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        Client {
            writer: s.try_clone().unwrap(),
            reader: BufReader::new(s),
        }
    }

    fn send_raw(&mut self, line: &str) -> Value {
        writeln!(self.writer, "{line}").unwrap();
        let mut answer = String::new();
        self.reader.read_line(&mut answer).unwrap();
        serde_json::from_str(&answer).unwrap()
    }

    fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send_raw(&json!({"id": id, "method": method, "params": params}).to_string())
    }
}

/// Hostiteľ: vybavuje požiadavky, kým klient neskončí.
fn pump(server: &ControlServer, mut s: Session, done: impl Fn() -> bool) -> Session {
    while !done() {
        if let Some(inc) = server.next_timeout(Duration::from_millis(20)) {
            let r = engine::handle(&mut s, &inc.request.method, inc.request.params.clone())
                .unwrap_or_else(|| Err(ControlError::unknown_method(&inc.request.method)));
            inc.respond(r);
        }
    }
    s
}

#[test]
fn engine_methods_over_tcp() {
    let server = ControlServer::start(0, Arc::new(|| {})).unwrap();
    let port = server.port();
    let client = thread::spawn(move || {
        let mut c = Client::connect(port);
        let pong = c.call(1, "ping", json!({}));
        assert_eq!(pong, json!({"id": 1, "ok": true, "result": "pong"}));

        let methods = c.call(2, "control.methods", Value::Null);
        assert!(methods["result"].as_array().unwrap().len() >= 5);

        let added = c.call(
            3,
            "engine.execute",
            json!({"command": "cabinet.add", "params": {"type": "tall"}}),
        );
        assert_eq!(added["ok"], true, "{added}");
        assert_eq!(added["result"]["id"], "VY1");

        let p = c.call(4, "project.inspect", json!({}));
        assert_eq!(p["result"]["cabinets"].as_array().unwrap().len(), 3);

        let bad = c.call(5, "engine.execute", json!({"command": "cabinet.fly"}));
        assert_eq!(bad["ok"], false);
        assert_eq!(bad["kind"], "unknown-command");

        let unknown = c.call(6, "ui.fly", json!({}));
        assert_eq!(unknown["kind"], "unknown-method");
        assert!(unknown["error"]
            .as_str()
            .unwrap()
            .contains("control.methods"));

        let garbage = c.send_raw("{nie je json");
        assert_eq!(garbage["ok"], false);
        assert_eq!(garbage["kind"], "params");

        let undo = c.call(8, "command", json!({"command": "edit.undo"}));
        assert_eq!(undo["ok"], true, "{undo}");
    });
    let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let f = finished.clone();
    let watcher = thread::spawn(move || {
        let r = client.join();
        f.store(true, std::sync::atomic::Ordering::SeqCst);
        r
    });
    let s = pump(&server, session(), || {
        finished.load(std::sync::atomic::Ordering::SeqCst)
    });
    watcher.join().unwrap().unwrap();
    assert_eq!(s.doc().unwrap().project.cabinets.len(), 2, "späť cez kanál");
}

#[test]
fn timeout_before_host_takes_request_is_not_run() {
    let server =
        ControlServer::start_with_timeout(0, Arc::new(|| {}), Duration::from_millis(100)).unwrap();
    let mut c = Client::connect(server.port());
    let r = c.call(
        7,
        "engine.execute",
        json!({"command": "cabinet.add", "params": {"type": "base"}}),
    );
    assert_eq!(r["state"], "not-run", "{r}");
    // zrušená požiadavka sa hostiteľovi už neponúkne
    assert!(server.try_next().is_none());
}
