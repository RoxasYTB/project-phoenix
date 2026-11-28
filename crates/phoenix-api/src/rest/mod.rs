pub mod guard;
pub mod agent_scope;
pub mod form;
pub mod auth;
pub mod profile;
pub mod settings;
pub mod ratelimit;
pub mod search;
pub mod commands;
pub mod inbox;
pub mod moderation;
pub mod forum;
pub mod message_actions;
pub mod members;
pub mod guild_admin;
pub mod reporting;
pub mod guild_manage;
pub mod guild_server;
pub mod threads;
pub mod thread_create;
pub mod content_inventory;
pub mod collectibles;
pub mod frecency;
pub mod favorites;
pub mod soundboard;
pub mod voice_regions;
pub mod experiments;
pub mod stage;
pub mod streams;
pub mod uploads;
pub mod backoff;

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{ApiErrorBody, Error, Result};
use crate::models::*;
use crate::props::ClientProperties;
use ratelimit::{RateLimitInfo, RateLimiter, major_param, route_key};

pub const API_BASE: &str = "https://discord.com/api/v10";

#[derive(Debug, Clone, Default)]
pub struct FileUpload {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub description: Option<String>,
    pub spoiler: bool,
    pub duration_secs: Option<f64>,
    pub waveform: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MessageQuery {
    pub before: Option<MessageId>,
    pub after: Option<MessageId>,
    pub around: Option<MessageId>,
    pub limit: Option<u8>,
}

impl MessageQuery {
    pub fn latest(limit: u8) -> Self {
        Self { limit: Some(limit), ..Default::default() }
    }
    pub fn before(id: MessageId, limit: u8) -> Self {
        Self { before: Some(id), limit: Some(limit), ..Default::default() }
    }
    pub fn after(id: MessageId, limit: u8) -> Self {
        Self { after: Some(id), limit: Some(limit), ..Default::default() }
    }
    pub fn around(id: MessageId, limit: u8) -> Self {
        Self { around: Some(id), limit: Some(limit), ..Default::default() }
    }

    fn to_query(self) -> String {
        let mut q = vec![format!("limit={}", self.limit.unwrap_or(50).clamp(1, 100))];
        if let Some(b) = self.before {
            q.push(format!("before={b}"));
        } else if let Some(a) = self.after {
            q.push(format!("after={a}"));
        } else if let Some(a) = self.around {
            q.push(format!("around={a}"));
        }
        q.join("&")
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateMessage {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sticker_ids: Vec<Snowflake>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_nonce: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Value>,
}

impl CreateMessage {
    pub fn text(content: impl Into<String>) -> Self {
        Self { content: content.into(), ..Default::default() }
    }

    pub fn reply_to(mut self, channel: ChannelId, message: MessageId, mention: bool) -> Self {
        self.message_reference = Some(MessageReference {
            kind: None,
            message_id: Some(message),
            channel_id: Some(channel),
            guild_id: None,
            fail_if_not_exists: Some(false),
        });
        if !mention {
            self.allowed_mentions = Some(json!({"parse": ["users", "roles", "everyone"], "replied_user": false}));
        }
        self
    }
}

#[derive(Clone)]
pub enum LoginResponse {
    Token { user_id: Option<UserId>, token: String },
    MfaRequired {
        user_id: Option<UserId>,
        ticket: String,
        totp: bool,
        sms: bool,
        backup: bool,
        webauthn: bool,
        login_instance_id: Option<String>,
    },
}

impl std::fmt::Debug for LoginResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token { user_id, .. } => {
                f.debug_struct("Token").field("user_id", user_id).field("token", &"<redacted>").finish()
            }
            Self::MfaRequired { user_id, totp, sms, backup, webauthn, .. } => f
                .debug_struct("MfaRequired")
                .field("user_id", user_id)
                .field("ticket", &"<redacted>")
                .field("totp", totp)
                .field("sms", sms)
                .field("backup", backup)
                .field("webauthn", webauthn)
                .finish(),
        }
    }
}

#[derive(Deserialize)]
struct RawLogin {
    #[serde(default)]
    user_id: Option<UserId>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    mfa: bool,
    #[serde(default)]
    ticket: Option<String>,
    #[serde(default)]
    totp: bool,
    #[serde(default)]
    sms: bool,
    #[serde(default)]
    backup: bool,
    #[serde(default)]
    webauthn: Option<Value>,
    #[serde(default)]
    login_instance_id: Option<String>,
}

enum Body {
    None,
    Json(Value),
    Multipart { payload: Value, files: Arc<Vec<FileUpload>> },
    Form(form::FormBody),
}

struct Inner {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
    props: ClientProperties,
    super_props: [String; 2],
    limiter: RateLimiter,
    max_retries: u32,
    locale: parking_lot::RwLock<String>,
    timezone: Option<String>,
    notes: parking_lot::Mutex<std::collections::HashMap<u64, Arc<tokio::sync::OnceCell<String>>>>,
    proto_writer: settings::ProtoWriter,
    fingerprint: parking_lot::RwLock<Option<String>>,
}

#[derive(Clone, Default)]
pub struct CaptchaSolution {
    pub key: String,
    pub rqtoken: Option<String>,
    pub session_id: Option<String>,
}

impl std::fmt::Debug for CaptchaSolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CaptchaSolution(<redacted>)")
    }
}

impl CaptchaSolution {
    pub fn from_challenge(key: String, body: &ApiErrorBody) -> Self {
        Self { key, rqtoken: body.captcha_rqtoken.clone(), session_id: body.captcha_session_id.clone() }
    }

    pub fn headers(&self) -> Vec<(&'static str, String)> {
        let mut h = vec![("x-captcha-key", self.key.clone())];
        if let Some(t) = &self.rqtoken {
            h.push(("x-captcha-rqtoken", t.clone()));
        }
        if let Some(s) = &self.session_id {
            h.push(("x-captcha-session-id", s.clone()));
        }
        h
    }
}

#[derive(Clone)]
pub struct RestClient {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for RestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestClient").field("base", &self.inner.base).finish_non_exhaustive()
    }
}

impl RestClient {
    pub fn new(token: impl Into<String>, props: ClientProperties) -> Self {
        Self::build(Some(token.into()), props, API_BASE.to_owned())
    }

    pub fn unauthenticated(props: ClientProperties) -> Self {
        Self::build(None, props, API_BASE.to_owned())
    }

    pub fn with_base(token: Option<String>, props: ClientProperties, base: impl Into<String>) -> Self {
        Self::build(token, props, base.into())
    }

    fn build(token: Option<String>, props: ClientProperties, base: String) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(props.user_agent())
            .use_rustls_tls()
            .build()
            .expect("reqwest client");
        let super_props = [props.to_header_with("focused"), props.to_header_with("unfocused")];
        Self {
            inner: Arc::new(Inner { http, base, token, props, super_props, limiter: RateLimiter::new(), max_retries: 3,
                locale: parking_lot::RwLock::new(crate::props::os_discord_locale()),
                timezone: crate::props::os_timezone(),
                notes: Default::default(),
                proto_writer: Default::default(),
                fingerprint: Default::default(),
            }),
        }
    }

    pub fn token(&self) -> Option<&str> {
        self.inner.token.as_deref()
    }

    pub fn properties(&self) -> &ClientProperties {
        &self.inner.props
    }

    pub fn set_locale(&self, locale: &str) {
        if !locale.is_empty() {
            *self.inner.locale.write() = locale.to_owned();
        }
    }

    pub fn locale(&self) -> String {
        self.inner.locale.read().clone()
    }

    fn headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(t) = &self.inner.token
            && let Ok(v) = HeaderValue::from_str(t)
        {
            h.insert(reqwest::header::AUTHORIZATION, v);
        }
        let state = usize::from(crate::props::client_app_state() != "focused");
        if let Ok(v) = HeaderValue::from_str(&self.inner.super_props[state]) {
            h.insert("x-super-properties", v);
        }
        let locale = self.inner.locale.read().clone();
        if let Ok(v) = HeaderValue::from_str(&crate::props::accept_language(&locale, &crate::props::os_locale_tag()))
        {
            h.insert(reqwest::header::ACCEPT_LANGUAGE, v);
        }
        if let Ok(v) = HeaderValue::from_str(&locale) {
            h.insert("x-discord-locale", v);
        }
        if let Some(tz) = &self.inner.timezone
            && let Ok(v) = HeaderValue::from_str(tz)
        {
            h.insert("x-discord-timezone", v);
        }
        if let Some(fp) = self.inner.fingerprint.read().as_deref()
            && let Ok(v) = HeaderValue::from_str(fp)
        {
            h.insert("x-fingerprint", v);
        }
        h.insert("x-debug-options", HeaderValue::from_static("bugReporterEnabled"));
        h
    }

    async fn raw(&self, method: Method, path: &str, body: Body) -> Result<(StatusCode, Vec<u8>)> {
        self.raw_failed(method, path, body, 0).await
    }

    async fn raw_failed(&self, method: Method, path: &str, body: Body, failed: u32) -> Result<(StatusCode, Vec<u8>)> {
        self.raw_with(method, path, body, failed, &[]).await
    }

    async fn raw_with(&self, method: Method, path: &str, body: Body, failed: u32, extra: &[(&'static str, String)]) -> Result<(StatusCode, Vec<u8>)> {
        let sent = match &body {
            Body::Json(v) => Some(v),
            Body::Multipart { payload, .. } => Some(payload),
            Body::None | Body::Form(_) => None,
        };
        if let Some(err) = crate::trap::refusal(&method, path, sent) {
            tracing::warn!(%method, path, "trap channel: write refused");
            return Err(err);
        }
        let scoped = agent_scope::scoped(guard::agent_session(), &self.inner.base);
        if scoped && let Some(err) = agent_scope::refusal(&method, path, sent) {
            tracing::warn!(%method, path, "agent session: write refused");
            return Err(err);
        }
        let route = route_key(method.as_str(), path);
        let major = major_param(path).to_owned();
        let url = format!("{}{}", self.inner.base, path);
        let mut attempt = 0;
        let mut backoff = backoff::Backoff::default();
        loop {
            while let Some(wait) = self.inner.limiter.acquire(&route, &major, Instant::now()) {
                tracing::debug!(%route, ?wait, "rate limit: waiting");
                tokio::time::sleep(wait).await;
            }
            let mut req = self.inner.http.request(method.clone(), &url).headers(self.headers());
            let retried = failed + backoff.fails();
            if retried > 0 {
                req = req.header("x-failed-requests", retried.to_string());
            }
            for (name, value) in extra {
                req = req.header(*name, value.as_str());
            }
            req = match &body {
                Body::None => req,
                Body::Json(v) => req.json(v),
                Body::Multipart { payload, files } => req.multipart(build_form(payload, files)?),
                Body::Form(f) => {
                    let boundary = form::FormBody::boundary();
                    let mut r = req.header(reqwest::header::CONTENT_TYPE, form::FormBody::content_type(&boundary));
                    for (k, v) in &f.headers {
                        r = r.header(k.as_str(), v.as_str());
                    }
                    r.body(f.encode(&boundary))
                }
            };
            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    self.inner.limiter.release(&route, &major);
                    return Err(e.into());
                }
            };
            let status = resp.status();
            let info = RateLimitInfo::from_headers(resp.headers());
            let now = Instant::now();
            self.inner.limiter.update(&route, &major, &info, now);
            let bytes = resp.bytes().await?.to_vec();

            if status == StatusCode::TOO_MANY_REQUESTS {
                #[derive(Deserialize, Default)]
                struct Rl {
                    #[serde(default)]
                    retry_after: f64,
                    #[serde(default)]
                    global: bool,
                }
                let rl: Rl = serde_json::from_slice(&bytes).unwrap_or_default();
                let retry = Duration::from_secs_f64(rl.retry_after.clamp(0.0, 3600.0))
                    .max(info.reset_after.unwrap_or_default());
                let global = rl.global || info.global;
                self.inner.limiter.on_429(&route, &major, retry, global, now);
                if attempt >= self.inner.max_retries {
                    return Err(Error::RateLimited { retry_after: retry, global });
                }
                attempt += 1;
                tracing::warn!(%route, ?retry, global, "429, retrying");
                continue;
            }
            if backoff::retries_status(status.as_u16()) && attempt < self.inner.max_retries {
                attempt += 1;
                tokio::time::sleep(backoff.fail()).await;
                continue;
            }
            if !status.is_success() {
                let raw = String::from_utf8_lossy(&bytes).into_owned();
                let body: ApiErrorBody = serde_json::from_slice(&bytes).unwrap_or_default();
                if body.captcha_key.is_some() {
                    return Err(Error::CaptchaRequired { sitekey: body.captcha_sitekey.clone(), body: Box::new(body) });
                }
                return Err(Error::Api { status: status.as_u16(), body: Box::new(body), raw });
            }
            if scoped {
                agent_scope::observe(&method, path, &bytes);
            }
            return Ok((status, bytes));
        }
    }

    async fn call<T: DeserializeOwned>(&self, method: Method, path: &str, body: Body) -> Result<T> {
        let (_, bytes) = self.raw(method, path, body).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn call_empty(&self, method: Method, path: &str, body: Body) -> Result<()> {
        self.raw(method, path, body).await.map(|_| ())
    }

    pub async fn get_me(&self) -> Result<User> {
        self.call(Method::GET, "/users/@me", Body::None).await
    }

    pub async fn get_user(&self, id: UserId) -> Result<User> {
        self.call(Method::GET, &format!("/users/{id}"), Body::None).await
    }

    pub async fn get_guilds(&self) -> Result<Vec<PartialGuild>> {
        self.call(Method::GET, "/users/@me/guilds?with_counts=true", Body::None).await
    }

    pub async fn get_private_channels(&self) -> Result<Vec<Channel>> {
        self.call(Method::GET, "/users/@me/channels", Body::None).await
    }

    pub async fn create_dm(&self, recipient: UserId) -> Result<Channel> {
        self.call(Method::POST, "/users/@me/channels", Body::Json(json!({ "recipients": [recipient] })))
            .await
    }

    pub async fn create_group_dm(&self, recipients: &[UserId]) -> Result<Channel> {
        self.call(Method::POST, "/users/@me/channels", Body::Json(json!({ "recipients": recipients })))
            .await
    }

    pub async fn get_relationships(&self) -> Result<Vec<Relationship>> {
        self.call(Method::GET, "/users/@me/relationships", Body::None).await
    }

    pub async fn request_json(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let body = body.map(Body::Json).unwrap_or(Body::None);
        let (_, bytes) = self.raw(method, path, body).await?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn request_json_mfa(&self, method: Method, path: &str, body: Option<Value>, mfa_token: &str) -> Result<Value> {
        let body = body.map(Body::Json).unwrap_or(Body::None);
        let (_, bytes) = self.raw_with(method, path, body, 0, &[("x-discord-mfa-authorization", mfa_token.to_owned())]).await?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn mfa_finish(&self, ticket: &str, method: guild_manage::AdminMfaMethod, code: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct T {
            token: String,
        }
        let code: String = code.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
        let t: T = self.call(Method::POST, "/mfa/finish", Body::Json(json!({ "ticket": ticket, "mfa_type": method.api_name(), "data": code }))).await?;
        Ok(t.token)
    }

    pub async fn get_guild(&self, id: GuildId) -> Result<Guild> {
        self.call(Method::GET, &format!("/guilds/{id}"), Body::None).await
    }

    pub async fn get_guild_channels(&self, id: GuildId) -> Result<Vec<Channel>> {
        self.call(Method::GET, &format!("/guilds/{id}/channels"), Body::None).await
    }

    pub async fn get_guild_member(&self, guild: GuildId, user: UserId) -> Result<Member> {
        self.call(Method::GET, &format!("/guilds/{guild}/members/{user}"), Body::None).await
    }

    pub async fn get_channel(&self, id: ChannelId) -> Result<Channel> {
        self.call(Method::GET, &format!("/channels/{id}"), Body::None).await
    }

    pub async fn get_messages(&self, channel: ChannelId, q: MessageQuery) -> Result<Vec<Message>> {
        self.call(Method::GET, &format!("/channels/{channel}/messages?{}", q.to_query()), Body::None)
            .await
    }

    pub async fn get_pinned_messages(&self, channel: ChannelId) -> Result<Vec<Message>> {
        self.call(Method::GET, &format!("/channels/{channel}/pins"), Body::None).await
    }

    pub async fn send_message(&self, channel: ChannelId, msg: CreateMessage) -> Result<Message> {
        self.send_message_failed(channel, msg, 0).await
    }

    pub async fn send_message_failed(&self, channel: ChannelId, mut msg: CreateMessage, failed: u32) -> Result<Message> {
        fill_nonce(&mut msg);
        let body = serde_json::to_value(&msg)?;
        let (_, bytes) = self.raw_failed(Method::POST, &format!("/channels/{channel}/messages"), Body::Json(body), failed).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn send_message_with_files(
        &self,
        channel: ChannelId,
        mut msg: CreateMessage,
        files: Vec<FileUpload>,
    ) -> Result<Message> {
        fill_nonce(&mut msg);
        let mut payload = serde_json::to_value(&msg)?;
        let attachments: Vec<Value> = files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut a = json!({ "id": i, "filename": upload_name(f) });
                if let Some(d) = &f.description {
                    a["description"] = json!(d);
                }
                a
            })
            .collect();
        payload["attachments"] = Value::Array(attachments);
        self.call(
            Method::POST,
            &format!("/channels/{channel}/messages"),
            Body::Multipart { payload, files: Arc::new(files) },
        )
        .await
    }

    pub async fn edit_message(&self, channel: ChannelId, message: MessageId, content: &str) -> Result<Message> {
        self.call(
            Method::PATCH,
            &format!("/channels/{channel}/messages/{message}"),
            Body::Json(json!({ "content": content })),
        )
        .await
    }

    pub async fn delete_message(&self, channel: ChannelId, message: MessageId) -> Result<()> {
        self.call_empty(Method::DELETE, &format!("/channels/{channel}/messages/{message}"), Body::None)
            .await
    }

    pub async fn ack_message(&self, channel: ChannelId, message: MessageId) -> Result<()> {
        self.call_empty(
            Method::POST,
            &format!("/channels/{channel}/messages/{message}/ack"),
            Body::Json(json!({ "token": null })),
        )
        .await
    }

    pub async fn ack_message_manual(&self, channel: ChannelId, message: MessageId, mention_count: u32) -> Result<()> {
        self.call_empty(
            Method::POST,
            &format!("/channels/{channel}/messages/{message}/ack"),
            Body::Json(json!({ "manual": true, "mention_count": mention_count })),
        )
        .await
    }

    pub async fn pin_message(&self, channel: ChannelId, message: MessageId) -> Result<()> {
        self.call_empty(Method::PUT, &format!("/channels/{channel}/pins/{message}"), Body::None).await
    }

    pub async fn unpin_message(&self, channel: ChannelId, message: MessageId) -> Result<()> {
        self.call_empty(Method::DELETE, &format!("/channels/{channel}/pins/{message}"), Body::None).await
    }

    pub async fn trigger_typing(&self, channel: ChannelId) -> Result<()> {
        self.call_empty(Method::POST, &format!("/channels/{channel}/typing"), Body::None).await
    }

    fn reaction_path(channel: ChannelId, message: MessageId, emoji: &ReactionEmoji) -> String {
        let e: String = url::form_urlencoded::byte_serialize(emoji.route_form().as_bytes()).collect();
        format!("/channels/{channel}/messages/{message}/reactions/{e}")
    }

    pub async fn add_reaction(&self, channel: ChannelId, message: MessageId, emoji: &ReactionEmoji) -> Result<()> {
        let p = format!("{}/@me", Self::reaction_path(channel, message, emoji));
        self.call_empty(Method::PUT, &p, Body::None).await
    }

    pub async fn remove_own_reaction(&self, channel: ChannelId, message: MessageId, emoji: &ReactionEmoji) -> Result<()> {
        let p = format!("{}/@me", Self::reaction_path(channel, message, emoji));
        self.call_empty(Method::DELETE, &p, Body::None).await
    }

    pub async fn remove_user_reaction(
        &self,
        channel: ChannelId,
        message: MessageId,
        emoji: &ReactionEmoji,
        user: UserId,
    ) -> Result<()> {
        let p = format!("{}/{user}", Self::reaction_path(channel, message, emoji));
        self.call_empty(Method::DELETE, &p, Body::None).await
    }

    pub async fn get_reactions(
        &self,
        channel: ChannelId,
        message: MessageId,
        emoji: &ReactionEmoji,
        limit: u8,
        burst: bool,
    ) -> Result<Vec<User>> {
        let p = format!("{}?limit={}&type={}", Self::reaction_path(channel, message, emoji), limit.clamp(1, 100), u8::from(burst));
        self.call(Method::GET, &p, Body::None).await
    }

    pub async fn get_gateway_url(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct G {
            url: String,
        }
        let g: G = self.call(Method::GET, "/gateway", Body::None).await?;
        Ok(g.url)
    }

    pub async fn login(&self, login: &str, password: &str) -> Result<LoginResponse> {
        self.login_with(login, password, None).await
    }

    pub async fn login_with(&self, login: &str, password: &str, captcha: Option<&CaptchaSolution>) -> Result<LoginResponse> {
        let body = Body::Json(json!({
            "login": login,
            "password": password,
            "undelete": false,
            "login_source": null,
            "gift_code_sku_id": null,
        }));
        let headers = captcha.map(CaptchaSolution::headers).unwrap_or_default();
        let (_, bytes) = self.raw_with(Method::POST, "/auth/login", body, 0, &headers).await?;
        parse_login(serde_json::from_slice(&bytes)?)
    }

    pub fn fingerprint(&self) -> Option<String> {
        self.inner.fingerprint.read().clone()
    }

    pub fn set_fingerprint(&self, fp: Option<String>) {
        *self.inner.fingerprint.write() = fp.filter(|f| !f.is_empty());
    }

    pub async fn ensure_fingerprint(&self) -> Option<String> {
        if self.inner.token.is_some() {
            return None;
        }
        if let Some(fp) = self.fingerprint() {
            return Some(fp);
        }
        #[derive(Deserialize)]
        struct E {
            #[serde(default)]
            fingerprint: Option<String>,
        }
        let e: E = self.call(Method::GET, "/experiments", Body::None).await.ok()?;
        self.set_fingerprint(e.fingerprint);
        self.fingerprint()
    }
}

fn parse_login(raw: RawLogin) -> Result<LoginResponse> {
    if let Some(token) = raw.token {
        return Ok(LoginResponse::Token { user_id: raw.user_id, token });
    }
    if raw.mfa
        && let Some(ticket) = raw.ticket
    {
        return Ok(LoginResponse::MfaRequired {
            user_id: raw.user_id,
            ticket,
            totp: raw.totp,
            sms: raw.sms,
            backup: raw.backup,
            webauthn: raw.webauthn.is_some_and(|v| !v.is_null()),
            login_instance_id: raw.login_instance_id,
        });
    }
    Err(Error::Protocol("login response had neither token nor MFA ticket".into()))
}

fn fill_nonce(msg: &mut CreateMessage) {
    if msg.nonce.is_none() {
        msg.nonce = Some(Snowflake::now().to_string());
        msg.enforce_nonce.get_or_insert(true);
    }
}

fn upload_name(f: &FileUpload) -> String {
    if f.spoiler && !f.filename.starts_with("SPOILER_") {
        format!("SPOILER_{}", f.filename)
    } else {
        f.filename.clone()
    }
}

fn build_form(payload: &Value, files: &[FileUpload]) -> Result<reqwest::multipart::Form> {
    let mut form = reqwest::multipart::Form::new().part(
        "payload_json",
        reqwest::multipart::Part::text(payload.to_string()).mime_str("application/json")?,
    );
    for (i, f) in files.iter().enumerate() {
        let mut part = reqwest::multipart::Part::bytes(f.bytes.clone()).file_name(upload_name(f));
        if let Some(ct) = &f.content_type {
            part = part.mime_str(ct)?;
        }
        form = form.part(format!("files[{i}]"), part);
    }
    Ok(form)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_query() {
        assert_eq!(MessageQuery::latest(50).to_query(), "limit=50");
        assert_eq!(MessageQuery::before(Snowflake(5), 200).to_query(), "limit=100&before=5");
    }

    #[test]
    fn login_shapes() {
        let t: RawLogin = serde_json::from_str(r#"{"user_id":"1","token":"abc","user_settings":{}}"#).unwrap();
        assert!(matches!(parse_login(t).unwrap(), LoginResponse::Token { token, .. } if token == "abc"));
        let m: RawLogin = serde_json::from_str(
            r#"{"user_id":"1","mfa":true,"sms":false,"ticket":"tkt","backup":true,"totp":true,"webauthn":null}"#,
        )
        .unwrap();
        match parse_login(m).unwrap() {
            LoginResponse::MfaRequired { ticket, totp, webauthn, .. } => {
                assert_eq!(ticket, "tkt");
                assert!(totp);
                assert!(!webauthn);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn nonce_and_reply_serialization() {
        let mut m = CreateMessage::text("hi").reply_to(Snowflake(1), Snowflake(2), false);
        fill_nonce(&mut m);
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["content"], "hi");
        assert_eq!(v["message_reference"]["message_id"], "2");
        assert_eq!(v["allowed_mentions"]["replied_user"], false);
        assert!(v["nonce"].as_str().unwrap().parse::<u64>().is_ok());
        assert!(v.get("tts").is_none());
    }
}
