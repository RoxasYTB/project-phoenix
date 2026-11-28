pub mod protocol;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::Message;

pub use protocol::{DaveServerMessage, Ready, ServerMessage, SessionDescription, VideoPayload, VoiceConnectInfo};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Disconnected,
    Identifying,
    Resuming,
    Connected,
}

pub struct GatewayMachine {
    info: VoiceConnectInfo,
    state: State,
    seq_ack: Option<u64>,
    session_established: bool,
    heartbeat_interval: Option<Duration>,
    pending_nonce: Option<u64>,
    missed_acks: u32,
}

impl GatewayMachine {
    pub fn new(info: VoiceConnectInfo) -> Self {
        Self {
            info,
            state: State::Disconnected,
            seq_ack: None,
            session_established: false,
            heartbeat_interval: None,
            pending_nonce: None,
            missed_acks: 0,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn seq_ack(&self) -> Option<u64> {
        self.seq_ack
    }

    pub fn heartbeat_interval(&self) -> Option<Duration> {
        self.heartbeat_interval
    }

    pub fn on_open(&mut self) -> String {
        self.heartbeat_interval = None;
        self.pending_nonce = None;
        self.missed_acks = 0;
        if self.session_established {
            self.state = State::Resuming;
            protocol::resume(&self.info, self.seq_ack)
        } else {
            self.state = State::Identifying;
            protocol::identify(&self.info)
        }
    }

    fn track(&mut self, d: protocol::Decoded) -> ServerMessage {
        if let Some(s) = d.seq {
            self.seq_ack = Some(s);
        }
        match &d.msg {
            ServerMessage::Hello { heartbeat_interval_ms } => {
                let ms = heartbeat_interval_ms.clamp(1.0, 600_000.0);
                self.heartbeat_interval = Some(Duration::from_millis(ms as u64));
            }
            ServerMessage::Ready(_) => {
                self.session_established = true;
                self.state = State::Connected;
            }
            ServerMessage::Resumed => self.state = State::Connected,
            ServerMessage::HeartbeatAck { .. } if self.pending_nonce.is_some() => {
                self.pending_nonce = None;
                self.missed_acks = 0;
            }
            _ => {}
        }
        d.msg
    }

    pub fn on_text(&mut self, t: &str) -> Result<ServerMessage> {
        let d = protocol::decode_text(t)?;
        Ok(self.track(d))
    }

    pub fn on_binary(&mut self, b: &[u8]) -> Result<ServerMessage> {
        let d = protocol::decode_binary(b)?;
        Ok(self.track(d))
    }

    pub fn heartbeat(&mut self, nonce: u64) -> Result<String> {
        if self.pending_nonce.is_some() {
            self.missed_acks += 1;
            if self.missed_acks >= 2 {
                return Err(Error::Timeout("heartbeat ack"));
            }
        }
        self.pending_nonce = Some(nonce);
        Ok(protocol::heartbeat(nonce, self.seq_ack))
    }

    pub fn on_close(&mut self, code: Option<u16>) -> bool {
        self.state = State::Disconnected;
        let resumable = code.is_none_or(protocol::close_code_resumable);
        if !resumable {
            self.session_established = false;
        }
        resumable && self.session_established
    }
}

#[derive(Debug)]
pub enum Command {
    Text(String),
    Binary { opcode: u8, payload: Vec<u8> },
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GatewayEvent {
    Message(ServerMessage),
    Reconnecting { code: Option<u16> },
    Closed { code: Option<u16>, reason: String },
}

#[derive(Clone)]
pub struct GatewayHandle {
    tx: mpsc::UnboundedSender<Command>,
}

impl GatewayHandle {
    fn send(&self, c: Command) -> Result<()> {
        self.tx.send(c).map_err(|_| Error::Closed { code: None, reason: "gateway task ended".into() })
    }

    pub fn send_text(&self, s: String) -> Result<()> {
        self.send(Command::Text(s))
    }

    pub fn select_protocol(&self, address: &str, port: u16, mode: &str, codecs: &[crate::codec::CodecOffer]) -> Result<()> {
        self.send_text(protocol::select_protocol(address, port, mode, codecs))
    }

    pub fn speaking(&self, flags: u32, ssrc: u32) -> Result<()> {
        self.send_text(protocol::speaking(flags, ssrc))
    }

    pub fn video(&self, p: &VideoPayload) -> Result<()> {
        self.send_text(protocol::video(p)?)
    }

    pub fn media_sink_wants(&self, wants: &serde_json::Value) -> Result<()> {
        self.send_text(protocol::media_sink_wants(wants))
    }

    pub fn dave_transition_ready(&self, transition_id: u16) -> Result<()> {
        self.send_text(protocol::dave_transition_ready(transition_id))
    }

    pub fn dave_invalid_commit_welcome(&self, transition_id: u16) -> Result<()> {
        self.send_text(protocol::dave_invalid_commit_welcome(transition_id))
    }

    pub fn dave_key_package(&self, kp: Vec<u8>) -> Result<()> {
        self.send(Command::Binary { opcode: protocol::op::DAVE_MLS_KEY_PACKAGE, payload: kp })
    }

    pub fn dave_commit_welcome(&self, cw: &crate::dave::mls::CommitWelcome) -> Result<()> {
        let mut p = cw.commit.clone();
        if let Some(w) = &cw.welcome {
            p.extend_from_slice(w);
        }
        self.send(Command::Binary { opcode: protocol::op::DAVE_MLS_COMMIT_WELCOME, payload: p })
    }

    pub fn close(&self) -> Result<()> {
        self.send(Command::Close)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReconnectPolicy {
    pub max_attempts: u32,
    pub backoff: Duration,
    pub hello_timeout: Duration,
}

pub const HELLO_TIMEOUT: Duration = Duration::from_secs(20);

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self { max_attempts: 5, backoff: Duration::from_millis(500), hello_timeout: HELLO_TIMEOUT }
    }
}

pub fn spawn(
    info: VoiceConnectInfo,
    url: Option<String>,
    policy: ReconnectPolicy,
) -> (GatewayHandle, mpsc::UnboundedReceiver<GatewayEvent>, JoinHandle<Result<()>>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (etx, erx) = mpsc::unbounded_channel();
    let url = url.unwrap_or_else(|| info.url());
    let task = tokio::spawn(run(GatewayMachine::new(info), url, policy, rx, etx));
    (GatewayHandle { tx }, erx, task)
}

enum Exit {
    UserClose,
    Closed(Option<u16>, String),
}

async fn run(
    mut m: GatewayMachine,
    url: String,
    policy: ReconnectPolicy,
    mut cmds: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<GatewayEvent>,
) -> Result<()> {
    let mut attempts = 0u32;
    loop {
        let exit = match connect_once(&mut m, &url, policy.hello_timeout, &mut cmds, &events).await {
            Ok(e) => e,
            Err(e) => Exit::Closed(None, e.to_string()),
        };
        match exit {
            Exit::UserClose => {
                let _ = events.send(GatewayEvent::Closed { code: Some(1000), reason: "closed by client".into() });
                return Ok(());
            }
            Exit::Closed(code, reason) => {
                if m.state() == State::Connected {
                    attempts = 0;
                }
                attempts += 1;
                if m.on_close(code) && attempts <= policy.max_attempts {
                    let _ = events.send(GatewayEvent::Reconnecting { code });
                    tokio::time::sleep(policy.backoff * attempts).await;
                    continue;
                }
                let _ = events.send(GatewayEvent::Closed { code, reason: reason.clone() });
                return Err(Error::Closed { code, reason });
            }
        }
    }
}

async fn connect_once(
    m: &mut GatewayMachine,
    url: &str,
    hello_timeout: Duration,
    cmds: &mut mpsc::UnboundedReceiver<Command>,
    events: &mpsc::UnboundedSender<GatewayEvent>,
) -> Result<Exit> {
    let hello_by = tokio::time::Instant::now() + hello_timeout;
    let no_hello = || Exit::Closed(None, format!("did not receive OP_HELLO within {} ms", hello_timeout.as_millis()));
    let Ok(open) = tokio::time::timeout_at(hello_by, tokio_tungstenite::connect_async(url)).await else {
        return Ok(no_hello());
    };
    let (mut ws, _) = open?;
    ws.send(Message::Text(m.on_open().into())).await?;
    let mut hb: Option<tokio::time::Interval> = None;
    let mut hello = false;
    loop {
        let tick = async {
            match hb.as_mut() {
                Some(i) => {
                    i.tick().await;
                }
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            msg = ws.next() => {
                let Some(msg) = msg else { return Ok(Exit::Closed(None, "stream ended".into())) };
                let parsed = match msg? {
                    Message::Text(t) => Some(m.on_text(t.as_str())),
                    Message::Binary(b) => Some(m.on_binary(&b)),
                    Message::Close(f) => {
                        let (code, reason) = f.map(|f| (Some(u16::from(f.code)), f.reason.to_string())).unwrap_or((None, String::new()));
                        return Ok(Exit::Closed(code, reason));
                    }
                    _ => None,
                };
                match parsed {
                    Some(Ok(msg)) => {
                        if let ServerMessage::Hello { .. } = msg {
                            hello = true;
                            if let Some(iv) = m.heartbeat_interval() {
                                let mut i = tokio::time::interval_at(tokio::time::Instant::now() + iv.mul_f32(0.5), iv);
                                i.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                                hb = Some(i);
                            }
                        }
                        let _ = events.send(GatewayEvent::Message(msg));
                    }
                    Some(Err(e)) => tracing::warn!(error = %e, "ignoring undecodable voice gateway frame"),
                    None => {}
                }
            }
            _ = tokio::time::sleep_until(hello_by), if !hello => {
                let _ = ws.close(None).await;
                return Ok(no_hello());
            }
            _ = tick => {
                let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                match m.heartbeat(nonce) {
                    Ok(s) => ws.send(Message::Text(s.into())).await?,
                    Err(_) => {
                        let _ = ws.close(None).await;
                        return Ok(Exit::Closed(None, "heartbeat timeout".into()));
                    }
                }
            }
            cmd = cmds.recv() => match cmd {
                Some(Command::Text(s)) => ws.send(Message::Text(s.into())).await?,
                Some(Command::Binary { opcode, payload }) => ws.send(Message::Binary(protocol::encode_binary(opcode, &payload).into())).await?,
                Some(Command::Close) | None => {
                    let _ = ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: CloseCode::Normal, reason: "".into() })).await;
                    return Ok(Exit::UserClose);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> VoiceConnectInfo {
        VoiceConnectInfo {
            endpoint: "x".into(),
            server_id: "1".into(),
            user_id: "2".into(),
            session_id: "s".into(),
            token: "t".into(),
            max_dave_protocol_version: 1,
            stream: false,
        }
    }

    #[test]
    fn machine_identify_then_resume() {
        let mut m = GatewayMachine::new(info());
        assert!(m.on_open().contains("\"op\":0"));
        m.on_text(r#"{"op":8,"d":{"heartbeat_interval":41250.5}}"#).unwrap();
        assert_eq!(m.heartbeat_interval(), Some(Duration::from_millis(41250)));
        m.on_text(r#"{"op":2,"seq":4,"d":{"ssrc":1,"ip":"1.1.1.1","port":1,"modes":[]}}"#).unwrap();
        assert_eq!(m.state(), State::Connected);
        m.on_binary(&[0, 5, 25, 1, 2]).unwrap();
        assert_eq!(m.seq_ack(), Some(5));
        let hb = m.heartbeat(10).unwrap();
        assert!(hb.contains("\"seq_ack\":5"));
        assert!(m.on_close(Some(4015)));
        let r = m.on_open();
        assert!(r.contains("\"op\":7") && r.contains("\"seq_ack\":5"));
        assert!(!m.on_close(Some(4006)));
        assert!(m.on_open().contains("\"op\":0"));
    }

    #[test]
    fn zombie_heartbeat_detected() {
        let mut m = GatewayMachine::new(info());
        m.heartbeat(1).unwrap();
        m.heartbeat(2).unwrap();
        assert!(m.heartbeat(3).is_err());
        let mut m = GatewayMachine::new(info());
        m.heartbeat(1).unwrap();
        m.on_text(r#"{"op":6,"d":{"t":1}}"#).unwrap();
        m.heartbeat(2).unwrap();
        m.on_text(r#"{"op":6,"d":{"t":2}}"#).unwrap();
        assert!(m.heartbeat(3).is_ok());
    }

    fn quick_hello(ms: u64) -> ReconnectPolicy {
        ReconnectPolicy { max_attempts: 0, backoff: Duration::from_millis(10), hello_timeout: Duration::from_millis(ms) }
    }

    async fn next(rx: &mut mpsc::UnboundedReceiver<GatewayEvent>) -> GatewayEvent {
        tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.expect("event timeout").expect("channel")
    }

    #[tokio::test]
    async fn a_server_that_never_says_hello_is_given_up() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (s, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(s).await.unwrap();
            let first = ws.next().await.unwrap().unwrap();
            assert!(first.to_text().unwrap().contains("\"op\":0"));
            while let Some(Ok(m)) = ws.next().await {
                if m.is_close() {
                    break;
                }
            }
        });
        let (_h, mut rx, task) = spawn(info(), Some(format!("ws://{addr}")), quick_hello(300));
        match next(&mut rx).await {
            GatewayEvent::Closed { code: None, reason } => assert!(reason.contains("OP_HELLO"), "{reason}"),
            e => panic!("unexpected {e:?}"),
        }
        let err = task.await.unwrap().unwrap_err();
        assert!(crate::connection::retryable(&err), "{err}");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn a_stuck_websocket_handshake_is_given_up() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hold = tokio::spawn(async move {
            let (s, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(10)).await;
            drop(s);
        });
        let (_h, mut rx, task) = spawn(info(), Some(format!("ws://{addr}")), quick_hello(300));
        assert!(matches!(next(&mut rx).await, GatewayEvent::Closed { code: None, .. }));
        assert!(crate::connection::retryable(&task.await.unwrap().unwrap_err()));
        hold.abort();
    }

    #[tokio::test]
    async fn a_hello_in_time_disarms_the_deadline() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (s, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(s).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(r#"{"op":8,"d":{"heartbeat_interval":5000}}"#.into())).await.unwrap();
            ws.send(Message::Text(r#"{"op":2,"seq":1,"d":{"ssrc":11,"ip":"127.0.0.1","port":4000,"modes":["aead_aes256_gcm_rtpsize"],"streams":[]}}"#.into())).await.unwrap();
            tokio::time::sleep(Duration::from_millis(600)).await;
            let _ = ws.close(None).await;
        });
        let (_h, mut rx, _task) = spawn(info(), Some(format!("ws://{addr}")), quick_hello(300));
        let mut ready = false;
        loop {
            match next(&mut rx).await {
                GatewayEvent::Message(ServerMessage::Ready(_)) => ready = true,
                GatewayEvent::Message(_) => {}
                GatewayEvent::Closed { reason, .. } => {
                    assert!(!reason.contains("OP_HELLO"), "{reason}");
                    break;
                }
                GatewayEvent::Reconnecting { .. } => break,
            }
        }
        assert!(ready);
        server.await.unwrap();
    }
}
