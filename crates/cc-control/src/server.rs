//! TCP server: vlákno na pripojenia, front požiadaviek pre hostiteľa.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use crate::protocol::{self, ControlError, Request};

/// Ako dlho server čaká na odpoveď hostiteľa.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Najdlhší riadok požiadavky.
const MAX_LINE: u64 = 1 << 20;

const WAITING: u8 = 0;
const TAKEN: u8 = 1;
const CANCELLED: u8 = 2;

/// Požiadavka čakajúca na hostiteľa.
pub struct Incoming {
    pub request: Request,
    reply: SyncSender<Value>,
    state: Arc<AtomicU8>,
}

impl Incoming {
    /// Odošle odpoveď klientovi.
    pub fn respond(self, result: Result<Value, ControlError>) {
        let line = match result {
            Ok(v) => protocol::ok(&self.request.id, v),
            Err(e) => protocol::error(&self.request.id, &e),
        };
        // Klient sa mohol medzitým odpojiť; odpoveď sa potom zahodí.
        let _ = self.reply.send(line);
    }
}

pub struct ControlServer {
    rx: Receiver<Incoming>,
    port: u16,
}

impl ControlServer {
    /// Spustí server na `127.0.0.1:port` (`0` = voľný port). `wake` sa zavolá pri
    /// každej novej požiadavke (okno si vyžiada prekreslenie).
    pub fn start(port: u16, wake: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<Self> {
        Self::start_with_timeout(port, wake, DEFAULT_TIMEOUT)
    }

    pub fn start_with_timeout(
        port: u16,
        wake: Arc<dyn Fn() + Send + Sync>,
        timeout: Duration,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let port = listener.local_addr()?.port();
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("cc-control".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let tx = tx.clone();
                    let wake = wake.clone();
                    let _ = thread::Builder::new()
                        .name("cc-control-conn".into())
                        .spawn(move || serve(stream, tx, wake, timeout));
                }
            })?;
        Ok(ControlServer { rx, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Ďalšia požiadavka, ak nejaká čaká (neblokuje).
    pub fn try_next(&self) -> Option<Incoming> {
        loop {
            let incoming = self.rx.try_recv().ok()?;
            if take(&incoming) {
                return Some(incoming);
            }
        }
    }

    /// Ďalšia požiadavka; čaká najviac `timeout`.
    pub fn next_timeout(&self, timeout: Duration) -> Option<Incoming> {
        let incoming = self.rx.recv_timeout(timeout).ok()?;
        if take(&incoming) {
            Some(incoming)
        } else {
            self.try_next()
        }
    }
}

/// Hostiteľ si požiadavku berie; zrušenú (po vypršaní času) preskočí.
fn take(incoming: &Incoming) -> bool {
    incoming
        .state
        .compare_exchange(WAITING, TAKEN, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

fn serve(
    stream: TcpStream,
    tx: Sender<Incoming>,
    wake: Arc<dyn Fn() + Send + Sync>,
    timeout: Duration,
) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        match (&mut reader).take(MAX_LINE).read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if !line.ends_with('\n') && line.len() as u64 >= MAX_LINE {
            let e = ControlError::params("požiadavka je dlhšia ako 1 MB");
            let _ = writeln!(writer, "{}", protocol::error(&Value::Null, &e));
            return;
        }
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let response = handle_line(text, &tx, &wake, timeout);
        if writeln!(writer, "{response}")
            .and_then(|()| writer.flush())
            .is_err()
        {
            return;
        }
    }
}

fn handle_line(
    text: &str,
    tx: &Sender<Incoming>,
    wake: &Arc<dyn Fn() + Send + Sync>,
    timeout: Duration,
) -> Value {
    let request: Request = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return protocol::error(
                &Value::Null,
                &ControlError::params(format!("zlý JSON požiadavky: {e}")),
            )
        }
    };
    // `ping` odpovie aj vtedy, keď je hostiteľ zaneprázdnený.
    if request.method == "ping" {
        return protocol::ok(&request.id, json!("pong"));
    }
    let id = request.id.clone();
    let state = Arc::new(AtomicU8::new(WAITING));
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    let incoming = Incoming {
        request,
        reply: reply_tx,
        state: state.clone(),
    };
    if tx.send(incoming).is_err() {
        return protocol::error(&id, &ControlError::new("closed", "aplikácia sa ukončuje"));
    }
    wake();
    match reply_rx.recv_timeout(timeout) {
        Ok(response) => response,
        Err(RecvTimeoutError::Disconnected) => protocol::error(
            &id,
            &ControlError::new("closed", "aplikácia požiadavku nevybavila"),
        ),
        Err(RecvTimeoutError::Timeout) => {
            let not_run = state
                .compare_exchange(WAITING, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok();
            if not_run {
                json!({"id": id, "ok": false, "error": "timeout", "kind": "timeout", "state": "not-run"})
            } else {
                // Hostiteľ už požiadavku vybavuje; neskorú odpoveď zahodíme.
                json!({"id": id, "ok": false, "error": "timeout", "kind": "timeout", "state": "may-have-run"})
            }
        }
    }
}
