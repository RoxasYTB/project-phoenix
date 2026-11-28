pub mod convert;
mod account;
mod expressions;
pub mod http;
mod nav;
mod people;
mod profile_edit;
mod profile_widgets;
pub(crate) mod profile_games;
mod collectibles_shop;
mod thread_members;
mod pins;
mod member_list_id;
mod history;
mod inbox;
mod msg_cache;
mod snapshot;
mod preupload;
mod self_presence;
mod qos_store;
mod network;
mod subscriptions;
#[cfg(test)]
pub(crate) use profile_edit::bodies as profile_edit_bodies;
pub mod store;

use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use phoenix_api::{
    Cache, ClientProperties, Error as ApiError, Gateway, GatewayConfig, GatewayEvent, GatewayHandle, RestClient,
    models as api,
    rest::{CreateMessage, LoginResponse},
};
use phoenix_api::gateway::events as api_ready;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    backend::{AppEvent, Backend, BackendError, BoxFuture, Credentials, EventSink, ReadyData},
    model::{Id, Message},
};
use convert::{Adapter, Prefs, sf};
use store::{KeyringStore, Secret, TokenStore};

const READY_TIMEOUT: Duration = Duration::from_secs(45);
const TYPING_INTERVAL: Duration = Duration::from_secs(8);
const ACK_DEBOUNCE: Duration = Duration::from_millis(1500);

fn err(s: impl Into<String>) -> BackendError {
    BackendError(s.into())
}

const SEND_RETRIES: u32 = 3;

fn first_field_error(v: &serde_json::Value) -> Option<String> {
    let obj = v.as_object()?;
    if let Some(msg) = obj.get("_errors").and_then(|e| e.as_array()).and_then(|a| a.iter().find_map(|e| e.get("message")?.as_str())) {
        return Some(msg.to_owned());
    }
    obj.values().find_map(first_field_error)
}

fn api_err(e: ApiError) -> BackendError {
    match &e {
        ApiError::CaptchaRequired { .. } => err(crate::t!("login.captcha_failed").to_string()),
        ApiError::Api { body, .. } if body.code == 50035 => err(
            body.errors.as_ref().and_then(first_field_error).filter(|m| !m.is_empty()).unwrap_or_else(|| {
                if body.message.is_empty() { crate::t!("login.invalid").to_string() } else { body.message.clone() }
            }),
        ),
        ApiError::Api { body, .. } if e.is_unauthorized() => {
            err(if body.message.is_empty() { crate::t!("login.invalid").to_string() } else { body.message.clone() })
        }
        _ if e.is_unauthorized() => err("Invalid token."),
        ApiError::RateLimited { retry_after, .. } => {
            err(crate::t!("login.rate_limited", secs = retry_after.as_secs().max(1)).to_string())
        }
        _ => err(e.to_string()),
    }
}

fn login_err(e: ApiError) -> BackendError {
    if let ApiError::Api { body, .. } = &e
        && body.code == 50035
    {
        let login = body.field_error("login").or_else(|| body.field_error("email"));
        let password = body.field_error("password");
        if login.is_some() || password.is_some() {
            return BackendError::fields(login, password);
        }
    }
    if matches!(e, ApiError::Http(_)) {
        return err(crate::t!("login.network_error").to_string());
    }
    api_err(e)
}

struct Session {
    rest: RestClient,
    gateway: GatewayHandle,
    cache: Cache,
    adapter: Arc<Mutex<Adapter>>,
    selected_channel: Mutex<Option<api::Snowflake>>,
    pending_acks: Mutex<HashSet<u64>>,
    subs: subscriptions::Subscriptions,
    last_typing: Mutex<Option<(u64, Instant)>>,
    notify: Mutex<crate::notifications::NotifyPrefs>,
    new_notifications: bool,
    presence: Mutex<self_presence::Local>,
}

struct Inner {
    rt: tokio::runtime::Runtime,
    sink: Mutex<Option<EventSink>>,
    session: Mutex<Option<Arc<Session>>>,
    mfa_ticket: Mutex<Option<Secret>>,
    mfa_instance: Mutex<Option<String>>,
    mfa_opts: Mutex<crate::auth::MfaOptions>,
    remote_auth: Mutex<Option<tokio::task::AbortHandle>>,
    pre_rest: Mutex<Option<RestClient>>,
    pre_session_pages: Mutex<Vec<Vec<api::Message>>>,
    store: Arc<dyn TokenStore>,
    props: ClientProperties,
}

impl Inner {
    fn emit(&self, ev: AppEvent) {
        if let Some(s) = self.sink.lock().as_ref() {
            s(ev);
        }
    }
    fn session(&self) -> Result<Arc<Session>, BackendError> {
        self.session.lock().clone().ok_or_else(|| err("Not logged in."))
    }
    fn note_pre_session(&self, msgs: &[api::Message]) {
        let mut pages = self.pre_session_pages.lock();
        match self.session() {
            Ok(s) => {
                drop(pages);
                ensure_message_authors(self, &s, msgs);
            }
            Err(_) => pages.push(msgs.to_vec()),
        }
    }
}

#[derive(Clone)]
pub struct DiscordBackend {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for DiscordBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordBackend").field("logged_in", &self.inner.session.lock().is_some()).finish_non_exhaustive()
    }
}

impl DiscordBackend {
    pub fn new() -> Self {
        Self::with_store(Arc::new(KeyringStore))
    }

    pub fn with_store(store: Arc<dyn TokenStore>) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("phoenix-net")
            .enable_all()
            .build()
            .expect("tokio runtime");
        let _ = std::thread::Builder::new().name("phoenix-buildinfo".into()).spawn(|| {
            phoenix_api::BuildInfo::refresh_cache();
        });
        snapshot::preload();
        Self {
            inner: Arc::new(Inner {
                rt,
                sink: Mutex::new(None),
                session: Mutex::new(None),
                mfa_ticket: Mutex::new(None),
                mfa_instance: Mutex::new(None),
                mfa_opts: Mutex::new(Default::default()),
                remote_auth: Mutex::new(None),
                pre_rest: Mutex::new(None),
                pre_session_pages: Mutex::new(Vec::new()),
                store,
                props: ClientProperties::current(),
            }),
        }
    }

    pub fn http_client(&self) -> Arc<dyn gpui::http_client::HttpClient> {
        Arc::new(http::CdnHttpClient::new(self.inner.rt.handle().clone(), self.inner.props.user_agent()))
    }

    fn spawn<T, F>(&self, f: impl FnOnce(Arc<Inner>) -> F) -> BoxFuture<T>
    where
        T: Send + 'static,
        F: std::future::Future<Output = Result<T, BackendError>> + Send + 'static,
    {
        let handle = self.inner.rt.spawn(f(self.inner.clone()));
        Box::pin(async move { handle.await.map_err(|e| err(format!("internal error: {e}")))? })
    }

    fn with_session<T, F>(&self, f: impl FnOnce(Arc<Inner>, Arc<Session>) -> F + Send + 'static) -> BoxFuture<T>
    where
        T: Send + 'static,
        F: std::future::Future<Output = Result<T, BackendError>> + Send + 'static,
    {
        self.spawn(move |inner| async move {
            let s = inner.session()?;
            f(inner, s).await
        })
    }
}

impl Default for DiscordBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn clean_token(t: &str) -> String {
    t.trim().trim_matches('"').trim().to_owned()
}

async fn store_op<T: Send + 'static>(
    store: &Arc<dyn TokenStore>,
    f: impl FnOnce(&dyn TokenStore) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let s = store.clone();
    tokio::task::spawn_blocking(move || f(&*s)).await.map_err(|e| e.to_string())?
}

async fn connect(inner: Arc<Inner>, token: Secret, remember: bool) -> Result<ReadyData, BackendError> {
    if let Some(old) = inner.session.lock().take() {
        old.gateway.shutdown();
    }
    let t0 = Instant::now();
    crate::perf::mark("connect_start");
    let rest = RestClient::new(token.expose(), inner.props.clone());
    let mut cfg = GatewayConfig::new(token.expose());
    cfg.properties = inner.props.clone();
    cfg.raw_dispatch = Some(snapshot::hook());
    cfg.derived_qos = qos_store::load(token.expose());
    let (foregrounded, rtc_connected) = activity();
    cfg.qos = phoenix_api::gateway::QosPayload::new(foregrounded, rtc_connected);
    let core = if remember { None } else { tokio::task::spawn_blocking(snapshot::take_core).await.ok().flatten() };
    if let Some(c) = &core {
        cfg.resume = c.resume.clone();
        crate::perf::mark("gateway_resume_attempt");
    }
    let (gateway, mut rx) = Gateway::spawn(cfg);
    if let Err(e) = rest.get_me().await {
        gateway.shutdown();
        return Err(api_err(e));
    }
    crate::perf::span("rest_get_me", t0);
    if let Some(snapshot::Core { cache, ready, adapter, .. }) = core {
        let data = convert::build_ready(&cache, &ready, &adapter.prefs);
        crate::perf::span("resume_session_built", t0);
        return Ok(start_session(&inner, rest, gateway, cache, &ready, adapter, rx, data));
    }
    let cache = Cache::default();

    let ready = tokio::time::timeout(READY_TIMEOUT, async {
        while let Some(ev) = rx.recv().await {
            cache.apply(&ev);
            match ev {
                GatewayEvent::Ready(r) => {
                    crate::perf::span("gateway_ready", t0);
                    return Ok(r);
                }
                GatewayEvent::Fatal { code, reason } => return Err(err(convert::fatal_message(code, &reason))),
                _ => {}
            }
        }
        Err(err("Gateway closed before READY."))
    })
    .await
    .map_err(|_| err("Timed out waiting for Discord."))
    .flatten();
    let ready = match ready {
        Ok(r) => r,
        Err(e) => {
            gateway.shutdown();
            return Err(e);
        }
    };

    if remember && let Err(e) = store_op(&inner.store, move |s| s.save(&token)).await {
        tracing_warn(&format!("could not save token to the keyring: {e}"));
    }

    qos_store::save(ready.user.id.0, ready.qos_token.as_deref());
    let prefs = Prefs::from_ready(&ready);
    let data = convert::build_ready(&cache, &ready, &prefs);
    crate::perf::span("ready_built", t0);
    let adapter = Adapter::new(ready.user.id, prefs);
    let data = start_session(&inner, rest, gateway, cache, &ready, adapter, rx, data);
    if let Some(s) = inner.session.lock().clone() {
        self_presence::resend(&s);
    }
    Ok(data)
}

#[allow(clippy::too_many_arguments)]
fn start_session(
    inner: &Arc<Inner>,
    rest: RestClient,
    gateway: GatewayHandle,
    cache: Cache,
    ready: &api_ready::Ready,
    adapter: Adapter,
    rx: UnboundedReceiver<GatewayEvent>,
    data: ReadyData,
) -> ReadyData {
    crate::commands::set_session_id(&ready.session_id);
    let adapter = Arc::new(Mutex::new(adapter));
    let session = Arc::new(Session {
        rest,
        gateway,
        cache,
        adapter,
        selected_channel: Mutex::new(None),
        pending_acks: Mutex::new(HashSet::new()),
        subs: Default::default(),
        last_typing: Mutex::new(None),
        notify: Mutex::new(crate::notifications::NotifyPrefs::from_ready_with_defaults(
            ready.user_guild_settings.as_ref(),
            ready.guilds.iter().map(|g| (g.id.0, crate::notifications::Level::from_default(g.properties.default_message_notifications))).collect(),
        )),
        new_notifications: inbox::new_notifications(ready.notification_settings.as_ref()),
        presence: Mutex::new(self_presence::Local::from_base64(ready.user_settings_proto.as_deref())),
    });
    pins::init(&session.cache);
    phoenix_api::rest::experiments::set_apex(ready.apex_experiments.as_ref());
    *inner.session.lock() = Some(session.clone());
    let afk_timeout = session.presence.lock().afk_timeout;
    self_presence::watch_idle(inner, afk_timeout);
    network::watch(inner);
    for page in std::mem::take(&mut *inner.pre_session_pages.lock()) {
        ensure_message_authors(inner, &session, &page);
    }
    if let Some(e) = convert::self_activities(ready) {
        inner.emit(e);
    }
    if let Some(e) = convert::self_status(ready) {
        inner.emit(e);
    }
    nav::snapshot(&session.cache, ready).into_iter().for_each(|e| inner.emit(e));
    for g in &ready.guilds {
        convert::voice_flag_events(&session.cache, Some(g.id)).into_iter().for_each(|e| inner.emit(e));
    }
    history::warm_recent(inner, &session);
    tokio::spawn(pump(inner.clone(), session, rx));
    data
}

fn ensure_authors(inner: &Inner, s: &Session, guild: api::Snowflake, users: Vec<api::Snowflake>) {
    if users.is_empty() {
        return;
    }
    let (events, missing) = s.adapter.lock().note_authors(&s.cache, guild, users);
    for e in events {
        inner.emit(e);
    }
    for ids in missing.chunks(100) {
        let _ = s.gateway.request_guild_members(phoenix_api::gateway::RequestGuildMembers {
            guild_id: vec![guild],
            user_ids: Some(ids.to_vec()),
            presences: true,
            ..Default::default()
        });
    }
}

fn ensure_message_authors(inner: &Inner, s: &Session, msgs: &[api::Message]) {
    let mut by_guild: Vec<(api::Snowflake, Vec<api::Snowflake>)> = Vec::new();
    for m in msgs {
        let Some(g) = m.guild_id.or_else(|| s.cache.channel(m.channel_id).and_then(|c| c.guild_id)) else { continue };
        let at = match by_guild.iter().position(|(x, _)| *x == g) {
            Some(i) => i,
            None => {
                by_guild.push((g, Vec::new()));
                by_guild.len() - 1
            }
        };
        convert::message_authors(m, &mut by_guild[at].1);
    }
    for (g, ids) in by_guild {
        ensure_authors(inner, s, g, ids);
    }
}

fn tracing_warn(msg: &str) {
    tracing::warn!("{msg}");
}

async fn pump(inner: Arc<Inner>, s: Arc<Session>, mut rx: UnboundedReceiver<GatewayEvent>) {
    while let Some(ev) = rx.recv().await {
        if !inner.session.lock().as_ref().is_some_and(|cur| Arc::ptr_eq(cur, &s)) {
            return;
        }
        s.cache.apply(&ev);
        msg_cache::on_gateway(&ev);
        if matches!(
            ev,
            GatewayEvent::VoiceStateUpdate(_)
                | GatewayEvent::VoiceServerUpdate(_)
                | GatewayEvent::StreamCreate(_)
                | GatewayEvent::StreamUpdate(_)
                | GatewayEvent::StreamServerUpdate(_)
                | GatewayEvent::StreamDelete(_)
        ) {
            let i = inner.clone();
            let emit: crate::voice::call::Emit = Arc::new(move |e| i.emit(e));
            crate::voice::call::manager().on_gateway(&emit, s.adapter.lock().me.0, &ev);
        }
        account::on_gateway(&s, &ev);
        if let GatewayEvent::Unknown { name, data } = &ev
            && name == "USER_NOTE_UPDATE"
            && let Some(user) = data.get("id").and_then(serde_json::Value::as_str).and_then(|v| v.parse::<u64>().ok())
        {
            s.rest.cache_user_note(api::Snowflake(user), data.get("note").and_then(serde_json::Value::as_str).unwrap_or(""));
        }
        crate::voice::start_time::on_gateway(&ev);
        thread_members::on_gateway(&ev);
        pins::on_gateway(&ev);
        match &ev {
            GatewayEvent::Resumed => self_presence::resend(&s),
            GatewayEvent::Unknown { name, data } if name == "USER_SETTINGS_PROTO_UPDATE" => self_presence::on_settings_update(&s, data),
            _ => {}
        }
        if let GatewayEvent::Ready(r) = &ev {
            *s.presence.lock() = self_presence::Local::from_base64(r.user_settings_proto.as_deref());
            self_presence::send(&s);
            pins::init(&s.cache);
            let run_id = crate::notifications::reminders::restart();
            let (rest, cache) = (s.rest.clone(), s.cache.clone());
            tokio::spawn(async move {
                crate::notifications::reminders::set(&rest.get_reminders().await.unwrap_or_default());
                crate::notifications::reminders::run(cache, run_id).await;
            });
            crate::commands::set_session_id(&r.session_id);
            qos_store::save(r.user.id.0, r.qos_token.as_deref());
            let prefs = Prefs::from_ready(r);
            let data = convert::build_ready(&s.cache, r, &prefs);
            let authors = {
                let mut a = s.adapter.lock();
                let authors = a.take_authors();
                *a = Adapter::new(r.user.id, prefs);
                authors
            };
            let selected = *s.selected_channel.lock();
            let selected_guild = selected.and_then(|c| s.cache.channel(c)).and_then(|c| c.guild_id);
            s.subs.new_session(&s.gateway, selected_guild);
            inner.emit(AppEvent::Connection(true));
            inner.emit(AppEvent::Resync(crate::commands::Resync(data)));
            nav::snapshot(&s.cache, r).into_iter().for_each(|e| inner.emit(e));
            for (g, mut ids) in authors {
                ids.drain(..ids.len().saturating_sub(100));
                ensure_authors(&inner, &s, api::Snowflake(g), ids);
            }
            continue;
        }
        if let Some(e) = crate::commands::from_gateway(&ev) {
            inner.emit(AppEvent::Parity(e));
        }
        nav::on_gateway(&s.cache, &ev).into_iter().for_each(|e| inner.emit(e));
        if let GatewayEvent::MessageCreate(m) = &ev
            && *s.selected_channel.lock() == Some(m.channel_id)
            && m.author.id != s.adapter.lock().me
        {
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            schedule_ack(&s, m.channel_id, m.id);
        }
        let events = s.adapter.lock().map(&s.cache, &ev);
        if let GatewayEvent::MessageCreate(m) = &ev {
            ensure_message_authors(&inner, &s, std::slice::from_ref(&**m));
        }
        let fatal = matches!(ev, GatewayEvent::Fatal { .. });
        for e in events {
            inner.emit(e);
        }
        if let GatewayEvent::Fatal { code: 4004, .. } = ev {
            let _ = store_op(&inner.store, |st| st.clear()).await;
        }
        if fatal {
            inner.session.lock().take();
            return;
        }
    }
}

static ACTIVITY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);
static MEMBERS_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

fn idle_unsubscribe(s: &Session) {
    let me = s.adapter.lock().me;
    let mut keep: Vec<api::Snowflake> = s.cache.guilds().iter().map(|g| g.id).filter(|g| s.cache.voice_states(Some(*g)).iter().any(|v| v.user_id == me)).collect();
    let selected = *s.selected_channel.lock();
    keep.extend(selected.and_then(|c| s.cache.channel(c)).and_then(|c| c.guild_id));
    s.subs.idle(&s.gateway, &keep);
}

fn activity() -> (bool, bool) {
    let v = ACTIVITY.load(std::sync::atomic::Ordering::Relaxed);
    (v & 1 != 0, v & 2 != 0)
}

pub static NO_ACKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn acks_off(no_acks: bool, agent: bool) -> bool {
    no_acks || agent
}

fn acks_disabled() -> bool {
    acks_off(NO_ACKS.load(std::sync::atomic::Ordering::Relaxed), phoenix_api::rest::guard::agent_session())
}

fn ack_allowed(channel: u64) -> bool {
    !acks_disabled() && !phoenix_api::trap::is_trap(channel)
}


fn schedule_ack(s: &Arc<Session>, channel: api::Snowflake, msg: api::Snowflake) {
    if !ack_allowed(channel.0) {
        return;
    }
    s.adapter.lock().mark_read(channel.0, msg.0);
    if !s.pending_acks.lock().insert(channel.0) {
        return;
    }
    let s = s.clone();
    tokio::spawn(async move {
        tokio::time::sleep(ACK_DEBOUNCE).await;
        s.pending_acks.lock().remove(&channel.0);
        let last = s.cache.channel(channel).and_then(|c| c.last_message_id).unwrap_or(msg);
        let _ = s.rest.ack_message(channel, last).await;
    });
}

impl Backend for DiscordBackend {
    fn notify_prefs(&self) -> Option<crate::notifications::NotifyPrefs> {
        Some(self.inner.session.lock().as_ref()?.notify.lock().clone())
    }

    fn set_event_sink(&self, sink: EventSink) {
        *self.inner.sink.lock() = Some(sink);
    }

    fn login(&self, creds: Credentials) -> BoxFuture<ReadyData> {
        self.spawn(move |inner| async move {
            match creds {
                Credentials::Token(t) => {
                    let t = clean_token(&t);
                    if t.is_empty() {
                        return Err(err("Enter a token."));
                    }
                    connect(inner, Secret::new(t), true).await
                }
                Credentials::Password { login, password } => {
                    if login.trim().is_empty() || password.is_empty() {
                        return Err(err("Enter your email and password."));
                    }
                    let rest = RestClient::unauthenticated(inner.props.clone());
                    rest.ensure_fingerprint().await;
                    match account::password_login(&rest, login.trim(), &password).await.map_err(login_err)? {
                        LoginResponse::Token { token, .. } => connect(inner, Secret::new(token), true).await,
                        LoginResponse::MfaRequired { ticket, totp, sms, backup, webauthn, login_instance_id, .. } => {
                            account::mfa_required(&inner, ticket, login_instance_id, crate::auth::MfaOptions { totp, sms, backup, webauthn })
                        }
                    }
                }
            }
        })
    }

    fn restore_session(&self) -> Option<BoxFuture<ReadyData>> {
        Some(self.spawn(move |inner| async move {
            let tok = store_op(&inner.store, |s| s.load()).await.map_err(|e| {
                tracing_warn(&format!("keyring unavailable: {e}"));
                err("")
            })?;
            let Some(tok) = tok else { return Err(err("")) };
            crate::perf::mark("keyring_loaded");
            *inner.pre_rest.lock() = Some(RestClient::new(tok.expose(), inner.props.clone()));
            let res = connect(inner.clone(), tok, false).await;
            if let Err(e) = &res
                && e.0 == "Invalid token."
            {
                let _ = store_op(&inner.store, |s| s.clear()).await;
            }
            res
        }))
    }

    fn mfa_options(&self) -> crate::auth::MfaOptions {
        self.mfa_options_impl()
    }

    fn login_mfa(&self, method: crate::auth::MfaMethod, code: String) -> BoxFuture<ReadyData> {
        self.login_mfa_impl(method, code)
    }

    fn send_mfa_sms(&self) -> BoxFuture<String> {
        self.send_mfa_sms_impl()
    }

    fn reset_mfa(&self) {
        self.inner.mfa_ticket.lock().take();
        self.inner.mfa_instance.lock().take();
    }

    fn remote_auth(&self) -> Option<BoxFuture<ReadyData>> {
        Some(self.remote_auth_impl())
    }

    fn cancel_remote_auth(&self) {
        self.cancel_remote_auth_impl()
    }

    fn load_settings(&self) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        self.load_settings_impl()
    }

    fn save_settings(&self, patch: crate::ui::settings::SettingsPatch) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        self.save_settings_impl(patch)
    }

    fn needs_mfa(&self) -> bool {
        self.inner.mfa_ticket.lock().is_some()
    }

    fn parity(&self, req: crate::commands::ParityRequest) -> BoxFuture<crate::commands::ParityReply> {
        let posts_in = match &req {
            crate::commands::ParityRequest::ArchivedThreads { channel } => Some(*channel),
            crate::commands::ParityRequest::ForumPosts { forum, .. } => Some(*forum),
            _ => None,
        };
        self.with_session(move |inner, s| async move {
            let sink = inner.clone();
            let emit: crate::backend::EventSink = Arc::new(move |e| sink.emit(e));
            let reply = crate::commands::exec::request(s.rest.clone(), req, emit).await?;
            if let crate::commands::ParityReply::ForumPosts { ref posts, .. } = reply
                && let Some(g) = posts_in.and_then(|c| s.cache.channel(sf(c))).and_then(|c| c.guild_id)
            {
                let mut ids: Vec<api::Snowflake> = Vec::new();
                for u in posts.iter().flat_map(|p| p.owner.into_iter().chain(p.first_message.as_ref().map(|m| m.author))) {
                    if !ids.contains(&sf(u)) {
                        ids.push(sf(u));
                    }
                }
                ensure_authors(&inner, &s, g, ids);
            }
            if let crate::commands::ParityReply::Messages(ref msgs) = reply {
                let mut by_guild: std::collections::HashMap<api::Snowflake, Vec<api::Snowflake>> = std::collections::HashMap::new();
                for m in msgs {
                    if let Some(g) = s.cache.channel(sf(m.channel)).and_then(|c| c.guild_id) {
                        let list = by_guild.entry(g).or_default();
                        let u = sf(m.author);
                        if !list.contains(&u) {
                            list.push(u);
                        }
                    }
                }
                for (g, ids) in by_guild {
                    ensure_authors(&inner, &s, g, ids);
                }
            }
            Ok(reply)
        })
    }

    fn action(&self, req: crate::actions::ActionRequest) -> BoxFuture<crate::actions::ActionReply> {
        self.with_session(move |inner, s| async move {
            use crate::actions::ActionRequest;
            use phoenix_api::rest::{guild_manage::AdminCall, guild_server::ServerCall};

            if let ActionRequest::Admin(AdminCall::Server(ServerCall::DeleteGuild { guild }))
            | ActionRequest::AdminMfa(AdminCall::Server(ServerCall::DeleteGuild { guild }), _, _) = &req
            {
                let owner = s.cache.guild(*guild).and_then(|g| g.owner_id());
                let current_user = s.cache.current_user().map(|user| user.id);
                if current_user.is_none() || owner != current_user {
                    return Err(err(crate::t!("gs2.delete_owner_only").to_string()));
                }
            }
            let emit: crate::backend::EventSink = Arc::new(move |e| inner.emit(e));
            crate::actions::exec::request(s.rest.clone(), req, emit).await
        })
    }

    fn guild_features(&self, guild: Id) -> Vec<String> {
        let Ok(s) = self.inner.session() else { return Vec::new() };
        s.cache.guild(sf(guild)).map(|g| g.properties.features.clone()).unwrap_or_default()
    }

    fn channel_opted_in(&self, channel: Id) -> Option<bool> {
        let s = self.inner.session().ok()?;
        Some(s.adapter.lock().prefs.mutes.opted_in.contains(&channel.0))
    }

    fn inbox_unreads(&self, guild_order: &[Id]) -> Vec<crate::ui::inbox_unreads::Entry> {
        let Ok(s) = self.inner.session() else { return Vec::new() };
        let notify = s.notify.lock().clone();
        let adapter = s.adapter.lock();
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        inbox::Ctx { cache: &s.cache, prefs: &adapter.prefs, notify: &notify, new_notifications: s.new_notifications }.entries(guild_order, now)
    }

    fn channel_last_message(&self, channel: Id) -> Option<Id> {
        let s = self.inner.session().ok()?;
        s.cache.channel(sf(channel))?.last_message_id.map(|m| Id(m.0))
    }

    fn guild_has_application(&self, guild: Id) -> bool {
        self.inner.session().ok().and_then(|s| s.cache.guild(sf(guild))).is_some_and(|g| {
            g.roles.iter().any(|role| role.tags.as_ref().is_some_and(|tags| tags.bot_id.is_some() || tags.integration_id.is_some()))
        })
    }

    fn guild_boosts(&self, guild: Id) -> Option<(u32, bool)> {
        let s = self.inner.session().ok()?;
        let g = s.cache.guild(sf(guild))?;
        Some((g.properties.premium_subscription_count.unwrap_or(0), g.properties.premium_progress_bar_enabled.unwrap_or(false)))
    }

    fn guild_tag(&self, guild: Id) -> Option<(String, Option<String>)> {
        let s = self.inner.session().ok()?;
        let p = s.cache.guild(sf(guild))?.properties.profile.clone()?;
        Some((p.tag.filter(|t| !t.is_empty())?, p.badge))
    }

    fn guild_incidents(&self, guild: Id) -> Option<phoenix_api::models::IncidentsData> {
        let s = self.inner.session().ok()?;
        s.cache.guild(sf(guild))?.properties.incidents_data.clone()
    }

    fn guild_home_header(&self, guild: Id) -> Option<String> {
        let s = self.inner.session().ok()?;
        s.cache.guild(sf(guild))?.properties.home_header.clone()
    }

    fn channel_user_limit(&self, channel: Id) -> Option<u32> {
        let s = self.inner.session().ok()?;
        s.cache.channel(sf(channel))?.user_limit.filter(|n| *n > 0)
    }

    fn is_thread_joined(&self, channel: Id) -> bool {
        let Ok(s) = self.inner.session() else { return false };
        s.cache.channel(sf(channel)).is_some_and(|c| c.member.is_some())
    }

    fn thread_info(&self, thread: Id) -> Option<crate::model::ThreadInfo> {
        let s = self.inner.session().ok()?;
        let c = s.cache.channel(sf(thread))?;
        if !c.kind.is_thread() {
            return None;
        }
        let parent_forum = c.parent_id.and_then(|p| s.cache.channel(p)).is_some_and(|p| matches!(p.kind, api::ChannelType::GuildForum | api::ChannelType::GuildMedia));
        let (unread, mentions) = {
            let a = s.adapter.lock();
            let joined = c.member.is_some();
            let (u, m) = a.prefs.ctx().unread(&c);
            (u && joined, if joined { m } else { 0 })
        };
        let md = c.thread_metadata.clone().unwrap_or_default();
        let member = c.member.as_ref();
        let mute = nav::thread_member_mute(&c);
        Some(crate::model::ThreadInfo {
            parent: c.parent_id.map(|p| Id(p.0)),
            owner: c.owner_id.map(|u| Id(u.0)),
            private: c.kind == api::ChannelType::PrivateThread,
            forum_post: parent_forum,
            joined: member.is_some(),
            member_flags: member.and_then(|m| m.get("flags")).and_then(serde_json::Value::as_u64).unwrap_or(0) as u8,
            member_muted: mute.0,
            mute_end: mute.1,
            archived: md.archived,
            locked: md.locked,
            invitable: md.invitable.unwrap_or(true),
            auto_archive: md.auto_archive_duration.unwrap_or(4320),
            slowmode: c.rate_limit_per_user.unwrap_or(0),
            flags: c.flags,
            unread,
            mentions,
            member_count: c.member_count.unwrap_or(0),
            member_ids_preview: c.member_ids_preview.iter().map(|u| Id(u.0)).collect(),
            message_count: c.message_count.unwrap_or(0),
            last_message: c.last_message_id.map(|m| Id(m.0)),
        })
    }

    fn guild_permissions(&self, guild: Id) -> u64 {
        let Ok(s) = self.inner.session() else { return 0 };
        s.cache.guild_permissions(sf(guild)).bits()
    }

    fn channel_permissions(&self, channel: Id) -> Option<u64> {
        let s = self.inner.session().ok()?;
        s.cache.channel(sf(channel))?;
        Some(s.cache.permissions_in(sf(channel)).bits())
    }

    fn overwrites_match(&self, a: Id, b: Id) -> Option<bool> {
        let s = self.inner.session().ok()?;
        let norm = |id: Id| {
            s.cache.channel(sf(id)).map(|c| {
                let mut v: Vec<(u64, u8, u64, u64)> = c
                    .permission_overwrites
                    .iter()
                    .filter(|o| !o.allow.is_empty() || !o.deny.is_empty())
                    .map(|o| (o.id.0, o.kind.value(), o.allow.bits(), o.deny.bits()))
                    .collect();
                v.sort();
                v
            })
        };
        Some(norm(a)? == norm(b)?)
    }

    fn last_read(&self, channel: Id) -> Option<Id> {
        let s = self.inner.session().ok()?;
        let r = s.adapter.lock().prefs.reads.get(&channel.0).map(|r| r.last_read)?;
        Some(Id(r))
    }

    fn unreads_all(&self, channel: Id) -> bool {
        let Ok(s) = self.inner.session() else { return true };
        let Some(c) = s.cache.channel(sf(channel)) else { return true };
        let Some(g) = c.guild_id else { return true };
        let notify = s.notify.lock();
        let adapter = s.adapter.lock();
        inbox::Ctx { cache: &s.cache, prefs: &adapter.prefs, notify: &notify, new_notifications: s.new_notifications }.unreads_all(g.0, &c)
    }

    fn is_media_channel(&self, channel: Id) -> bool {
        let Ok(s) = self.inner.session() else { return false };
        s.cache.channel(sf(channel)).is_some_and(|c| c.kind == api::ChannelType::GuildMedia)
    }

    fn roles_can_view(&self, channel: Id, roles: &[Id]) -> bool {
        let Ok(s) = self.inner.session() else { return true };
        let roles: Vec<_> = roles.iter().map(|r| sf(*r)).collect();
        s.cache.roles_permissions_in(sf(channel), &roles).is_none_or(|p| p.contains(phoenix_api::models::Permissions::VIEW_CHANNEL))
    }

    fn member_permissions(&self, channel: Id, user: Id) -> Option<u64> {
        let s = self.inner.session().ok()?;
        s.cache.member_permissions_in(sf(channel), sf(user)).map(|p| p.bits())
    }

    fn member_timed_out(&self, guild: Id, user: Id) -> bool {
        let Ok(s) = self.inner.session() else { return false };
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        s.cache.member(sf(guild), sf(user)).is_some_and(|m| phoenix_api::models::timed_out(m.communication_disabled_until.as_deref(), now))
    }

    fn member_can_view(&self, channel: Id, user: Id) -> bool {
        let Ok(s) = self.inner.session() else { return true };
        s.cache
            .member_permissions_in(sf(channel), sf(user))
            .is_none_or(|p| p.contains(phoenix_api::models::Permissions::VIEW_CHANNEL))
    }

    fn request_feed_members(&self, guild: Id, users: Vec<Id>) {
        let Ok(s) = self.inner.session() else { return };
        ensure_authors(&self.inner, &s, sf(guild), users.into_iter().map(sf).collect());
    }

    fn activity_feed_enabled(&self, guild: Id) -> bool {
        let Ok(s) = self.inner.session() else { return false };
        let Some(g) = s.cache.guild(sf(guild)) else { return false };
        let has = |f: &str| g.properties.features.iter().any(|x| x == f);
        let (on, off) = (has("ACTIVITY_FEED_ENABLED_BY_USER"), has("ACTIVITY_FEED_DISABLED_BY_USER"));
        if on || off {
            return on;
        }
        g.member_count.is_some_and(|n| n < 10_000)
    }

    fn forum_settings(&self, forum: Id) -> Option<crate::commands::ForumSettings> {
        let s = self.inner.session().ok()?;
        s.cache.channel(sf(forum)).map(|c| crate::commands::ForumSettings::from_channel(&c))
    }

    fn forum_tags(&self, forum: Id) -> Vec<crate::commands::ForumTag> {
        let Ok(s) = self.inner.session() else { return Vec::new() };
        s.cache.channel(sf(forum)).map(|c| convert::forum_tags(&c)).unwrap_or_default()
    }

    fn nav(&self, req: crate::ui::home::data::NavRequest) -> BoxFuture<crate::ui::home::data::NavReply> {
        self.with_session(move |_, s| nav::request(s.rest.clone(), req))
    }

    fn logout(&self) -> BoxFuture<()> {
        self.spawn(move |inner| async move {
            inner.mfa_ticket.lock().take();
            inner.mfa_instance.lock().take();
            inner.pre_rest.lock().take();
            if let Some(s) = inner.session.lock().take() {
                s.gateway.shutdown();
            }
            let _ = tokio::task::spawn_blocking(|| {
                snapshot::clear();
                msg_cache::clear();
            })
            .await;
            store_op(&inner.store, |s| s.clear()).await.map_err(err)
        })
    }

    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            s.subs.select(guild.map(sf));
            if let Some(g) = guild {
                s.subs.guild(&s.gateway, sf(g));
                if let Some((op, d)) = crate::voice::start_time::request(g.0) {
                    let _ = s.gateway.send(phoenix_api::gateway::GatewayCommand::Raw { op, d });
                }
            }
            Ok(())
        })
    }

    fn select_channel(&self, channel: Id) -> BoxFuture<()> {
        self.with_session(move |inner, s| async move {
            let ch = sf(channel);
            *s.selected_channel.lock() = Some(ch);
            let c = s.cache.channel(ch);
            if let Some(g) = c.as_ref().and_then(|c| c.guild_id) {
                if MEMBERS_SHOWN.load(std::sync::atomic::Ordering::Relaxed) {
                    s.subs.channel(&s.gateway, g, ch, vec![subscriptions::FIRST_RANGE], true);
                } else {
                    s.subs.guild(&s.gateway, g);
                }
            }
            let shown = s.adapter.lock().show_list(&s.cache, ch);
            shown.into_iter().for_each(|e| inner.emit(e));
            if let Some(last) = c.and_then(|c| c.last_message_id) {
                let was_unread = {
                    let mut a = s.adapter.lock();
                    let unread = a.prefs.reads.get(&ch.0).is_none_or(|r| r.last_read < last.0 || r.mentions > 0);
                    a.mark_read(ch.0, last.0);
                    unread
                };
                if was_unread && ack_allowed(ch.0) {
                    s.rest.ack_message(ch, last).await.map_err(api_err)?;
                }
            }
            Ok(())
        })
    }

    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        self.load_messages_impl(channel, before, limit)
    }

    fn load_messages_at(&self, channel: Id, at: crate::backend::HistoryAt, limit: u32) -> BoxFuture<Vec<Message>> {
        self.load_messages_at_impl(channel, at, limit)
    }

    fn prefetch_messages(&self, channel: Id) {
        self.prefetch_impl(channel)
    }

    fn has_cached_messages(&self, channel: Id) -> bool {
        msg_cache::has_page(channel.0)
    }

    fn recent_cached_channels(&self, n: usize) -> Vec<Id> {
        msg_cache::recent_on_disk(n).into_iter().map(Id).collect()
    }

    fn restore_cached(&self) -> Option<BoxFuture<ReadyData>> {
        if !snapshot::exists() {
            return None;
        }
        Some(self.spawn(move |inner| async move {
            let t0 = Instant::now();
            let loaded = tokio::task::spawn_blocking(snapshot::take).await.ok().flatten();
            let (data, events) = loaded.ok_or_else(|| err(""))?;
            crate::perf::span("snapshot_loaded", t0);
            if inner.session.lock().is_none() {
                events.into_iter().for_each(|e| inner.emit(e));
            }
            Ok(data)
        }))
    }

    fn send_message(&self, channel: Id, content: String, reply_to: Option<Id>) -> BoxFuture<Message> {
        self.with_session(move |_, s| async move {
            let mut msg = CreateMessage::text(content);
            if let Some(r) = reply_to {
                msg = msg.reply_to(sf(channel), sf(r), true);
            }
            let m = s.rest.send_message(sf(channel), msg).await.map_err(api_err)?;
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            Ok(convert::message(&m))
        })
    }

    fn send_tts_message(&self, channel: Id, content: String) -> BoxFuture<Message> {
        self.with_session(move |_, s| async move {
            let mut msg = CreateMessage::text(content);
            msg.tts = true;
            msg.nonce = Some(crate::ui::chat::pending::make_nonce().to_string());
            let m = s.rest.send_message(sf(channel), msg).await.map_err(api_err)?;
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            Ok(convert::message(&m))
        })
    }

    fn edit_message(&self, channel: Id, msg: Id, content: String) -> BoxFuture<Message> {
        self.with_session(move |_, s| async move {
            let m = s.rest.edit_message(sf(channel), sf(msg), &content).await.map_err(api_err)?;
            Ok(convert::message(&m))
        })
    }

    fn delete_message(&self, channel: Id, msg: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move { s.rest.delete_message(sf(channel), sf(msg)).await.map_err(api_err) })
    }

    fn trigger_typing(&self, channel: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            {
                let mut last = s.last_typing.lock();
                if last.is_some_and(|(c, t)| c == channel.0 && t.elapsed() < TYPING_INTERVAL) {
                    return Ok(());
                }
                *last = Some((channel.0, Instant::now()));
            }
            s.rest.trigger_typing(sf(channel)).await.map_err(api_err)
        })
    }

    fn send_message_ex(
        &self,
        channel: Id,
        content: String,
        reply: Option<(Id, bool)>,
        files: Vec<crate::backend::Upload>,
        nonce: u64,
        progress: Option<crate::backend::UploadProgress>,
    ) -> BoxFuture<Message> {
        self.with_session(move |_, s| async move {
            let mut msg = CreateMessage::text(content);
            msg.nonce = Some(nonce.to_string());
            msg.enforce_nonce = Some(true);
            if let Some((r, mention)) = reply {
                msg = msg.reply_to(sf(channel), sf(r), mention);
            }
            let pres: Vec<Option<preupload::Pre>> = files.iter().map(|f| f.pre.and_then(preupload::take)).collect();
            let hero = files.iter().position(|f| f.hero);
            let mut files: Vec<phoenix_api::rest::FileUpload> = files
                .into_iter()
                .map(|f| phoenix_api::rest::FileUpload {
                    filename: f.filename,
                    bytes: f.bytes,
                    description: f.description,
                    spoiler: f.spoiler,
                    ..Default::default()
                })
                .collect();
            if let Some(hero) = hero {
                phoenix_api::rest::uploads::unique_filenames(&mut files);
                msg.components = phoenix_api::rest::uploads::announcement_components(&msg.content, &files, hero);
                msg.content.clear();
                msg.flags = Some(phoenix_api::rest::uploads::FLAG_IS_COMPONENTS_V2);
            }
            let attachments = if files.is_empty() {
                Vec::new()
            } else {
                let slots = preupload::resolve(&s.rest, sf(channel), &files, pres, progress).await.map_err(|e| match e {
                    e @ ApiError::Api { status: 400..=499, .. } => BackendError::rejected(api_err(e)),
                    e => api_err(e),
                })?;
                phoenix_api::rest::uploads::message_attachments(&files, &slots).as_array().cloned().unwrap_or_default()
            };
            msg.attachments = attachments;
            let mut backoff = phoenix_api::rest::backoff::Backoff::default();
            let m = loop {
                let res = s.rest.send_message_failed(sf(channel), msg.clone(), backoff.fails()).await;
                match res {
                    Ok(m) => break m,
                    Err(ApiError::Http(_)) if backoff.fails() < SEND_RETRIES => {
                        tokio::time::sleep(backoff.fail()).await;
                    }
                    Err(ApiError::Http(e)) => return Err(api_err(ApiError::Http(e))),
                    Err(e @ ApiError::Api { status: 400..=499, .. }) => return Err(BackendError::rejected(api_err(e))),
                    Err(e) => return Err(api_err(e)),
                }
            };
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            Ok(convert::message(&m))
        })
    }

    fn preupload(&self, channel: Id, key: u64, filename: String, bytes: Arc<Vec<u8>>) {
        let Ok(s) = self.inner.session() else { return };
        let (tx, rx) = tokio::sync::watch::channel(None);
        let sent = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let counter = sent.clone();
        let task = self.inner.rt.spawn(async move {
            let file = phoenix_api::rest::FileUpload { filename, bytes: bytes.to_vec(), ..Default::default() };
            let res = s
                .rest
                .upload_one(sf(channel), &file, &|n| {
                    counter.store(n, std::sync::atomic::Ordering::Relaxed);
                    true
                })
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(Some(res));
        });
        preupload::insert(key, preupload::Pre { sent, rx, abort: task.abort_handle() });
    }

    fn drop_preupload(&self, key: u64) {
        preupload::drop_key(key);
    }

    fn send_voice_message(&self, channel: Id, voice: crate::backend::VoiceUpload, reply: Option<(Id, bool)>, nonce: u64) -> BoxFuture<Message> {
        use phoenix_voice::audio::voice_message as vm;
        self.with_session(move |_, s| async move {
            let mut msg = CreateMessage::text(String::new());
            msg.nonce = Some(nonce.to_string());
            msg.enforce_nonce = Some(true);
            msg.flags = Some(vm::FLAG_IS_VOICE_MESSAGE);
            if let Some((r, mention)) = reply {
                msg = msg.reply_to(sf(channel), sf(r), mention);
            }
            let files = vec![phoenix_api::rest::FileUpload {
                filename: vm::FILENAME.into(),
                bytes: voice.ogg,
                content_type: Some(vm::CONTENT_TYPE.into()),
                duration_secs: Some(voice.duration_secs),
                waveform: Some(voice.waveform),
                ..Default::default()
            }];
            let rejected = |e: ApiError| match e {
                e @ ApiError::Api { status: 400..=499, .. } => BackendError::rejected(api_err(e)),
                e => api_err(e),
            };
            let slots = s.rest.upload_files(sf(channel), &files, None, phoenix_api::rest::uploads::UPLOAD_CHUNK).await.map_err(rejected)?;
            msg.attachments = phoenix_api::rest::uploads::message_attachments(&files, &slots).as_array().cloned().unwrap_or_default();
            let m = s.rest.send_message(sf(channel), msg).await.map_err(rejected)?;
            s.adapter.lock().mark_read(m.channel_id.0, m.id.0);
            Ok(convert::message(&m))
        })
    }

    fn guild_emojis(&self, guild: Id) -> Vec<crate::ui::pickers::types::GuildEmoji> {
        self.inner.session().map(|s| expressions::guild_emojis(&s, guild)).unwrap_or_default()
    }
    fn max_upload_size(&self, channel: Id) -> u64 {
        use phoenix_api::rest::uploads::{max_file_size, message_upload_limit};
        let Ok(s) = self.inner.session() else { return message_upload_limit(10 << 20) };
        let premium = s.cache.current_user().and_then(|u| u.premium_type);
        let guild = s.cache.channel(sf(channel)).and_then(|c| c.guild_id).and_then(|g| s.cache.guild(g));
        let limit = max_file_size(premium, guild.as_ref().map(|g| (g.properties.premium_tier, g.properties.features.iter().map(String::as_str))));
        message_upload_limit(limit)
    }
    fn premium_group_member(&self) -> bool {
        self.inner.session().ok().and_then(|s| s.cache.current_user()).is_some_and(|u| u.is_premium_group_member())
    }
    fn premium_with_group(&self) -> bool {
        self.inner.session().ok().and_then(|s| s.cache.current_user()).is_some_and(|u| u.is_premium_with_premium_group())
    }
    fn premium_tier2(&self) -> bool {
        self.inner.session().ok().and_then(|s| s.cache.current_user()).is_some_and(|u| u.premium_type == Some(2))
    }
    fn nitro_tier(&self) -> Option<u8> {
        self.inner.session().ok()?.cache.current_user().map(|user| user.premium_type.unwrap_or(0))
    }
    fn has_premium(&self) -> bool {
        self.inner.session().ok().and_then(|s| s.cache.current_user()).is_none_or(|u| u.premium_type.unwrap_or(0) > 0)
    }
    fn guild_stickers(&self, guild: Id) -> Vec<crate::ui::pickers::types::StickerData> {
        self.inner.session().map(|s| expressions::guild_stickers(&s, guild)).unwrap_or_default()
    }
    fn sticker_packs(&self) -> BoxFuture<Vec<crate::ui::pickers::types::StickerPackData>> {
        self.with_session(|_, s| expressions::sticker_packs(s))
    }
    fn stop_ringing(&self, channel: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let path = format!("/channels/{}/call/stop-ringing", channel.0);
            s.rest.request_json(reqwest::Method::POST, &path, Some(serde_json::json!({}))).await.map_err(api_err)?;
            Ok(())
        })
    }
    fn send_sticker(&self, channel: Id, sticker: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| expressions::send_sticker(s, channel, sticker))
    }
    fn gif_trending(&self) -> BoxFuture<crate::ui::pickers::types::GifHome> {
        self.with_session(|_, s| expressions::gif_trending(s))
    }
    fn gif_trending_gifs(&self) -> BoxFuture<Vec<crate::ui::pickers::types::GifItem>> {
        self.with_session(|_, s| expressions::gif_trending_gifs(s))
    }
    fn expression_frecency(&self) -> BoxFuture<crate::ui::pickers::types::Frecency> {
        self.with_session(|_, s| expressions::frecency(s))
    }
    fn gif_search(&self, query: String) -> BoxFuture<Vec<crate::ui::pickers::types::GifItem>> {
        self.with_session(move |_, s| expressions::gif_search(s, query))
    }
    fn search_messages(&self, req: crate::ui::search::SearchRequest) -> BoxFuture<crate::ui::search::SearchPage> {
        self.with_session(move |inner, s| expressions::search(inner, s, req))
    }

    fn set_reaction(&self, channel: Id, msg: Id, emoji: crate::model::Reaction, add: bool) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let e = api::ReactionEmoji {
                id: emoji.emoji_id.map(sf),
                name: Some(crate::ui::chat::state::reaction_name(&emoji.emoji)),
                animated: false,
            };
            if add {
                s.rest.add_reaction(sf(channel), sf(msg), &e).await
            } else {
                s.rest.remove_own_reaction(sf(channel), sf(msg), &e).await
            }
            .map_err(api_err)
        })
    }

    fn set_pinned(&self, channel: Id, msg: Id, pinned: bool) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            if pinned {
                s.rest.pin_message(sf(channel), sf(msg)).await
            } else {
                s.rest.unpin_message(sf(channel), sf(msg)).await
            }
            .map_err(api_err)
        })
    }

    fn load_pins(&self, channel: Id) -> BoxFuture<Vec<Message>> {
        self.with_session(move |inner, s| async move {
            let msgs = s.rest.get_pinned_messages(sf(channel)).await.map_err(api_err)?;
            let mut users = Vec::new();
            for m in &msgs {
                convert::message_users(m, &mut users);
            }
            inner.emit(AppEvent::Users(users));
            ensure_message_authors(&inner, &s, &msgs);
            Ok(msgs.iter().map(convert::message).collect())
        })
    }

    fn has_unread_pins(&self, channel: Id) -> bool {
        pins::has_unread(channel.0)
    }

    fn ack_pins(&self, channel: Id) -> BoxFuture<()> {
        let send = pins::mark_seen(channel.0) && ack_allowed(channel.0);
        self.with_session(move |_, s| async move {
            if send {
                s.rest.request_json(reqwest::Method::POST, &format!("/channels/{}/pins/ack", channel.0), None).await.map_err(api_err)?;
            }
            Ok(())
        })
    }

    fn mark_unread(&self, channel: Id, last_read: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            s.adapter.lock().prefs.reads.entry(channel.0).or_default().last_read = last_read.0;
            if !ack_allowed(channel.0) {
                return Ok(());
            }
            s.rest.ack_message_manual(sf(channel), sf(last_read), 0).await.map_err(api_err)
        })
    }

    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()> {
        self.with_session(move |inner, s| async move {
            let emit: crate::voice::call::Emit = Arc::new(move |e| inner.emit(e));
            crate::voice::call::manager().join(&emit, &s.gateway, &s.cache, channel.map(|c| c.0)).map_err(err)
        })
    }

    fn voice_control(&self, control: crate::voice::VoiceControl) -> BoxFuture<()> {
        let s = self.inner.session.lock().clone();
        crate::voice::call::manager().apply_control(s.as_ref().map(|s| &s.gateway), control);
        Box::pin(async { Ok(()) })
    }

    fn subscribe_member_ranges(&self, guild: Id, channel: Id, ranges: Vec<[u32; 2]>) -> BoxFuture<()> {
        self.people_ranges(guild, channel, ranges)
    }
    fn set_activity(&self, foregrounded: bool, rtc_connected: bool) {
        phoenix_api::props::set_app_focused(foregrounded);
        let v = u8::from(foregrounded) | (u8::from(rtc_connected) << 1);
        if ACTIVITY.swap(v, std::sync::atomic::Ordering::Relaxed) == v {
            return;
        }
        if let Some(s) = self.inner.session.lock().as_ref() {
            let _ = s.gateway.set_activity(foregrounded, rtc_connected);
        }
    }
    fn set_members_shown(&self, shown: bool) {
        if MEMBERS_SHOWN.swap(shown, std::sync::atomic::Ordering::Relaxed) == shown || !shown {
            return;
        }
        let Some(s) = self.inner.session.lock().clone() else { return };
        let Some(ch) = *s.selected_channel.lock() else { return };
        if let Some(g) = s.cache.channel(ch).and_then(|c| c.guild_id) {
            let _guard = self.inner.rt.enter();
            s.subs.channel(&s.gateway, g, ch, vec![subscriptions::FIRST_RANGE], true);
        }
    }
    fn thread_members(&self, thread: Id) -> BoxFuture<Vec<crate::model::Member>> {
        self.thread_members_impl(thread)
    }
    fn user_profile(&self, user: Id, guild: Option<Id>) -> BoxFuture<crate::presence::Profile> {
        self.people_profile(user, guild, true)
    }
    fn user_profile_without_note(&self, user: Id, guild: Option<Id>) -> BoxFuture<crate::presence::Profile> {
        self.people_profile(user, guild, false)
    }
    fn guild_profile(&self, guild: Id) -> BoxFuture<crate::commands::guide::GuildProfile> {
        self.with_session(move |_, s| async move {
            let v = s.rest.guild_profile(sf(guild)).await.map_err(api_err)?;
            let mut profile = crate::commands::guide::parse_profile(&v);
            if profile.banner_hash.is_none() {
                profile.banner_hash = s.cache.guild(sf(guild)).and_then(|g| g.properties.banner.clone());
            }
            Ok(profile)
        })
    }
    fn join_guild(&self, guild: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            s.rest.guarded_write(&phoenix_api::rest::guard::GuardedWrite::join_guild(guild.0)).await.map(|_| ()).map_err(api_err)
        })
    }
    fn set_note(&self, user: Id, note: String) -> BoxFuture<()> {
        self.people_note(user, note)
    }
    fn wishlist(&self, user: Id) -> BoxFuture<Vec<crate::ui::profile_wishlist::WishItem>> {
        self.wishlist_impl(user)
    }
    fn game_widgets(&self, user: Id) -> BoxFuture<Vec<crate::ui::profile_games::GameWidget>> {
        self.game_widgets_impl(user)
    }
    fn games(&self, ids: Vec<u64>) -> BoxFuture<Vec<crate::ui::profile_games::Game>> {
        self.games_impl(ids)
    }
    fn recent_activity(&self, user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_activity::RecentGame>> {
        self.recent_activity_impl(user)
    }
    fn owned_collectibles(&self) -> BoxFuture<Vec<crate::ui::settings::collectibles::Collectible>> {
        self.owned_collectibles_impl()
    }
    fn collectibles_shop(&self) -> BoxFuture<crate::ui::settings::collectibles::ShopCatalog> {
        self.collectibles_shop_impl()
    }
    fn shop_storefront(&self) -> BoxFuture<serde_json::Value> {
        self.shop_storefront_impl()
    }
    fn shop_home(&self) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move { s.rest.get_collectibles_shop_home().await.map_err(api_err) })
    }
    fn shop_tab(&self, tab: &'static str) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move { s.rest.get_collectibles_shop_tab(tab).await.map_err(api_err) })
    }
    fn quests(&self) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move { s.rest.request_json(reqwest::Method::GET, "/quests/@me", None).await.map_err(api_err) })
    }
    fn quest_home_hero(&self) -> BoxFuture<serde_json::Value> {
        let path = crate::ui::quests_page::home_hero_path();
        self.with_session(move |_, s| async move { s.rest.request_json(reqwest::Method::GET, &path, None).await.map_err(api_err) })
    }
    fn discoverable_guilds(&self, category: Option<u32>, offset: u32) -> BoxFuture<serde_json::Value> {
        let path = crate::ui::discovery::featured_path(category, offset);
        self.with_session(move |_, s| async move { s.rest.request_json(reqwest::Method::GET, &path, None).await.map_err(api_err) })
    }
    fn discovery_categories(&self, locale: String) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move {
            let path = format!("/discovery/categories?locale={locale}");
            s.rest.request_json(reqwest::Method::GET, &path, None).await.map_err(api_err)
        })
    }
    fn app_directory_collections(&self, locale: String) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move {
            let path = format!("/application-directory-static/collections?surface=1&active_state=1&platform=4&locale={locale}&cache=true");
            s.rest.request_json(reqwest::Method::GET, &path, None).await.map_err(api_err)
        })
    }
    fn directory_get(&self, path: String) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move {
            if !crate::ui::discovery::directory_path_allowed(&path) {
                return Err(BackendError(format!("directory_get refused: {path}")));
            }
            s.rest.request_json(reqwest::Method::GET, &path, None).await.map_err(api_err)
        })
    }
    fn webauthn_credentials(&self) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move { s.rest.request_json(reqwest::Method::GET, "/users/@me/mfa/webauthn/credentials", None).await.map_err(api_err) })
    }
    fn teams(&self) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move { s.rest.request_json(reqwest::Method::GET, "/teams?include_payout_account_status=true", None).await.map_err(api_err) })
    }
    fn lurk_guild(&self, guild: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let path = crate::ui::discovery::lurk_path(guild.0, &crate::commands::session_id());
            s.rest.request_json(reqwest::Method::PUT, &path, Some(serde_json::json!({}))).await.map_err(api_err).map(|_| ())
        })
    }
    fn orbs_balance(&self) -> BoxFuture<Option<u64>> {
        self.orbs_balance_impl()
    }
    fn collectible_product(&self, sku: u64) -> BoxFuture<Option<crate::ui::settings::collectibles::Collectible>> {
        self.collectible_product_impl(sku)
    }
    fn storefront_skus(&self, skus: Vec<u64>) -> BoxFuture<Vec<phoenix_api::rest::collectibles::LookSku>> {
        self.storefront_skus_impl(skus)
    }
    fn storefront_pricing(&self, skus: Vec<u64>) -> BoxFuture<serde_json::Value> {
        self.with_session(move |_, s| async move { s.rest.get_storefront_pricing(&skus).await.map_err(api_err) })
    }
    fn authorized_apps(&self) -> BoxFuture<Option<Vec<crate::ui::settings::authorized_apps::AuthorizedApp>>> {
        self.authorized_apps_impl()
    }
    fn user_connections(&self) -> BoxFuture<Option<Vec<crate::ui::settings::connections::LinkedAccount>>> {
        self.user_connections_impl()
    }
    fn connection_authorize(&self, path: String) -> BoxFuture<Option<String>> {
        self.connection_authorize_impl(path)
    }
    fn billing(&self) -> BoxFuture<Option<crate::ui::settings::billing::Billing>> {
        self.billing_impl()
    }
    fn subscriptions(&self) -> BoxFuture<Option<crate::ui::settings::subscriptions::Subs>> {
        self.subscriptions_impl()
    }
    fn deauthorize_app(&self, token_id: u64) -> BoxFuture<()> {
        self.deauthorize_app_impl(token_id)
    }
    fn account_standing(&self) -> BoxFuture<Option<u64>> {
        self.account_standing_impl()
    }
    fn guild_affinities(&self) -> BoxFuture<Vec<crate::model::Id>> {
        self.guild_affinities_impl()
    }
    fn user_affinities(&self) -> BoxFuture<Vec<crate::ui::home::friends::Affinity>> {
        self.user_affinities_impl()
    }
    fn guarded_write(&self, w: crate::actions::guard::GuardedWrite) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            s.rest.guarded_write(&w).await.map_err(api_err)?;
            Ok(())
        })
    }
    fn auth_sessions(&self) -> BoxFuture<Option<Vec<crate::ui::settings::sessions::Session>>> {
        self.auth_sessions_impl()
    }
    fn logout_sessions(&self, hashes: Vec<String>) -> BoxFuture<()> {
        self.logout_sessions_impl(hashes)
    }
    fn profile_widgets(&self, user: Id) -> BoxFuture<Vec<crate::ui::settings::profile_widgets::AppWidget>> {
        self.profile_widgets_impl(user)
    }
    fn update_profile(&self, guild: Option<Id>, patch: crate::ui::settings::profile_editor::ProfilePatch) -> BoxFuture<()> {
        self.profile_update_impl(guild, patch)
    }
    fn set_status(&self, status: crate::model::Presence, custom: Option<String>) -> BoxFuture<()> {
        self.people_status(status, custom)
    }
    fn save_custom_status(&self, status: crate::model::Presence, draft: Option<crate::ui::modals::custom_status::StatusDraft>) -> BoxFuture<()> {
        self.people_custom_status(status, draft)
    }
    fn open_dm(&self, user: Id) -> BoxFuture<Id> {
        self.people_open_dm(user)
    }
    fn soundboard_default_sounds(&self) -> BoxFuture<Vec<crate::model::SoundboardSound>> {
        self.with_session(move |_, s| async move {
            let sounds = s.rest.soundboard_default_sounds().await.map_err(api_err)?;
            Ok(sounds
                .into_iter()
                .map(|x| crate::model::SoundboardSound {
                    id: Id(x.sound_id.0),
                    guild: x.guild_id.map(|g| Id(g.0)),
                    name: Some(x.name.into()),
                    emoji_id: x.emoji_id.map(|e| Id(e.0)),
                    emoji_name: x.emoji_name.map(Into::into),
                    volume: x.volume as f32,
                    available: x.available,
                    user: x.user_id.map(|u| Id(u.0)),
                })
                .collect())
        })
    }
    fn request_soundboard_sounds(&self, guilds: Vec<Id>) {
        if guilds.is_empty() {
            return;
        }
        let Ok(s) = self.inner.session() else { return };
        let ids = guilds.into_iter().map(sf).collect();
        let _ = s.gateway.request_soundboard_sounds(ids);
    }
    fn top_soundboard_sounds(&self, guild: Id) -> BoxFuture<Vec<Id>> {
        self.with_session(move |_, s| async move {
            let top = s.rest.top_soundboard_sounds(sf(guild)).await.map_err(api_err)?;
            Ok(top.ranked().into_iter().map(|x| Id(x.0)).collect())
        })
    }
    fn user_experiments(&self) -> BoxFuture<Vec<(u32, i64)>> {
        self.with_session(move |_, s| async move { s.rest.user_experiments().await.map_err(api_err) })
    }
    fn voice_regions(&self) -> BoxFuture<Vec<(gpui::SharedString, gpui::SharedString)>> {
        self.with_session(move |_, s| async move {
            let regions = s.rest.voice_regions().await.map_err(api_err)?;
            Ok(regions.into_iter().map(|r| (gpui::SharedString::from(r.id), gpui::SharedString::from(r.name))).collect())
        })
    }
    fn play_entrance_sound(&self, channel: Id) -> BoxFuture<()> {
        self.with_session(move |_, s| async move { s.rest.play_entrance_sound(sf(channel)).await.map_err(api_err) })
    }

    fn send_soundboard_sound(&self, channel: Id, sound: crate::model::SoundboardSound) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            s.rest
                .send_soundboard_sound(
                    sf(channel),
                    sf(sound.id),
                    sound.guild.map(sf),
                    sound.emoji_id.map(sf),
                    sound.emoji_name.as_deref(),
                )
                .await
                .map_err(api_err)
        })
    }
    fn upload_stream_preview(&self, stream_key: String, thumbnail: String) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            s.rest.upload_stream_preview(&stream_key, &thumbnail).await.map_err(|e| {
                tracing::warn!(key = %stream_key, "stream preview upload failed: {e}");
                api_err(e)
            })
        })
    }
    fn stream_preview(&self, stream_key: String) -> BoxFuture<Option<String>> {
        self.with_session(move |_, s| async move {
            match s.rest.stream_preview(&stream_key).await {
                Ok(p) => Ok(p.url),
                Err(e) => {
                    tracing::warn!(key = %stream_key, "stream preview fetch failed: {e}");
                    Err(api_err(e))
                }
            }
        })
    }
}

#[cfg(test)]
mod ack_tests {
    #[test]
    fn agent_sessions_and_trap_channels_never_ack() {
        assert!(super::acks_off(false, true));
        assert!(super::acks_off(true, false));
        assert!(!super::acks_off(false, false));
        assert!(!super::ack_allowed(1507838362562859119));
        if phoenix_api::rest::guard::agent_session() {
            assert!(!super::ack_allowed(1403447573389381652));
        }
    }
}
