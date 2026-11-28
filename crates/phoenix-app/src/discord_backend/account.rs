use phoenix_api::{
    remote_auth::{self, RemoteAuthEvent},
    rest::settings::{self as api_settings, PreloadedUserSettings, SettingChange, Theme},
};

use super::*;
use crate::theme::Density;
use crate::{
    auth::{MfaMethod, MfaOptions, RemoteAuthUpdate},
    ui::settings::{AppTheme, SettingsPatch, SyncedSettings},
};

pub(super) fn mfa_required(inner: &Inner, ticket: String, login_instance_id: Option<String>, opts: MfaOptions) -> Result<ReadyData, BackendError> {
    *inner.mfa_ticket.lock() = Some(Secret::new(ticket));
    *inner.mfa_instance.lock() = login_instance_id;
    *inner.mfa_opts.lock() = opts;
    Err(err(""))
}

pub(super) fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        tracing_warn(&format!("captcha: could not open the browser: {e}"));
    }
}

fn captcha_text() -> phoenix_api::captcha::PageText {
    phoenix_api::captcha::PageText {
        lang: crate::i18n::locale().split(['-', '_']).next().unwrap_or("en").to_owned(),
        title: crate::t!("captcha.title").to_string(),
        subtitle: crate::t!("captcha.subtitle").to_string(),
        done: crate::t!("captcha.done").to_string(),
        cancel: crate::t!("common.cancel").to_string(),
    }
}

pub(super) fn captcha_solver() -> phoenix_api::captcha::Solver {
    phoenix_api::captcha::solver(|body| async move { phoenix_api::captcha::solve(&body, &captcha_text(), open_browser).await })
}

pub(super) async fn password_login(rest: &RestClient, login: &str, password: &str) -> Result<phoenix_api::rest::LoginResponse, ApiError> {
    let solver = captcha_solver();
    let mut solution = None;
    for _ in 0..3 {
        match rest.login_with(login, password, solution.as_ref()).await {
            Err(ApiError::CaptchaRequired { body, sitekey }) => match solver((*body).clone()).await {
                Some(s) => solution = Some(s),
                None => return Err(ApiError::CaptchaRequired { body, sitekey }),
            },
            other => return other,
        }
    }
    rest.login_with(login, password, solution.as_ref()).await
}

fn mfa_err(e: ApiError) -> BackendError {
    match &e {
        ApiError::CaptchaRequired { .. } | ApiError::RateLimited { .. } => api_err(e),
        ApiError::Api { body, .. } if !body.message.is_empty() => err(body.message.clone()),
        _ => err(crate::t!("login_mfa.error_generic").to_string()),
    }
}

impl DiscordBackend {
    pub(super) fn mfa_options_impl(&self) -> MfaOptions {
        *self.inner.mfa_opts.lock()
    }

    pub(super) fn login_mfa_impl(&self, method: MfaMethod, code: String) -> BoxFuture<ReadyData> {
        self.spawn(move |inner| async move {
            let ticket = inner.mfa_ticket.lock().clone().ok_or_else(|| err(crate::t!("login_mfa.error_generic").to_string()))?;
            let instance = inner.mfa_instance.lock().clone();
            let rest = RestClient::unauthenticated(inner.props.clone());
            let token = rest.login_mfa(method.api(), ticket.expose(), &code, instance.as_deref()).await.map_err(mfa_err)?;
            inner.mfa_ticket.lock().take();
            inner.mfa_instance.lock().take();
            connect(inner, Secret::new(token), true).await
        })
    }

    pub(super) fn send_mfa_sms_impl(&self) -> BoxFuture<String> {
        self.spawn(move |inner| async move {
            let ticket = inner.mfa_ticket.lock().clone().ok_or_else(|| err(crate::t!("login_mfa.error_generic").to_string()))?;
            let rest = RestClient::unauthenticated(inner.props.clone());
            rest.login_mfa_sms_send(ticket.expose()).await.map_err(mfa_err)
        })
    }

    pub(super) fn cancel_remote_auth_impl(&self) {
        if let Some(h) = self.inner.remote_auth.lock().take() {
            h.abort();
        }
    }

    pub(super) fn remote_auth_impl(&self) -> BoxFuture<ReadyData> {
        self.cancel_remote_auth_impl();
        self.spawn(move |inner| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let rest = RestClient::unauthenticated(inner.props.clone());
            rest.ensure_fingerprint().await;
            let task = tokio::spawn(remote_auth::run(rest, tx, Some(captcha_solver())));
            *inner.remote_auth.lock() = Some(task.abort_handle());
            while let Some(ev) = rx.recv().await {
                match ev {
                    RemoteAuthEvent::Qr(url) => inner.emit(AppEvent::RemoteAuth(RemoteAuthUpdate::Qr(url))),
                    RemoteAuthEvent::Scanned { username, .. } => {
                        inner.emit(AppEvent::RemoteAuth(RemoteAuthUpdate::Scanned { username }))
                    }
                    RemoteAuthEvent::Token(t) => {
                        inner.remote_auth.lock().take();
                        return connect(inner, Secret::new(t), true).await;
                    }
                    RemoteAuthEvent::Cancelled => return Err(err("")),
                }
            }
            match task.await {
                Ok(Err(e)) if e.captcha().is_some() => Err(err(crate::t!("login.captcha_failed").to_string())),
                Ok(Err(e)) => {
                    tracing_warn(&format!("remote auth failed: {}", short(&e)));
                    Err(err(crate::t!("login.qr_error").to_string()))
                }
                _ => Err(err("")),
            }
        })
    }

    pub(super) fn load_settings_impl(&self) -> BoxFuture<SyncedSettings> {
        self.with_session(move |_, s| async move {
            let mut out = match s.rest.get_settings_proto_raw().await {
                Ok(raw) => synced_raw(&raw)?,
                Err(_) => {
                    let v = s.rest.get_settings_json().await.map_err(api_err)?;
                    synced(&api_settings::from_json(&v))
                }
            };
            s.rest.set_locale(&out.locale);
            let consent = s.rest.get_consent().await.ok();
            let email = s.rest.get_email_settings().await.ok();
            out.rest = crate::ui::settings::synced_map::read_rest(consent.as_ref(), email.as_ref());
            Ok(out)
        })
    }

    pub(super) fn save_settings_impl(&self, patch: SettingsPatch) -> BoxFuture<SyncedSettings> {
        self.with_session(move |_, s| async move {
            if let SettingsPatch::Rest(id, v) = &patch {
                use crate::ui::settings::synced_map::{Rest, rest};
                match rest(id).map(|r| r.0) {
                    Some(Rest::Consent(k)) => s.rest.set_consent(k, *v).await.map_err(api_err)?,
                    Some(Rest::Email(k)) => s.rest.patch_email_setting(k, *v).await.map(|_| ()).map_err(api_err)?,
                    None => return Err(err(format!("unknown setting {id}"))),
                }
                let raw = s.rest.get_settings_proto_raw().await.map_err(api_err)?;
                let mut out = synced_raw(&raw)?;
                out.rest.insert(*id, *v);
                return Ok(out);
            }
            if let SettingsPatch::JoinSound(guild, sound) = patch {
                let raw = s.rest.edit_settings_proto(move |raw| api_settings::with_guild_join_sound(&raw, guild, sound)).await.map_err(api_err)?;
                return synced_raw(&raw);
            }
            if let SettingsPatch::RaidAlerts(g, a) = &patch {
                let raw = s.rest.save_guild_raid_alerts(*g, *a).await.map_err(api_err)?;
                return synced_raw(&raw);
            }
            if let SettingsPatch::Favorites(f) = &patch {
                let raw = s.rest.patch_favorites(f).await.map_err(api_err)?;
                return synced_raw(&raw);
            }
            let change = match &patch {
                SettingsPatch::Theme(t) => SettingChange::Theme(theme_to_api(*t)),
                SettingsPatch::Compact(b) => SettingChange::Compact(*b),
                SettingsPatch::DeveloperMode(b) => SettingChange::DeveloperMode(*b),
                SettingsPatch::Locale(l) => SettingChange::Locale(l.to_string()),
                SettingsPatch::AllowServerDms(b) => SettingChange::DefaultGuildsRestricted(!*b),
                SettingsPatch::Rest(..) | SettingsPatch::Favorites(..) | SettingsPatch::RaidAlerts(..) | SettingsPatch::JoinSound(..) => unreachable!("handled above"),
                SettingsPatch::Proto(id, v) => {
                    crate::ui::settings::synced_map::change(id, *v).ok_or_else(|| err(format!("unknown setting {id}")))?
                }
                SettingsPatch::Density(d) => SettingChange::UiDensity(density_to_proto(*d)),
            };
            let raw = s.rest.patch_setting_raw(&change).await.map_err(api_err)?;
            let r = synced_raw(&raw);
            if let Ok(set) = &r {
                s.rest.set_locale(&set.locale);
            }
            r
        })
    }
}

fn short(e: &phoenix_api::Error) -> String {
    let s = e.to_string();
    s.chars().take(120).collect()
}

fn theme_to_api(t: AppTheme) -> Theme {
    match t {
        AppTheme::Dark => Theme::Dark,
        AppTheme::Light => Theme::Light,
        AppTheme::Ash => Theme::Ash,
        AppTheme::Onyx => Theme::Onyx,
    }
}

fn synced_raw(raw: &[u8]) -> Result<SyncedSettings, crate::backend::BackendError> {
    let p = PreloadedUserSettings::decode_raw(raw).map_err(api_err)?;
    let mut s = synced(&p);
    s.proto = crate::ui::settings::synced_map::read_all(raw);
    if let Some(e) = phoenix_api::rest::settings::custom_status_expires_ms(raw) {
        s.proto.insert(crate::ui::modals::custom_status::EXPIRES_KEY, e);
    }
    s.favorites = phoenix_api::rest::favorites::Favorites::from_settings(raw);
    s.join_sounds = api_settings::guild_join_sounds(raw);
    s.raid_alerts = phoenix_api::rest::settings::guild_raid_alerts(raw);
    s.custom_theme = phoenix_api::rest::settings::custom_user_theme(raw).and_then(|t| crate::theme::client_theme::CustomTheme::parse(&t.colors, t.angle, t.base_mix));
    Ok(s)
}

pub fn synced(p: &PreloadedUserSettings) -> SyncedSettings {
    let d = SyncedSettings::default();
    crate::ui::settings::account::set_activity_restricted(p.privacy.as_ref().map(|x| x.activity_restricted_guild_ids.clone()).unwrap_or_default());
    crate::ui::settings::account::set_dm_restricted(p.privacy.as_ref().map(|x| x.restricted_guild_ids.clone()).unwrap_or_default());
    SyncedSettings {
        theme: match p.theme() {
            Theme::Dark => AppTheme::Dark,
            Theme::Light => AppTheme::Light,
            Theme::Ash => AppTheme::Ash,
            Theme::Onyx => AppTheme::Onyx,
        },
        compact: p.compact(),
        developer_mode: p.developer_mode(),
        locale: p.locale().filter(|l| !l.is_empty()).map(|l| l.to_owned().into()).unwrap_or(d.locale),
        allow_server_dms: !p.default_guilds_restricted(),
        proto: Default::default(),
        rest: Default::default(),
        density: density_from_proto(p.ui_density()).unwrap_or(d.density),
        favorites: Default::default(),
        custom_theme: None,
        join_sounds: Default::default(),
        raid_alerts: Default::default(),
    }
}

fn density_from_proto(v: i32) -> Option<Density> {
    match v {
        1 => Some(Density::Compact),
        2 => Some(Density::Cozy),
        3 | 4 => Some(Density::Default),
        _ => None,
    }
}

fn density_to_proto(d: Density) -> i32 {
    match d {
        Density::Compact => 1,
        Density::Cozy => 2,
        Density::Default => 4,
    }
}

pub(super) fn on_gateway(s: &Session, ev: &GatewayEvent) {
    if let GatewayEvent::Unknown { name, data } = ev
        && (name.starts_with("STAGE_INSTANCE_") || name.starts_with("GUILD_SCHEDULED_EVENT_") || name == "EMBEDDED_ACTIVITY_UPDATE_V2")
    {
        crate::guild_live::on_event(name, data);
    }
    match ev {
        GatewayEvent::MessageCreate(m) => {
            let me = s.adapter.lock().me;
            crate::notifications::on_message(&s.notify.lock(), &s.cache, me, m);
            crate::sounds::on_message(&s.notify.lock(), &s.cache, me, m);
        }
        GatewayEvent::Unknown { name, data } if name == "USER_GUILD_SETTINGS_UPDATE" => s.notify.lock().update(data),
        GatewayEvent::Unknown { name, data } if name == "GUILD_SCHEDULED_EVENT_UPDATE" => {
            let me = s.adapter.lock().me;
            crate::notifications::on_scheduled_event_update(&s.cache, me, data);
        }
        GatewayEvent::Unknown { name, data } if name == "STAGE_INSTANCE_UPDATE" => {
            let me = s.adapter.lock().me;
            crate::notifications::on_stage_instance_update(&s.notify.lock(), &s.cache, me, data);
        }
        GatewayEvent::Unknown { name, data } if name == "STAGE_INSTANCE_DELETE" => {
            crate::notifications::on_stage_instance_delete(data);
        }
        GatewayEvent::Unknown { name, data } if name == "ACTIVITY_START" => crate::notifications::on_activity_start(&s.cache, data),
        GatewayEvent::Unknown { name, data } if name == "SAVED_MESSAGE_CREATE" => crate::notifications::reminders::on_saved_message_create(data),
        GatewayEvent::Unknown { name, data } if name == "SAVED_MESSAGE_DELETE" => crate::notifications::reminders::on_saved_message_delete(data),
        GatewayEvent::ThreadCreate(ch) => {
            let me = s.adapter.lock().me;
            crate::notifications::on_thread_create(&s.notify.lock(), &s.cache, me, ch);
        }
        GatewayEvent::Ready(_) => {
            crate::notifications::on_ready(&s.cache);
            for g in s.cache.guilds() {
                crate::guild_live::load(g.id.0, &g.stage_instances, &g.guild_scheduled_events, &g.activity_instances);
            }
        }
        GatewayEvent::ReadySupplemental(r) => {
            for g in &r.guilds {
                crate::guild_live::load_activities(g.id.0, &g.activity_instances);
            }
        }
        GatewayEvent::GuildDelete(g) => crate::guild_live::forget(g.id.0),
        GatewayEvent::UserUpdate(u) => crate::notifications::on_user_update(&s.cache, u),
        GatewayEvent::RelationshipAdd(r) => crate::notifications::on_relationship_add(r),
        GatewayEvent::RelationshipRemove(r) => crate::notifications::on_relationship_remove(r),
        GatewayEvent::MessageAck(a) => crate::notifications::on_ack(a.channel_id.0),
        GatewayEvent::GuildCreate(g) | GatewayEvent::GuildUpdate(g) => {
            if matches!(ev, GatewayEvent::GuildCreate(_)) {
                crate::guild_live::load(g.id.0, &g.stage_instances, &g.guild_scheduled_events, &g.activity_instances);
            }
            if let Some(d) = g.properties.default_message_notifications {
                s.notify.lock().set_default(g.id.0, crate::notifications::Level::from_default(Some(d)));
            }
        }
        _ => {}
    }
}

impl DiscordBackend {
    pub(super) fn auth_sessions_impl(&self) -> BoxFuture<Option<Vec<crate::ui::settings::sessions::Session>>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, "/auth/sessions", None).await.map_err(api_err)?;
            Ok(v.get("user_sessions").is_some().then(|| crate::ui::settings::sessions::parse(&v)))
        })
    }

    pub(super) fn deauthorize_app_impl(&self, token_id: u64) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let (_, path) = crate::ui::settings::authorized_apps::deauth_request(token_id);
            s.rest.request_json(reqwest::Method::DELETE, &path, None).await.map_err(api_err)?;
            Ok(())
        })
    }

    pub(super) fn account_standing_impl(&self) -> BoxFuture<Option<u64>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, "/safety-hub/@me", None).await.map_err(api_err)?;
            Ok(v.get("account_standing").and_then(|a| a.get("state")).and_then(serde_json::Value::as_u64))
        })
    }

    pub(super) fn logout_sessions_impl(&self, hashes: Vec<String>) -> BoxFuture<()> {
        self.with_session(move |_, s| async move {
            let Some((_, path, body)) = crate::ui::settings::sessions::logout_request(&hashes) else { return Ok(()) };
            s.rest.request_json(reqwest::Method::POST, path, Some(body)).await.map_err(api_err)?;
            Ok(())
        })
    }
}

impl DiscordBackend {
    pub(super) fn billing_impl(&self) -> BoxFuture<Option<crate::ui::settings::billing::Billing>> {
        use crate::ui::settings::billing as b;
        self.with_session(move |_, s| async move {
            let get = |p: &'static str| {
                let rest = s.rest.clone();
                async move { rest.request_json(reqwest::Method::GET, p, None).await.ok() }
            };
            let raw_sources = get("/users/@me/billing/payment-sources").await.unwrap_or(serde_json::Value::Null);
            let sources = b::mark_default(b::parse_sources(&raw_sources));
            let payments = get("/users/@me/billing/payments?limit=10").await.map(|v| b::parse_payments(&v)).unwrap_or_default();
            let code = get("/users/@me/billing/country-code")
                .await
                .and_then(|v| v.get("country_code").and_then(serde_json::Value::as_str).map(str::to_owned));
            let country = b::source_country(&raw_sources, code);
            Ok(Some(b::Billing { country, sources, payments }))
        })
    }

    pub(super) fn subscriptions_impl(&self) -> BoxFuture<Option<crate::ui::settings::subscriptions::Subs>> {
        use crate::ui::settings::subscriptions as sub;
        self.with_session(move |_, s| async move {
            let get = |p: String| {
                let rest = s.rest.clone();
                async move { rest.request_json(reqwest::Method::GET, &p, None).await.ok() }
            };
            let me = s.cache.current_user().map(|u| u.id.0).unwrap_or(0);
            let mut squad = None;
            if let Some(m) = get("/users/@me/premium-group/membership".into()).await
                && let Some(id) = m.get("subscription_id").and_then(serde_json::Value::as_str)
                && let Some(v) = get(format!("/users/@me/billing/subscriptions/{id}/members")).await
            {
                squad = Some(sub::parse_members(&v, me));
            }
            let credits = get(format!("/users/@me/applications/{}/entitlements?exclude_consumed=true", sub::NITRO_APP))
                .await
                .map(|v| sub::parse_credits(&v))
                .unwrap_or_default();
            let personal = get("/users/@me/billing/subscriptions".into()).await
                .and_then(|value| sub::parse_subscriptions(&value));
            Ok(Some(sub::Subs { squad, credits, personal }))
        })
    }
}

impl DiscordBackend {
    pub(super) fn guild_affinities_impl(&self) -> BoxFuture<Vec<crate::model::Id>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, "/users/@me/affinities/guilds", None).await.map_err(api_err)?;
            Ok(parse_guild_affinities(&v))
        })
    }

    pub(super) fn user_affinities_impl(&self) -> BoxFuture<Vec<crate::ui::home::friends::Affinity>> {
        self.with_session(move |_, s| async move {
            let consent = s.rest.get_consent().await.map_err(api_err)?;
            if !crate::ui::home::friends::personalization_consented(&consent) {
                return Ok(Vec::new());
            }
            let v = s.rest.request_json(reqwest::Method::GET, "/users/@me/affinities/v2/users", None).await.map_err(api_err)?;
            Ok(crate::ui::home::friends::parse_affinities(&v))
        })
    }

    pub(super) fn authorized_apps_impl(&self) -> BoxFuture<Option<Vec<crate::ui::settings::authorized_apps::AuthorizedApp>>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, "/oauth2/tokens", None).await.map_err(api_err)?;
            Ok(Some(crate::ui::settings::authorized_apps::parse(&v)))
        })
    }
    pub(super) fn user_connections_impl(&self) -> BoxFuture<Option<Vec<crate::ui::settings::connections::LinkedAccount>>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, "/users/@me/connections", None).await.map_err(api_err)?;
            Ok(Some(crate::ui::settings::connections::parse(&v)))
        })
    }

    pub(super) fn connection_authorize_impl(&self, path: String) -> BoxFuture<Option<String>> {
        self.with_session(move |_, s| async move {
            let v = s.rest.request_json(reqwest::Method::GET, &path, None).await.map_err(api_err)?;
            Ok(v.get("url").and_then(serde_json::Value::as_str).filter(|u| u.starts_with("https://")).map(str::to_owned))
        })
    }
}

pub(crate) fn parse_guild_affinities(v: &serde_json::Value) -> Vec<crate::model::Id> {
    v.get("guild_affinities")
        .and_then(serde_json::Value::as_array)
        .map(|a| a.iter().filter_map(|e| e.get("guild_id")?.as_str()?.parse().ok().map(crate::model::Id)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod affinity_tests {
    #[test]
    fn keeps_the_server_order() {
        let v = serde_json::json!({"guild_affinities": [{"guild_id": "30", "affinity": 1.5}, {"guild_id": "10", "affinity": 9.0}, {"affinity": 2.0}]});
        assert_eq!(super::parse_guild_affinities(&v), vec![crate::model::Id(30), crate::model::Id(10)]);
    }
}

#[cfg(test)]
mod mfa_tests {
    use super::*;
    use phoenix_api::error::ApiErrorBody;

    fn api(status: u16, code: u32, message: &str) -> ApiError {
        ApiError::Api { status, body: Box::new(ApiErrorBody { code, message: message.into(), ..Default::default() }), raw: String::new() }
    }

    #[test]
    fn login_mfa_errors_show_what_the_client_shows() {
        assert_eq!(mfa_err(api(400, 60008, "Invalid two-factor code")).0, "Invalid two-factor code");
        assert_eq!(mfa_err(api(400, 60006, "Invalid two-factor auth ticket")).0, "Invalid two-factor auth ticket");
        assert_eq!(mfa_err(api(401, 0, "")).0, crate::t!("login_mfa.error_generic").to_string());
        let limited = mfa_err(ApiError::RateLimited { retry_after: std::time::Duration::from_secs(30), global: false });
        assert!(limited.0.contains("30"));
        let captcha = mfa_err(ApiError::CaptchaRequired { sitekey: None, body: Box::default() });
        assert!(captcha.0.contains("captcha"));
    }

    #[test]
    fn invalid_form_body_shows_the_field_messages_not_the_generic_one() {
        let body: ApiErrorBody = serde_json::from_str(
            r#"{"message":"Corps de formulaire non valide","code":50035,"errors":{"login":{"_errors":[{"code":"INVALID_LOGIN","message":"Identifiant ou mot de passe invalide."}]},"password":{"_errors":[{"code":"INVALID_LOGIN","message":"Identifiant ou mot de passe invalide."}]}}}"#,
        )
        .unwrap();
        let e = super::super::login_err(ApiError::Api { status: 400, body: Box::new(body), raw: String::new() });
        let (login, password) = BackendError::field_errors(&e.0).unwrap();
        assert_eq!(login, Some("Identifiant ou mot de passe invalide."));
        assert_eq!(password, Some("Identifiant ou mot de passe invalide."));
        let only_pw: ApiErrorBody = serde_json::from_str(
            r#"{"message":"Invalid Form Body","code":50035,"errors":{"password":{"_errors":[{"code":"BASE_TYPE_REQUIRED","message":"This field is required"}]}}}"#,
        )
        .unwrap();
        let e = super::super::login_err(ApiError::Api { status: 400, body: Box::new(only_pw), raw: String::new() });
        assert_eq!(BackendError::field_errors(&e.0), Some((None, Some("This field is required"))));
        let bare = super::super::login_err(api(400, 50035, "Invalid Form Body"));
        assert!(BackendError::field_errors(&bare.0).is_none());
        assert_eq!(bare.0, "Invalid Form Body");
        let captcha = super::super::login_err(ApiError::CaptchaRequired { sitekey: Some("k".into()), body: Box::default() });
        assert!(!captcha.0.contains("Some(") && !captcha.0.contains("sitekey"));
    }
}
