use gpui::{Context, SharedString};

use phoenix_api::rest::auth::LoginMfaType;

use crate::{app::PhoenixApp, backend::ReadyData};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MfaOptions {
    pub totp: bool,
    pub sms: bool,
    pub backup: bool,
    pub webauthn: bool,
}

impl MfaOptions {
    pub fn methods(self) -> Vec<MfaMethod> {
        [(self.webauthn, MfaMethod::Webauthn), (self.totp, MfaMethod::Totp), (self.backup, MfaMethod::Backup), (self.sms, MfaMethod::Sms)]
            .into_iter()
            .filter_map(|(on, m)| on.then_some(m))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MfaMethod {
    Webauthn,
    Totp,
    Backup,
    Sms,
}

impl MfaMethod {
    pub fn api(self) -> LoginMfaType {
        match self {
            Self::Webauthn => LoginMfaType::Webauthn,
            Self::Totp => LoginMfaType::Totp,
            Self::Backup => LoginMfaType::Backup,
            Self::Sms => LoginMfaType::Sms,
        }
    }

    pub fn choice_key(self) -> &'static str {
        match self {
            Self::Webauthn => "login_mfa.choice_webauthn",
            Self::Totp => "login_mfa.choice_totp",
            Self::Backup => "login_mfa.choice_backup",
            Self::Sms => "login_mfa.choice_sms",
        }
    }

    pub fn slide(self) -> MfaSlide {
        match self {
            Self::Webauthn => MfaSlide::Webauthn,
            Self::Totp => MfaSlide::Totp,
            Self::Backup => MfaSlide::Backup,
            Self::Sms => MfaSlide::Sms,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MfaSlide {
    #[default]
    Select,
    Webauthn,
    Totp,
    Sms,
    Backup,
    BackupAck,
}

pub const BACKUP_WAIT: u8 = 10;

impl MfaSlide {
    pub fn max_len(self) -> Option<usize> {
        match self {
            Self::Totp => Some(6),
            Self::Sms => Some(10),
            Self::BackupAck => Some(11),
            _ => None,
        }
    }

    pub fn has_input(self) -> bool {
        self.max_len().is_some()
    }

    pub fn placeholder_key(self) -> &'static str {
        match self {
            Self::BackupAck => "login_mfa.backup_placeholder",
            _ => "login_mfa.code_placeholder",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::BackupAck => "login_mfa.backup_label",
            _ => "login_mfa.code_label",
        }
    }

    pub fn can_submit(self, code: &str) -> bool {
        let n = code.encode_utf16().count();
        match self {
            Self::Totp | Self::Sms => n == 6,
            Self::BackupAck => n >= 8,
            _ => false,
        }
    }

    pub fn payload(self, code: &str) -> Option<(MfaMethod, String)> {
        if !self.can_submit(code) {
            return None;
        }
        match self {
            Self::Totp => Some((MfaMethod::Totp, code.to_owned())),
            Self::Sms => Some((MfaMethod::Sms, code.to_owned())),
            Self::BackupAck => Some((MfaMethod::Backup, code.replace('-', ""))),
            _ => None,
        }
    }
}

pub fn clamp_code(code: &str, max: usize) -> Option<String> {
    let mut units = 0;
    let mut out = String::new();
    for ch in code.chars() {
        units += ch.len_utf16();
        if units > max {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

#[derive(Default)]
pub struct MfaFlow {
    pub methods: Vec<MfaMethod>,
    pub slide: MfaSlide,
    pub error: Option<SharedString>,
    pub submitting: bool,
    pub sms_phone: Option<SharedString>,
    pub sms_sending: bool,
    pub backup_wait: u8,
    pub pending_input: bool,
    generation: u64,
}

impl MfaFlow {
    pub fn new(opts: MfaOptions) -> Self {
        let methods = opts.methods();
        let slide = methods.first().map_or(MfaSlide::Select, |m| m.slide());
        Self { methods, slide, ..Default::default() }
    }

    pub fn can_choose(&self) -> bool {
        self.methods.len() > 1
    }

    fn enter(&mut self, slide: MfaSlide) {
        self.slide = slide;
        self.error = None;
        self.submitting = false;
        self.sms_phone = None;
        self.sms_sending = false;
        self.backup_wait = if slide == MfaSlide::Backup { BACKUP_WAIT } else { 0 };
        self.pending_input = slide.has_input();
        self.generation += 1;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RemoteAuthUpdate {
    Qr(String),
    Scanned { username: String, avatar: String },
    Ended(Option<String>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum QrState {
    #[default]
    Idle,
    Connecting,
    Code {
        url: String,
        modules: Vec<bool>,
        size: usize,
    },
    Scanned { name: String, avatar: String },
    Unavailable(String),
}

#[derive(Default)]
pub struct LoginUi {
    pub mfa: MfaFlow,
    pub qr: QrState,
    pub qr_backoff_ms: u64,
    pub qr_retry_gen: u64,
    pub reveal_password: bool,
}

pub const QR_BACKOFF_MIN_MS: u64 = 1500;
pub const QR_BACKOFF_MAX_MS: u64 = 30_000;

pub fn next_backoff(prev: u64) -> u64 {
    if prev == 0 { QR_BACKOFF_MIN_MS } else { (prev * 2).min(QR_BACKOFF_MAX_MS) }
}

pub fn scanned_avatar(user_id: &str, avatar: Option<&str>) -> String {
    match avatar.filter(|a| !a.is_empty() && *a != "0") {
        Some(hash) => format!("https://cdn.discordapp.com/avatars/{user_id}/{hash}.png?size=128"),
        None => {
            let ix = user_id.parse::<u64>().map_or(0, |id| (id >> 22) % 6);
            format!("https://cdn.discordapp.com/embed/avatars/{ix}.png")
        }
    }
}

pub fn qr_matrix(url: &str) -> Option<(Vec<bool>, usize)> {
    let code = (1..=40).find_map(|v| {
        let mut bits = qrcode::bits::Bits::new(qrcode::Version::Normal(v));
        bits.push_byte_data(url.as_bytes()).ok()?;
        bits.push_terminator(qrcode::EcLevel::M).ok()?;
        qrcode::QrCode::with_bits(bits, qrcode::EcLevel::M).ok()
    })?;
    let size = code.width();
    let modules = code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect();
    Some((modules, size))
}

pub fn ensure_qr(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if app.screen != crate::app::Screen::Login || app.logging_in || app.login_mfa_step || app.login_ui.qr != QrState::Idle {
        return;
    }
    let Some(fut) = app.backend.remote_auth() else {
        app.login_ui.qr = QrState::Unavailable("QR login isn't available offline.".into());
        return;
    };
    app.login_ui.qr = QrState::Connecting;
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| finish(this, res, cx));
    })
    .detach();
}

fn finish(app: &mut PhoenixApp, res: Result<ReadyData, crate::backend::BackendError>, cx: &mut Context<PhoenixApp>) {
    match res {
        Ok(ready) => {
            app.login_ui.qr = QrState::Idle;
            app.login_error = None;
            app.apply_ready(ready, cx);
        }
        Err(e) if e.0.is_empty() => {
            if !matches!(app.login_ui.qr, QrState::Unavailable(_)) {
                app.login_ui.qr = QrState::Idle;
            }
        }
        Err(e) => {
            app.login_ui.qr = QrState::Unavailable(e.0);
            schedule_qr_retry(app, cx);
        }
    }
    cx.notify();
}

fn schedule_qr_retry(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let delay = next_backoff(app.login_ui.qr_backoff_ms);
    app.login_ui.qr_backoff_ms = delay;
    app.login_ui.qr_retry_gen += 1;
    let generation = app.login_ui.qr_retry_gen;
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(delay)).await;
        let _ = this.update(cx, |this, cx| {
            if this.login_ui.qr_retry_gen == generation && matches!(this.login_ui.qr, QrState::Unavailable(_)) {
                this.login_ui.qr = QrState::Idle;
                cx.notify();
            }
        });
    })
    .detach();
}

pub fn on_remote_auth(app: &mut PhoenixApp, u: RemoteAuthUpdate, cx: &mut Context<PhoenixApp>) {
    app.login_ui.qr = match u {
        RemoteAuthUpdate::Qr(url) => match qr_matrix(&url) {
            Some((modules, size)) => {
                app.login_ui.qr_backoff_ms = 0;
                QrState::Code { url, modules, size }
            }
            None => QrState::Unavailable("Could not draw the QR code.".into()),
        },
        RemoteAuthUpdate::Scanned { username, avatar } => QrState::Scanned { name: username, avatar },
        RemoteAuthUpdate::Ended(Some(msg)) => QrState::Unavailable(msg),
        RemoteAuthUpdate::Ended(None) => return,
    };
    if matches!(app.login_ui.qr, QrState::Unavailable(_)) {
        schedule_qr_retry(app, cx);
    }
}

pub fn retry_qr(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    app.backend.cancel_remote_auth();
    app.login_ui.qr = QrState::Idle;
    cx.notify();
}

pub fn start_mfa(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    app.backend.cancel_remote_auth();
    app.login_ui.qr = QrState::Idle;
    app.login_error = None;
    app.login_mfa_step = true;
    let flow = MfaFlow::new(app.backend.mfa_options());
    let slide = flow.slide;
    app.login_ui.mfa = flow;
    set_slide(app, slide, cx);
}

pub fn set_slide(app: &mut PhoenixApp, slide: MfaSlide, cx: &mut Context<PhoenixApp>) {
    app.login_ui.mfa.enter(slide);
    match slide {
        MfaSlide::Sms => send_sms(app, true, cx),
        MfaSlide::Backup => tick_backup(app, cx),
        _ => {}
    }
    cx.notify();
}

fn tick_backup(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let generation = app.login_ui.mfa.generation;
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
            let more = this
                .update(cx, |this, cx| {
                    let flow = &mut this.login_ui.mfa;
                    if flow.generation != generation || flow.slide != MfaSlide::Backup || flow.backup_wait == 0 {
                        return false;
                    }
                    flow.backup_wait -= 1;
                    cx.notify();
                    flow.backup_wait > 0
                })
                .unwrap_or(false);
            if !more {
                break;
            }
        }
    })
    .detach();
}

pub fn send_sms(app: &mut PhoenixApp, initial: bool, cx: &mut Context<PhoenixApp>) {
    let generation = app.login_ui.mfa.generation;
    if initial {
        app.login_ui.mfa.sms_sending = true;
    }
    let fut = app.backend.send_mfa_sms();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| {
            let flow = &mut this.login_ui.mfa;
            if flow.generation != generation {
                return;
            }
            if initial {
                flow.sms_sending = false;
            }
            match res {
                Ok(phone) => flow.sms_phone = Some(phone.into()),
                Err(e) => flow.error = Some(e.0.into()),
            }
            cx.notify();
        });
    })
    .detach();
}

pub fn sync(app: &mut PhoenixApp, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    if !app.login_mfa_step || !app.login_ui.mfa.pending_input {
        return;
    }
    app.login_ui.mfa.pending_input = false;
    let placeholder = crate::i18n::t(app.login_ui.mfa.slide.placeholder_key(), &[]);
    app.login_mfa.update(cx, |s, cx| {
        s.set_value("", window, cx);
        s.set_placeholder(placeholder, window, cx);
        s.focus(window, cx);
    });
}

pub fn on_code_change(app: &mut PhoenixApp, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) {
    let Some(max) = app.login_ui.mfa.slide.max_len() else { return };
    let value = app.login_mfa.read(cx).value().to_string();
    if let Some(cut) = clamp_code(&value, max) {
        app.login_mfa.update(cx, |s, cx| s.set_value(cut, window, cx));
    }
    if app.login_ui.mfa.slide == MfaSlide::BackupAck {
        app.login_ui.mfa.error = None;
    }
    cx.notify();
}

pub fn go_back(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    app.backend.reset_mfa();
    app.login_mfa_step = false;
    app.login_ui.mfa = MfaFlow::default();
    app.login_error = None;
    app.logging_in = false;
    cx.notify();
}

pub fn webauthn(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    app.login_ui.mfa.error = Some(crate::t!("login_mfa.error_generic"));
    cx.notify();
}

pub fn submit_mfa(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let flow = &app.login_ui.mfa;
    if flow.submitting {
        return;
    }
    let code = app.login_mfa.read(cx).value().to_string();
    let Some((method, data)) = flow.slide.payload(&code) else { return };
    let generation = flow.generation;
    let fut = app.backend.login_mfa(method, data);
    app.login_ui.mfa.submitting = true;
    app.login_ui.mfa.error = None;
    cx.notify();
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let _ = this.update(cx, |this, cx| {
            match res {
                Ok(ready) => {
                    this.login_mfa_step = false;
                    this.login_ui = LoginUi::default();
                    this.backend.cancel_remote_auth();
                    this.apply_ready(ready, cx);
                }
                Err(e) if this.login_ui.mfa.generation == generation => {
                    this.login_ui.mfa.submitting = false;
                    this.login_ui.mfa.error = Some(e.0.into());
                }
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_follow_the_client_order() {
        let all = MfaOptions { totp: true, sms: true, backup: true, webauthn: true };
        assert_eq!(all.methods(), vec![MfaMethod::Webauthn, MfaMethod::Totp, MfaMethod::Backup, MfaMethod::Sms]);
        let flow = MfaFlow::new(MfaOptions { totp: true, backup: true, ..Default::default() });
        assert_eq!(flow.slide, MfaSlide::Totp);
        assert!(flow.can_choose());
        let key = MfaFlow::new(MfaOptions { webauthn: true, ..Default::default() });
        assert_eq!(key.slide, MfaSlide::Webauthn);
        assert!(!key.can_choose());
        assert_eq!(MfaFlow::new(MfaOptions::default()).slide, MfaSlide::Select);
        assert_eq!(MfaMethod::Backup.api().api_name(), "backup");
    }

    #[test]
    fn codes_follow_the_client_limits() {
        assert!(MfaSlide::Totp.can_submit("123456"));
        assert!(!MfaSlide::Totp.can_submit("12345"));
        assert!(!MfaSlide::Totp.can_submit("123 456"));
        assert!(MfaSlide::Sms.can_submit("654321"));
        assert!(!MfaSlide::Sms.can_submit("6543210"));
        assert!(!MfaSlide::BackupAck.can_submit("abcd123"));
        assert_eq!(MfaSlide::BackupAck.payload("abcd-1234"), Some((MfaMethod::Backup, "abcd1234".into())));
        assert_eq!(MfaSlide::Totp.payload("123456"), Some((MfaMethod::Totp, "123456".into())));
        assert_eq!(MfaSlide::Backup.payload("abcd1234"), None);
        assert_eq!(MfaSlide::Webauthn.payload("x"), None);
        assert_eq!(clamp_code("123 456", 6).as_deref(), Some("123 45"));
        assert_eq!(clamp_code("123456", 6), None);
        assert_eq!(clamp_code("abcd-1234-x", 11), None);
        assert_eq!(clamp_code("abcd-1234-xy", 11).as_deref(), Some("abcd-1234-x"));
        assert_eq!(MfaSlide::Totp.max_len(), Some(6));
        assert_eq!(MfaSlide::Sms.max_len(), Some(10));
        assert_eq!(MfaSlide::Select.max_len(), None);
    }

    #[test]
    fn entering_a_slide_resets_its_state() {
        let mut flow = MfaFlow::new(MfaOptions { totp: true, backup: true, ..Default::default() });
        flow.error = Some("Invalid two-factor code".into());
        flow.submitting = true;
        flow.enter(MfaSlide::Backup);
        assert_eq!((flow.error.clone(), flow.submitting, flow.backup_wait, flow.pending_input), (None, false, BACKUP_WAIT, false));
        let g = flow.generation;
        flow.enter(MfaSlide::BackupAck);
        assert_eq!((flow.backup_wait, flow.pending_input), (0, true));
        assert!(flow.generation > g);
    }

    #[test]
    fn qr_matrix_is_square_with_finders() {
        let (m, n) = qr_matrix("https://discord.com/ra/abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG").unwrap();
        assert_eq!(m.len(), n * n);
        assert!(n >= 21);
        assert!(m[0] && !m[n + 1] && m[2 * n + 2]);
    }

    #[test]
    fn qr_matches_the_client_level_m_grid() {
        let (_, n) = qr_matrix("https://discord.com/ra/abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG").unwrap();
        assert_eq!(n, 37);
    }

    #[test]
    fn qr_retry_backs_off_like_the_client() {
        let mut d = 0;
        let seq: Vec<u64> = (0..7).map(|_| { d = next_backoff(d); d }).collect();
        assert_eq!(seq, vec![1500, 3000, 6000, 12_000, 24_000, 30_000, 30_000]);
    }

    #[test]
    fn scanned_avatar_falls_back_to_the_default_avatar() {
        assert_eq!(scanned_avatar("80351110224678912", Some("abc")), "https://cdn.discordapp.com/avatars/80351110224678912/abc.png?size=128");
        assert_eq!(scanned_avatar("80351110224678912", None), format!("https://cdn.discordapp.com/embed/avatars/{}.png", (80351110224678912u64 >> 22) % 6));
        assert!(scanned_avatar("1", Some("0")).contains("/embed/avatars/"));
    }
}
