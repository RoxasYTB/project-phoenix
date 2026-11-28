use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use futures_util::{SinkExt, StreamExt};
use rsa::pkcs8::EncodePublicKey;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{Message as WsMessage, client::IntoClientRequest};

use crate::error::{Error, Result};
use crate::rest::RestClient;

pub const GATEWAY_URL: &str = "wss://remote-auth-gateway.discord.gg/?v=2";

#[derive(Clone, PartialEq)]
pub enum RemoteAuthEvent {
    Qr(String),
    Scanned { user_id: String, discriminator: String, avatar: Option<String>, username: String },
    Token(String),
    Cancelled,
}

impl std::fmt::Debug for RemoteAuthEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Qr(u) => f.debug_tuple("Qr").field(u).finish(),
            Self::Scanned { username, .. } => f.debug_struct("Scanned").field("username", username).finish(),
            Self::Token(_) => f.write_str("Token(<redacted>)"),
            Self::Cancelled => f.write_str("Cancelled"),
        }
    }
}

pub struct Keys {
    private: RsaPrivateKey,
    spki_b64: String,
}

impl Keys {
    pub fn generate() -> Result<Self> {
        Self::generate_bits(2048)
    }

    pub fn generate_bits(bits: usize) -> Result<Self> {
        let mut rng = rsa::rand_core::OsRng;
        let private = RsaPrivateKey::new(&mut rng, bits).map_err(|e| Error::Protocol(format!("rsa: {e}")))?;
        let der = RsaPublicKey::from(&private)
            .to_public_key_der()
            .map_err(|e| Error::Protocol(format!("spki: {e}")))?;
        Ok(Self { private, spki_b64: STANDARD.encode(der.as_bytes()) })
    }

    pub fn public_key_b64(&self) -> &str {
        &self.spki_b64
    }

    pub fn public_key(&self) -> RsaPublicKey {
        RsaPublicKey::from(&self.private)
    }

    pub fn decrypt(&self, b64: &str) -> Result<Vec<u8>> {
        let data = STANDARD.decode(b64).map_err(|e| Error::Protocol(format!("base64: {e}")))?;
        self.private.decrypt(Oaep::new::<Sha256>(), &data).map_err(|e| Error::Protocol(format!("rsa decrypt: {e}")))
    }

    pub fn nonce_proof(&self, encrypted_nonce: &str) -> Result<String> {
        Ok(proof_of(&self.decrypt(encrypted_nonce)?))
    }

    pub fn expected_fingerprint(&self) -> String {
        let der = STANDARD.decode(&self.spki_b64).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(Sha256::digest(der))
    }
}

pub fn proof_of(nonce: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(nonce))
}

pub fn qr_url(fingerprint: &str) -> String {
    format!("https://discord.com/ra/{fingerprint}")
}

pub fn parse_user_payload(s: &str) -> Option<RemoteAuthEvent> {
    let mut it = s.splitn(4, ':');
    let user_id = it.next()?.to_owned();
    let discriminator = it.next()?.to_owned();
    let avatar = it.next()?;
    let username = it.next()?.to_owned();
    Some(RemoteAuthEvent::Scanned {
        user_id,
        discriminator,
        avatar: (!avatar.is_empty() && avatar != "0").then(|| avatar.to_owned()),
        username,
    })
}

#[derive(Debug, PartialEq)]
pub enum Step {
    Send(Value),
    Emit(RemoteAuthEvent),
    Ticket(String),
    Heartbeat(Duration),
    Ignore,
}

pub fn handle(keys: &Keys, msg: &Value) -> Result<Step> {
    let op = msg.get("op").and_then(Value::as_str).unwrap_or("");
    let s = |k: &str| msg.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    Ok(match op {
        "hello" => {
            let iv = msg.get("heartbeat_interval").and_then(Value::as_u64).unwrap_or(41_250);
            return Ok(Step::Heartbeat(Duration::from_millis(iv)));
        }
        "nonce_proof" => Step::Send(json!({"op": "nonce_proof", "proof": keys.nonce_proof(&s("encrypted_nonce"))?})),
        "pending_remote_init" => {
            let fp = s("fingerprint");
            if fp != keys.expected_fingerprint() {
                return Err(Error::Protocol("remote auth fingerprint mismatch".into()));
            }
            Step::Emit(RemoteAuthEvent::Qr(qr_url(&fp)))
        }
        "pending_ticket" => {
            let raw = keys.decrypt(&s("encrypted_user_payload"))?;
            match parse_user_payload(&String::from_utf8_lossy(&raw)) {
                Some(e) => Step::Emit(e),
                None => Step::Ignore,
            }
        }
        "pending_login" => Step::Ticket(s("ticket")),
        "cancel" => Step::Emit(RemoteAuthEvent::Cancelled),
        _ => Step::Ignore,
    })
}

pub async fn run(rest: RestClient, tx: mpsc::UnboundedSender<RemoteAuthEvent>, solver: Option<crate::captcha::Solver>) -> Result<()> {
    let keys = tokio::task::spawn_blocking(Keys::generate)
        .await
        .map_err(|e| Error::Protocol(e.to_string()))??;
    let mut req = GATEWAY_URL.into_client_request().map_err(|e| Error::WebSocket(Box::new(e)))?;
    req.headers_mut().insert("Origin", "https://discord.com".parse().expect("static header"));
    if let Ok(ua) = rest.properties().user_agent().parse() {
        req.headers_mut().insert("User-Agent", ua);
    }
    let (ws, _) = tokio_tungstenite::connect_async(req).await.map_err(|e| Error::WebSocket(Box::new(e)))?;
    let (mut sink, mut stream) = ws.split();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(3600));
    heartbeat.tick().await;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                sink.send(WsMessage::text(json!({"op": "heartbeat"}).to_string())).await.map_err(|e| Error::WebSocket(Box::new(e)))?;
            }
            _ = tx.closed() => {
                let _ = sink.close().await;
                return Ok(());
            }
            msg = stream.next() => {
                let Some(msg) = msg else {
                    let _ = tx.send(RemoteAuthEvent::Cancelled);
                    return Ok(());
                };
                let msg = msg.map_err(|e| Error::WebSocket(Box::new(e)))?;
                let text = match msg {
                    WsMessage::Text(t) => t.to_string(),
                    WsMessage::Close(_) => {
                        let _ = tx.send(RemoteAuthEvent::Cancelled);
                        return Ok(());
                    }
                    _ => continue,
                };
                let v: Value = serde_json::from_str(&text)?;
                match handle(&keys, &v)? {
                    Step::Heartbeat(iv) => {
                        heartbeat = tokio::time::interval(iv);
                        heartbeat.tick().await;
                        let init = json!({"op": "init", "encoded_public_key": keys.public_key_b64()});
                        sink.send(WsMessage::text(init.to_string())).await.map_err(|e| Error::WebSocket(Box::new(e)))?;
                    }
                    Step::Send(v) => {
                        sink.send(WsMessage::text(v.to_string())).await.map_err(|e| Error::WebSocket(Box::new(e)))?;
                    }
                    Step::Emit(e) => {
                        let done = e == RemoteAuthEvent::Cancelled;
                        let _ = tx.send(e);
                        if done {
                            return Ok(());
                        }
                    }
                    Step::Ticket(t) => {
                        let enc = login_ticket(&rest, &t, solver.as_ref()).await?;
                        let token = String::from_utf8(keys.decrypt(&enc)?)
                            .map_err(|_| Error::Protocol("token is not utf-8".into()))?;
                        let _ = tx.send(RemoteAuthEvent::Token(token));
                        let _ = sink.close().await;
                        return Ok(());
                    }
                    Step::Ignore => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(keys: &Keys, data: &[u8]) -> String {
        let mut rng = rsa::rand_core::OsRng;
        STANDARD.encode(keys.public_key().encrypt(&mut rng, Oaep::new::<Sha256>(), data).unwrap())
    }

    #[test]
    fn full_handshake() {
        let keys = Keys::generate_bits(1024).unwrap();
        assert!(matches!(
            handle(&keys, &json!({"op": "hello", "heartbeat_interval": 1000})).unwrap(),
            Step::Heartbeat(d) if d == Duration::from_secs(1)
        ));
        let nonce = b"hello-nonce";
        let step = handle(&keys, &json!({"op": "nonce_proof", "encrypted_nonce": enc(&keys, nonce)})).unwrap();
        assert_eq!(step, Step::Send(json!({"op": "nonce_proof", "proof": proof_of(nonce)})));

        let fp = keys.expected_fingerprint();
        let step = handle(&keys, &json!({"op": "pending_remote_init", "fingerprint": fp})).unwrap();
        assert_eq!(step, Step::Emit(RemoteAuthEvent::Qr(format!("https://discord.com/ra/{fp}"))));
        assert!(handle(&keys, &json!({"op": "pending_remote_init", "fingerprint": "bad"})).is_err());

        let payload = enc(&keys, b"123:0:abcdef:someone");
        let step = handle(&keys, &json!({"op": "pending_ticket", "encrypted_user_payload": payload})).unwrap();
        assert!(matches!(step, Step::Emit(RemoteAuthEvent::Scanned { ref username, ref avatar, .. })
            if username == "someone" && avatar.as_deref() == Some("abcdef")));

        assert_eq!(handle(&keys, &json!({"op": "pending_login", "ticket": "t"})).unwrap(), Step::Ticket("t".into()));
        assert_eq!(handle(&keys, &json!({"op": "cancel"})).unwrap(), Step::Emit(RemoteAuthEvent::Cancelled));
        assert_eq!(keys.decrypt(&enc(&keys, b"tok")).unwrap(), b"tok");
    }

    #[test]
    fn user_payload_with_colons_in_name() {
        let e = parse_user_payload("1:0:0:a:b").unwrap();
        assert!(matches!(e, RemoteAuthEvent::Scanned { username, avatar: None, .. } if username == "a:b"));
    }

    #[test]
    fn proof_is_unpadded_base64url() {
        let p = proof_of(b"x");
        assert!(!p.contains('=') && !p.contains('+') && !p.contains('/'));
    }
}

pub async fn login_ticket(rest: &RestClient, ticket: &str, solver: Option<&crate::captcha::Solver>) -> Result<String> {
    let mut solution = None;
    for _ in 0..3 {
        match rest.remote_auth_login(ticket, solution.as_ref()).await {
            Err(Error::CaptchaRequired { body, sitekey }) => {
                let Some(solve) = solver else { return Err(Error::CaptchaRequired { body, sitekey }) };
                match solve((*body).clone()).await {
                    Some(s) => solution = Some(s),
                    None => return Err(Error::CaptchaRequired { body, sitekey }),
                }
            }
            other => return other,
        }
    }
    rest.remote_auth_login(ticket, solution.as_ref()).await
}
