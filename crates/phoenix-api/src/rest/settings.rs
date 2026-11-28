use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::{Error, Result};

#[derive(Clone, PartialEq, prost::Message)]
pub struct BoolValue {
    #[prost(bool, tag = "1")]
    pub value: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StringValue {
    #[prost(string, tag = "1")]
    pub value: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TextAndImagesSettings {
    #[prost(message, optional, tag = "10")]
    pub inline_embed_media: Option<BoolValue>,
    #[prost(message, optional, tag = "11")]
    pub gif_auto_play: Option<BoolValue>,
    #[prost(message, optional, tag = "14")]
    pub animate_emoji: Option<BoolValue>,
    #[prost(message, optional, tag = "17")]
    pub message_display_compact: Option<BoolValue>,
    #[prost(message, optional, tag = "21")]
    pub convert_emoticons: Option<BoolValue>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct NotificationSettings {
    #[prost(message, optional, tag = "1")]
    pub show_in_app_notifications: Option<BoolValue>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PrivacySettings {
    #[prost(fixed64, repeated, tag = "3")]
    pub restricted_guild_ids: Vec<u64>,
    #[prost(bool, tag = "4")]
    pub default_guilds_restricted: bool,
    #[prost(fixed64, repeated, tag = "13")]
    pub activity_restricted_guild_ids: Vec<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StatusSettings {
    #[prost(message, optional, tag = "1")]
    pub status: Option<StringValue>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LocalizationSettings {
    #[prost(message, optional, tag = "1")]
    pub locale: Option<StringValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Ash,
    Onyx,
}

impl Theme {
    pub fn from_proto(v: i32) -> Self {
        match v {
            2 => Self::Light,
            3 => Self::Ash,
            4 => Self::Onyx,
            _ => Self::Dark,
        }
    }
    pub fn to_proto(self) -> i32 {
        match self {
            Self::Dark => 1,
            Self::Light => 2,
            Self::Ash => 3,
            Self::Onyx => 4,
        }
    }
    pub fn from_json(s: &str) -> Self {
        match s {
            "light" => Self::Light,
            "darker" => Self::Ash,
            "midnight" => Self::Onyx,
            _ => Self::Dark,
        }
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AppearanceSettings {
    #[prost(int32, tag = "1")]
    pub theme: i32,
    #[prost(bool, tag = "2")]
    pub developer_mode: bool,
    #[prost(int32, tag = "12")]
    pub ui_density: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PreloadedUserSettings {
    #[prost(message, optional, tag = "6")]
    pub text_and_images: Option<TextAndImagesSettings>,
    #[prost(message, optional, tag = "7")]
    pub notifications: Option<NotificationSettings>,
    #[prost(message, optional, tag = "8")]
    pub privacy: Option<PrivacySettings>,
    #[prost(message, optional, tag = "11")]
    pub status: Option<StatusSettings>,
    #[prost(message, optional, tag = "12")]
    pub localization: Option<LocalizationSettings>,
    #[prost(message, optional, tag = "13")]
    pub appearance: Option<AppearanceSettings>,
}

fn b(v: &Option<BoolValue>) -> Option<bool> {
    v.as_ref().map(|b| b.value)
}

impl PreloadedUserSettings {
    pub fn decode_base64(s: &str) -> Result<Self> {
        let bytes = STANDARD.decode(s.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))?;
        <Self as prost::Message>::decode(bytes.as_slice()).map_err(|e| Error::Protocol(format!("settings proto: {e}")))
    }
    pub fn decode_raw(bytes: &[u8]) -> Result<Self> {
        <Self as prost::Message>::decode(bytes).map_err(|e| Error::Protocol(format!("settings proto: {e}")))
    }
    pub fn encode_base64(&self) -> String {
        STANDARD.encode(prost::Message::encode_to_vec(self))
    }

    pub fn theme(&self) -> Theme {
        self.appearance.as_ref().map(|a| Theme::from_proto(a.theme)).unwrap_or_default()
    }
    pub fn developer_mode(&self) -> bool {
        self.appearance.as_ref().is_some_and(|a| a.developer_mode)
    }
    pub fn ui_density(&self) -> i32 {
        self.appearance.as_ref().map_or(0, |a| a.ui_density)
    }
    pub fn compact(&self) -> bool {
        self.text_and_images.as_ref().and_then(|t| b(&t.message_display_compact)).unwrap_or(false)
    }
    pub fn locale(&self) -> Option<&str> {
        self.localization.as_ref()?.locale.as_ref().map(|s| s.value.as_str())
    }
    pub fn status(&self) -> Option<&str> {
        self.status.as_ref()?.status.as_ref().map(|s| s.value.as_str())
    }
    pub fn default_guilds_restricted(&self) -> bool {
        self.privacy.as_ref().is_some_and(|p| p.default_guilds_restricted)
    }

    pub fn patch_appearance(theme: Theme, developer_mode: bool) -> Self {
        Self {
            appearance: Some(AppearanceSettings { theme: theme.to_proto(), developer_mode, ui_density: 0 }),
            ..Default::default()
        }
    }
    pub fn patch_compact(compact: bool) -> Self {
        Self {
            text_and_images: Some(TextAndImagesSettings {
                message_display_compact: Some(BoolValue { value: compact }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    pub fn patch_locale(locale: &str) -> Self {
        Self {
            localization: Some(LocalizationSettings { locale: Some(StringValue { value: locale.to_owned() }) }),
            ..Default::default()
        }
    }
    pub fn patch_default_guilds_restricted(v: bool) -> Self {
        Self {
            privacy: Some(PrivacySettings { default_guilds_restricted: v, ..Default::default() }),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingChange {
    Theme(Theme),
    DeveloperMode(bool),
    Compact(bool),
    Locale(String),
    DefaultGuildsRestricted(bool),
    UiDensity(i32),
    Path { path: Vec<u32>, wrapper: bool, value: ProtoValue },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtoValue {
    Int(u64),
    Fixed32(u32),
    Str(String),
}

fn read_varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte < 0x80 {
            return Some(v);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

pub(crate) fn fields(b: &[u8]) -> Vec<(u32, u8, u64, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let Some(key) = read_varint(b, &mut i) else { break };
        let (tag, wt) = ((key >> 3) as u32, (key & 7) as u8);
        match wt {
            0 => match read_varint(b, &mut i) {
                Some(v) => out.push((tag, 0, v, &b[0..0])),
                None => break,
            },
            1 | 5 => {
                let n = if wt == 1 { 8 } else { 4 };
                let Some(raw) = b.get(i..i + n) else { break };
                let mut v = 0u64;
                for (k, x) in raw.iter().enumerate() {
                    v |= (*x as u64) << (8 * k);
                }
                out.push((tag, wt, v, &b[0..0]));
                i += n;
            }
            2 => {
                let Some(n) = read_varint(b, &mut i) else { break };
                let Some(d) = usize::try_from(n).ok().and_then(|n| b.get(i..i.checked_add(n)?)) else { break };
                out.push((tag, 2, 0, d));
                i += d.len();
            }
            _ => break,
        }
    }
    out
}

pub fn proto_read(raw: &[u8], path: &[u32], wrapper: bool, as_str: bool) -> Option<ProtoValue> {
    let (&first, rest) = path.split_first()?;
    let mut found = None;
    for (tag, wt, v, d) in fields(raw) {
        if tag != first {
            continue;
        }
        let got = if !rest.is_empty() {
            if wt != 2 {
                continue;
            }
            proto_read(d, rest, wrapper, as_str)
        } else if wrapper {
            if wt != 2 {
                continue;
            }
            Some(proto_read(d, &[1], false, as_str).unwrap_or(if as_str { ProtoValue::Str(String::new()) } else { ProtoValue::Int(0) }))
        } else if as_str {
            (wt == 2).then(|| ProtoValue::Str(String::from_utf8_lossy(d).into_owned()))
        } else {
            (wt != 2).then_some(ProtoValue::Int(v))
        };
        if got.is_some() {
            found = got;
        }
    }
    found
}

fn varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn field_varint(tag: u32, v: u64, out: &mut Vec<u8>) {
    varint((tag as u64) << 3, out);
    varint(v, out);
}

pub(crate) fn field_bytes(tag: u32, data: &[u8], out: &mut Vec<u8>) {
    varint(((tag as u64) << 3) | 2, out);
    varint(data.len() as u64, out);
    out.extend_from_slice(data);
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderProto {
    pub guilds: Vec<u64>,
    pub id: Option<u64>,
    pub name: Option<String>,
    pub color: Option<u64>,
}

fn encode_folder(f: &FolderProto) -> Vec<u8> {
    let mut o = Vec::new();
    let ids: Vec<u8> = f.guilds.iter().flat_map(|g| g.to_le_bytes()).collect();
    field_bytes(1, &ids, &mut o);
    if let Some(id) = f.id {
        let mut w = Vec::new();
        field_varint(1, id, &mut w);
        field_bytes(2, &w, &mut o);
    }
    if let Some(n) = f.name.as_deref().filter(|n| !n.is_empty()) {
        let mut w = Vec::new();
        field_bytes(1, n.as_bytes(), &mut w);
        field_bytes(3, &w, &mut o);
    }
    if let Some(c) = f.color {
        let mut w = Vec::new();
        field_varint(1, c, &mut w);
        field_bytes(4, &w, &mut o);
    }
    o
}

fn reencode(tag: u32, wt: u8, v: u64, d: &[u8], out: &mut Vec<u8>) {
    match wt {
        0 => field_varint(tag, v, out),
        1 => {
            varint(((tag as u64) << 3) | 1, out);
            out.extend_from_slice(&v.to_le_bytes());
        }
        5 => {
            varint(((tag as u64) << 3) | 5, out);
            out.extend_from_slice(&(v as u32).to_le_bytes());
        }
        _ => field_bytes(tag, d, out),
    }
}

pub fn with_guild_folders(raw: &[u8], folders: &[FolderProto]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut kept = Vec::new();
    for (tag, wt, v, d) in fields(raw) {
        if tag == 14 && wt == 2 {
            for (t, w, x, dd) in fields(d) {
                if t != 1 {
                    reencode(t, w, x, dd, &mut kept);
                }
            }
            continue;
        }
        reencode(tag, wt, v, d, &mut out);
    }
    let mut gf = Vec::new();
    for f in folders {
        field_bytes(1, &encode_folder(f), &mut gf);
    }
    gf.extend(kept);
    field_bytes(14, &gf, &mut out);
    out
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomStatusProto {
    pub text: String,
    pub emoji_id: u64,
    pub emoji_name: String,
    pub expires_at_ms: u64,
    pub created_at_ms: u64,
}

fn field_fixed64(tag: u32, v: u64, out: &mut Vec<u8>) {
    varint(((tag as u64) << 3) | 1, out);
    out.extend_from_slice(&v.to_le_bytes());
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RaidAlerts {
    pub push_off: bool,
    pub nag_off: bool,
}

fn guild_entries(raw: &[u8]) -> Vec<(u64, Vec<u8>)> {
    let mut out: Vec<(u64, Vec<u8>)> = Vec::new();
    for (_, _, _, all) in fields(raw).into_iter().filter(|f| f.0 == 3 && f.1 == 2) {
        for (_, _, _, entry) in fields(all).into_iter().filter(|f| f.0 == 1 && f.1 == 2) {
            let mut key = None;
            let mut value = Vec::new();
            for (t, w, v, d) in fields(entry) {
                match (t, w) {
                    (1, 1) => key = Some(v),
                    (2, 2) => value.extend_from_slice(d),
                    _ => {}
                }
            }
            let Some(k) = key else { continue };
            match out.iter_mut().find(|e| e.0 == k) {
                Some(e) => e.1.extend_from_slice(&value),
                None => out.push((k, value)),
            }
        }
    }
    out
}

pub fn guild_raid_alerts(raw: &[u8]) -> std::collections::HashMap<u64, RaidAlerts> {
    guild_entries(raw)
        .into_iter()
        .map(|(k, v)| {
            let flag = |tag: u32| fields(&v).iter().rev().find(|f| f.0 == tag && f.1 == 0).is_some_and(|f| f.2 != 0);
            (k, RaidAlerts { push_off: flag(8), nag_off: flag(9) })
        })
        .collect()
}

pub fn with_guild_raid_alerts(raw: &[u8], guild: u64, alerts: RaidAlerts) -> Vec<u8> {
    update_guild_settings(raw, guild, |value| {
        let mut out = Vec::new();
        for (tag, wire, number, data) in fields(value) {
            if !matches!(tag, 8 | 9) {
                reencode(tag, wire, number, data, &mut out);
            }
        }
        if alerts.push_off {
            field_varint(8, 1, &mut out);
        }
        if alerts.nag_off {
            field_varint(9, 1, &mut out);
        }
        out
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoinSound {
    pub sound: u64,
    pub source_guild: u64,
}

pub fn guild_join_sounds(raw: &[u8]) -> std::collections::HashMap<u64, JoinSound> {
    guild_entries(raw)
        .into_iter()
        .filter_map(|(guild, settings)| {
            let (_, _, _, data) = fields(&settings).into_iter().rev().find(|f| f.0 == 6 && f.1 == 2)?;
            let sound_fields = fields(data);
            let read_id = |tag| sound_fields.iter().rev().find(|f| f.0 == tag && f.1 == 1).map_or(0, |f| f.2);
            let sound = read_id(1);
            (sound != 0).then(|| (guild, JoinSound { sound, source_guild: read_id(2) }))
        })
        .collect()
}

pub fn with_guild_join_sound(raw: &[u8], guild: u64, sound: Option<JoinSound>) -> Vec<u8> {
    update_guild_settings(raw, guild, |value| {
        let mut out = Vec::new();
        for (tag, wire, number, data) in fields(value) {
            if tag != 6 {
                reencode(tag, wire, number, data, &mut out);
            }
        }
        if let Some(sound) = sound {
            let mut data = Vec::new();
            field_fixed64(1, sound.sound, &mut data);
            if sound.source_guild != 0 {
                field_fixed64(2, sound.source_guild, &mut data);
            }
            field_bytes(6, &data, &mut out);
        }
        out
    })
}

fn update_guild_settings(raw: &[u8], guild: u64, update: impl FnOnce(&[u8]) -> Vec<u8>) -> Vec<u8> {
    let mut entries = guild_entries(raw);
    if let Some((_, value)) = entries.iter_mut().find(|(key, _)| *key == guild) {
        *value = update(value);
    } else {
        entries.push((guild, update(&[])));
    }
    let mut all = Vec::new();
    for (k, value) in entries {
        let mut entry = Vec::new();
        varint((1 << 3) | 1, &mut entry);
        entry.extend_from_slice(&k.to_le_bytes());
        field_bytes(2, &value, &mut entry);
        field_bytes(1, &entry, &mut all);
    }
    let mut kept = Vec::new();
    for (_, _, _, d) in fields(raw).into_iter().filter(|f| f.0 == 3 && f.1 == 2) {
        for (t, w, x, dd) in fields(d) {
            if t != 1 {
                reencode(t, w, x, dd, &mut kept);
            }
        }
    }
    all.extend_from_slice(&kept);
    replace_field(raw, 3, &all)
}

pub fn encode_custom_status(c: &CustomStatusProto) -> Vec<u8> {
    let mut o = Vec::new();
    if !c.text.is_empty() {
        field_bytes(1, c.text.as_bytes(), &mut o);
    }
    if c.emoji_id != 0 {
        field_fixed64(2, c.emoji_id, &mut o);
    }
    if !c.emoji_name.is_empty() {
        field_bytes(3, c.emoji_name.as_bytes(), &mut o);
    }
    if c.expires_at_ms != 0 {
        field_fixed64(4, c.expires_at_ms, &mut o);
    }
    if c.created_at_ms != 0 {
        field_fixed64(5, c.created_at_ms, &mut o);
    }
    o
}

pub fn custom_status_expires_ms(raw: &[u8]) -> Option<u64> {
    let status = fields(raw).into_iter().rev().find(|(t, w, ..)| *t == 11 && *w == 2)?.3;
    let custom = fields(status).into_iter().rev().find(|(t, w, ..)| *t == 2 && *w == 2)?.3;
    Some(fields(custom).into_iter().rev().find(|(t, w, ..)| *t == 4 && *w == 1).map_or(0, |f| f.2))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomUserTheme {
    pub colors: Vec<String>,
    pub angle: i32,
    pub base_mix: i32,
}

fn sub_message(b: &[u8], tag: u32) -> Option<&[u8]> {
    fields(b).into_iter().rev().find(|(t, w, ..)| *t == tag && *w == 2).map(|f| f.3)
}

pub fn custom_user_theme(raw: &[u8]) -> Option<CustomUserTheme> {
    let sub = sub_message;
    let custom = sub(sub(sub(raw, 13)?, 3)?, 4)?;
    let mut out = CustomUserTheme::default();
    for (tag, wt, v, d) in fields(custom) {
        match (tag, wt) {
            (1, 2) => out.colors.push(String::from_utf8_lossy(d).into_owned()),
            (3, 0) => out.angle = v as i32,
            (4, 0) => out.base_mix = v as i32,
            _ => {}
        }
    }
    (!out.colors.is_empty()).then_some(out)
}

pub fn with_custom_status(raw: &[u8], custom: Option<&CustomStatusProto>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut status = Vec::new();
    for (tag, wt, v, d) in fields(raw) {
        if tag == 11 && wt == 2 {
            for (t, w, x, dd) in fields(d) {
                if t != 2 {
                    reencode(t, w, x, dd, &mut status);
                }
            }
            continue;
        }
        reencode(tag, wt, v, d, &mut out);
    }
    if let Some(c) = custom {
        field_bytes(2, &encode_custom_status(c), &mut status);
    }
    field_bytes(11, &status, &mut out);
    out
}

pub fn replace_field(raw: &[u8], tag: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (t, w, v, d) in fields(raw) {
        if t != tag {
            reencode(t, w, v, d, &mut out);
        }
    }
    field_bytes(tag, payload, &mut out);
    out
}

pub fn with_status(raw: &[u8], status: &str, now_ms: u64) -> Vec<u8> {
    let cat: Vec<(u32, u8, u64, &[u8])> = fields(raw).into_iter().filter(|f| f.0 == 11 && f.1 == 2).flat_map(|f| fields(f.3)).collect();
    let prev = cat.iter().rev().find(|f| f.0 == 1 && f.1 == 2).and_then(|f| fields(f.3).into_iter().rev().find(|x| x.0 == 1 && x.1 == 2)).map(|x| x.3);
    let created = cat.iter().rev().find(|f| f.0 == 5 && f.1 == 2).map(|f| f.3);
    let mut st = Vec::new();
    let mut sv = Vec::new();
    field_bytes(1, status.as_bytes(), &mut sv);
    field_bytes(1, &sv, &mut st);
    for (t, w, v, d) in &cat {
        if !matches!(*t, 1 | 4 | 5) {
            reencode(*t, *w, *v, d, &mut st);
        }
    }
    match created {
        Some(c) if prev == Some(status.as_bytes()) => field_bytes(5, c, &mut st),
        _ => {
            let mut ts = Vec::new();
            field_varint(1, now_ms, &mut ts);
            field_bytes(5, &ts, &mut st);
        }
    }
    replace_field(raw, 11, &st)
}

fn by_tag(raw: &[u8]) -> std::collections::BTreeMap<u32, Vec<u8>> {
    let mut m: std::collections::BTreeMap<u32, Vec<u8>> = std::collections::BTreeMap::new();
    for (t, w, v, d) in fields(raw) {
        reencode(t, w, v, d, m.entry(t).or_default());
    }
    m
}

pub fn proto_delta(before: &[u8], after: &[u8]) -> Vec<u8> {
    let (b, a) = (by_tag(before), by_tag(after));
    let mut out = Vec::new();
    for (t, bytes) in &a {
        if b.get(t) != Some(bytes) {
            out.extend_from_slice(bytes);
        }
    }
    for t in b.keys().filter(|t| !a.contains_key(t)) {
        field_bytes(*t, &[], &mut out);
    }
    out
}

type ProtoEdit = Box<dyn FnOnce(Vec<u8>) -> Vec<u8> + Send>;
type ProtoReply = tokio::sync::oneshot::Sender<std::result::Result<Vec<u8>, String>>;

#[derive(Default)]
pub(crate) struct ProtoWriter {
    queue: parking_lot::Mutex<Vec<(ProtoEdit, ProtoReply)>>,
    lock: tokio::sync::Mutex<()>,
}

impl RestClient {
    pub async fn edit_settings_proto(&self, edit: impl FnOnce(Vec<u8>) -> Vec<u8> + Send + 'static) -> Result<Vec<u8>> {
        let w = &self.inner.proto_writer;
        let (tx, rx) = tokio::sync::oneshot::channel();
        w.queue.lock().push((Box::new(edit), tx));
        {
            let _held = w.lock.lock().await;
            let batch = std::mem::take(&mut *w.queue.lock());
            if !batch.is_empty() {
                let (edits, replies): (Vec<ProtoEdit>, Vec<ProtoReply>) = batch.into_iter().unzip();
                let r = self.flush_settings_proto(edits).await.map_err(|e| e.to_string());
                for tx in replies {
                    let _ = tx.send(r.clone());
                }
            }
        }
        rx.await.map_err(|_| Error::Protocol("settings write dropped".into()))?.map_err(Error::Protocol)
    }

    async fn flush_settings_proto(&self, edits: Vec<ProtoEdit>) -> Result<Vec<u8>> {
        let before = self.get_settings_proto_raw().await?;
        let n = edits.len();
        let after = edits.into_iter().fold(before.clone(), |raw, e| e(raw));
        let delta = proto_delta(&before, &after);
        if delta.is_empty() {
            return Ok(before);
        }
        tracing::debug!(edits = n, bytes = delta.len(), "settings-proto write");
        let b: ProtoBody = self
            .call(Method::PATCH, "/users/@me/settings-proto/1", Body::Json(json!({ "settings": STANDARD.encode(delta) })))
            .await?;
        STANDARD.decode(b.settings.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))
    }

    pub async fn save_guild_raid_alerts(&self, guild: u64, alerts: RaidAlerts) -> Result<Vec<u8>> {
        self.edit_settings_proto(move |raw| with_guild_raid_alerts(&raw, guild, alerts)).await
    }

    pub async fn save_guild_folders(&self, folders: &[FolderProto]) -> Result<Vec<u8>> {
        let folders = folders.to_vec();
        self.edit_settings_proto(move |raw| with_guild_folders(&raw, &folders)).await
    }
}

impl SettingChange {
    pub fn encode(&self) -> Vec<u8> {
        let mut inner = Vec::new();
        let mut out = Vec::new();
        match self {
            Self::Theme(t) => {
                field_varint(1, t.to_proto() as u64, &mut inner);
                field_bytes(13, &inner, &mut out);
            }
            Self::DeveloperMode(b) => {
                field_varint(2, *b as u64, &mut inner);
                field_bytes(13, &inner, &mut out);
            }
            Self::Compact(b) => {
                let mut w = Vec::new();
                field_varint(1, *b as u64, &mut w);
                field_bytes(17, &w, &mut inner);
                field_bytes(6, &inner, &mut out);
            }
            Self::Locale(l) => {
                let mut w = Vec::new();
                field_bytes(1, l.as_bytes(), &mut w);
                field_bytes(1, &w, &mut inner);
                field_bytes(12, &inner, &mut out);
            }
            Self::DefaultGuildsRestricted(b) => {
                field_varint(4, *b as u64, &mut inner);
                field_bytes(8, &inner, &mut out);
            }
            Self::UiDensity(v) => {
                field_varint(12, *v as u64, &mut inner);
                field_bytes(13, &inner, &mut out);
            }
            Self::Path { path, wrapper, value } => {
                let Some((&last, outer)) = path.split_last() else { return out };
                let put = |tag: u32, o: &mut Vec<u8>| match value {
                    ProtoValue::Int(v) => field_varint(tag, *v, o),
                    ProtoValue::Fixed32(v) => {
                        varint(((tag as u64) << 3) | 5, o);
                        o.extend_from_slice(&v.to_le_bytes());
                    }
                    ProtoValue::Str(s) => field_bytes(tag, s.as_bytes(), o),
                };
                let mut cur = Vec::new();
                if *wrapper {
                    let mut w = Vec::new();
                    put(1, &mut w);
                    field_bytes(last, &w, &mut cur);
                } else {
                    put(last, &mut cur);
                }
                for &tag in outer.iter().rev() {
                    let mut o = Vec::new();
                    field_bytes(tag, &cur, &mut o);
                    cur = o;
                }
                out = cur;
            }
        }
        out
    }
}

#[derive(Deserialize)]
struct ProtoBody {
    settings: String,
}

impl RestClient {
    pub async fn get_settings_proto(&self) -> Result<PreloadedUserSettings> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/1", Body::None).await?;
        PreloadedUserSettings::decode_base64(&b.settings)
    }

    pub async fn patch_settings_proto(&self, patch: &PreloadedUserSettings) -> Result<PreloadedUserSettings> {
        let b: ProtoBody = self
            .call(
                Method::PATCH,
                "/users/@me/settings-proto/1",
                Body::Json(json!({ "settings": patch.encode_base64() })),
            )
            .await?;
        PreloadedUserSettings::decode_base64(&b.settings)
    }

    pub async fn patch_setting(&self, change: &SettingChange) -> Result<PreloadedUserSettings> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/1", Body::None).await?;
        let mut raw = STANDARD.decode(b.settings.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))?;
        raw.extend(change.encode());
        let b: ProtoBody = self
            .call(Method::PATCH, "/users/@me/settings-proto/1", Body::Json(json!({ "settings": STANDARD.encode(raw) })))
            .await?;
        PreloadedUserSettings::decode_base64(&b.settings)
    }

    pub async fn save_custom_status(&self, custom: Option<&CustomStatusProto>) -> Result<Vec<u8>> {
        let custom = custom.cloned();
        self.edit_settings_proto(move |raw| with_custom_status(&raw, custom.as_ref())).await
    }

    pub async fn get_settings_proto_raw(&self) -> Result<Vec<u8>> {
        let b: ProtoBody = self.call(Method::GET, "/users/@me/settings-proto/1", Body::None).await?;
        STANDARD.decode(b.settings.trim()).map_err(|e| Error::Protocol(format!("settings base64: {e}")))
    }

    pub async fn patch_setting_raw(&self, change: &SettingChange) -> Result<Vec<u8>> {
        let change = change.encode();
        self.edit_settings_proto(move |mut raw| {
            raw.extend(change);
            raw
        })
        .await
    }

    pub async fn get_consent(&self) -> Result<Value> {
        self.call(Method::GET, "/users/@me/consent", Body::None).await
    }

    pub async fn set_consent(&self, kind: &str, granted: bool) -> Result<()> {
        let (grant, revoke): (Vec<&str>, Vec<&str>) = if granted { (vec![kind], vec![]) } else { (vec![], vec![kind]) };
        self.call_empty(Method::POST, "/users/@me/consent", Body::Json(json!({ "grant": grant, "revoke": revoke }))).await
    }

    pub async fn get_email_settings(&self) -> Result<Value> {
        self.call(Method::GET, "/users/@me/email-settings", Body::None).await
    }

    pub async fn patch_email_setting(&self, category: &str, on: bool) -> Result<Value> {
        self.call(
            Method::PATCH,
            "/users/@me/email-settings",
            Body::Json(json!({ "settings": { "categories": { category: on } } })),
        )
        .await
    }

    pub async fn get_settings_json(&self) -> Result<Value> {
        self.call(Method::GET, "/users/@me/settings", Body::None).await
    }

    pub async fn patch_settings_json(&self, patch: Value) -> Result<Value> {
        self.call(Method::PATCH, "/users/@me/settings", Body::Json(patch)).await
    }
}

pub fn from_json(v: &Value) -> PreloadedUserSettings {
    let mut s = PreloadedUserSettings::patch_appearance(
        Theme::from_json(v.get("theme").and_then(Value::as_str).unwrap_or("dark")),
        v.get("developer_mode").and_then(Value::as_bool).unwrap_or(false),
    );
    s.text_and_images = Some(TextAndImagesSettings {
        message_display_compact: v.get("message_display_compact").and_then(Value::as_bool).map(|value| BoolValue { value }),
        ..Default::default()
    });
    if let Some(l) = v.get("locale").and_then(Value::as_str) {
        s.localization = Some(LocalizationSettings { locale: Some(StringValue { value: l.to_owned() }) });
    }
    let ids = |k: &str| -> Vec<u64> {
        v.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().and_then(|s| s.parse().ok()).or_else(|| x.as_u64())).collect())
            .unwrap_or_default()
    };
    s.privacy = Some(PrivacySettings {
        restricted_guild_ids: ids("restricted_guilds"),
        default_guilds_restricted: v.get("default_guilds_restricted").and_then(Value::as_bool).unwrap_or(false),
        activity_restricted_guild_ids: ids("activity_restricted_guild_ids"),
    });
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entrance_sounds_round_trip_with_global_and_guild_settings() {
        let global = JoinSound { sound: 8, source_guild: 0 };
        let local = JoinSound { sound: 88, source_guild: 42 };
        let alerts = RaidAlerts { push_off: true, nag_off: true };
        let raw = with_guild_raid_alerts(&[], 42, alerts);
        let raw = with_guild_join_sound(&raw, 0, Some(global));
        let raw = with_guild_join_sound(&raw, 42, Some(local));
        let sounds = guild_join_sounds(&raw);
        assert_eq!(sounds.get(&0), Some(&global));
        assert_eq!(sounds.get(&42), Some(&local));
        let raw = with_guild_join_sound(&raw, 42, None);
        assert_eq!(guild_join_sounds(&raw).into_iter().collect::<Vec<_>>(), vec![(0, global)]);
        assert_eq!(guild_raid_alerts(&raw).get(&42), Some(&alerts));
    }

    #[test]
    fn raid_alert_flags_round_trip_inside_the_guild_map() {
        let mut other = Vec::new();
        field_varint(2, 7, &mut other);
        let mut entry = Vec::new();
        varint(9, &mut entry);
        entry.extend_from_slice(&42u64.to_le_bytes());
        field_bytes(2, &other, &mut entry);
        let mut all = Vec::new();
        field_bytes(1, &entry, &mut all);
        let mut raw = Vec::new();
        field_bytes(3, &all, &mut raw);
        field_varint(20, 5, &mut raw);
        assert_eq!(guild_raid_alerts(&raw).get(&42), Some(&RaidAlerts::default()));
        let next = with_guild_raid_alerts(&raw, 42, RaidAlerts { push_off: true, nag_off: false });
        assert_eq!(guild_raid_alerts(&next).get(&42), Some(&RaidAlerts { push_off: true, nag_off: false }));
        let entry = guild_entries(&next).into_iter().find(|e| e.0 == 42).unwrap().1;
        assert!(fields(&entry).iter().any(|f| f.0 == 2 && f.2 == 7));
        assert_eq!(proto_read(&next, &[20], false, false), Some(ProtoValue::Int(5)));
        let added = with_guild_raid_alerts(&next, 7, RaidAlerts { push_off: false, nag_off: true });
        let map = guild_raid_alerts(&added);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&7), Some(&RaidAlerts { push_off: false, nag_off: true }));
        assert_eq!(map.get(&42), Some(&RaidAlerts { push_off: true, nag_off: false }));
    }

    #[test]
    fn reads_the_custom_user_theme() {
        let mut custom = Vec::new();
        field_bytes(1, b"#5865f2", &mut custom);
        field_bytes(1, b"#121238", &mut custom);
        field_varint(3, 45, &mut custom);
        field_varint(4, 74, &mut custom);
        let mut client = Vec::new();
        field_bytes(4, &custom, &mut client);
        let mut appearance = Vec::new();
        field_bytes(3, &client, &mut appearance);
        let mut raw = Vec::new();
        field_bytes(13, &appearance, &mut raw);
        let t = custom_user_theme(&raw).unwrap();
        assert_eq!(t.colors, vec!["#5865f2".to_owned(), "#121238".to_owned()]);
        assert_eq!((t.angle, t.base_mix), (45, 74));
        assert!(custom_user_theme(&appearance).is_none());
    }

    #[test]
    fn reads_the_custom_status_expiry() {
        let c = CustomStatusProto { text: "x".into(), expires_at_ms: 1_700_000_000_000, ..Default::default() };
        let raw = with_custom_status(&[], Some(&c));
        assert_eq!(custom_status_expires_ms(&raw), Some(1_700_000_000_000));
        let never = with_custom_status(&[], Some(&CustomStatusProto { text: "x".into(), ..Default::default() }));
        assert_eq!(custom_status_expires_ms(&never), Some(0));
        assert_eq!(custom_status_expires_ms(&with_custom_status(&[], None)), None);
        assert_eq!(custom_status_expires_ms(&[]), None);
    }

    #[test]
    fn custom_status_save_replaces_only_the_custom_status() {
        let mut raw = Vec::new();
        let mut status = Vec::new();
        let mut online = Vec::new();
        field_bytes(1, b"dnd", &mut online);
        field_bytes(1, &online, &mut status);
        let old = CustomStatusProto { text: "old".into(), expires_at_ms: 5, ..Default::default() };
        field_bytes(2, &encode_custom_status(&old), &mut status);
        field_fixed64(4, 9, &mut status);
        field_bytes(11, &status, &mut raw);
        field_varint(13, 1, &mut raw);
        let new = CustomStatusProto {
            text: "Lunch".into(),
            emoji_id: 0,
            emoji_name: "\u{1f354}".into(),
            expires_at_ms: 1_764_100_000_000,
            created_at_ms: 1_764_085_600_000,
        };
        let next = with_custom_status(&raw, Some(&new));
        assert_eq!(proto_read(&next, &[11, 1], true, true), Some(ProtoValue::Str("dnd".into())));
        assert_eq!(proto_read(&next, &[11, 2, 1], false, true), Some(ProtoValue::Str("Lunch".into())));
        assert_eq!(proto_read(&next, &[11, 2, 3], false, true), Some(ProtoValue::Str("\u{1f354}".into())));
        assert_eq!(proto_read(&next, &[11, 2, 4], false, false), Some(ProtoValue::Int(1_764_100_000_000)));
        assert_eq!(proto_read(&next, &[11, 2, 5], false, false), Some(ProtoValue::Int(1_764_085_600_000)));
        assert_eq!(proto_read(&next, &[11, 2, 2], false, false), None);
        assert_eq!(proto_read(&next, &[11, 4], false, false), Some(ProtoValue::Int(9)));
        let status_fields = fields(fields(&next).into_iter().find(|f| f.0 == 11).unwrap().3);
        assert_eq!(status_fields.iter().filter(|f| f.0 == 2).count(), 1);
        assert!(fields(&next).iter().any(|f| f.0 == 13 && f.2 == 1));
        let cleared = with_custom_status(&raw, None);
        assert_eq!(proto_read(&cleared, &[11, 2, 1], false, true), None);
        assert_eq!(proto_read(&cleared, &[11, 1], true, true), Some(ProtoValue::Str("dnd".into())));
        let wire = encode_custom_status(&CustomStatusProto { text: "a".into(), expires_at_ms: 1, ..Default::default() });
        assert_eq!(wire, [0x0a, 1, b'a', 0x21, 1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn folder_save_replaces_only_the_folder_list() {
        let mut raw = Vec::new();
        let mut appearance = Vec::new();
        field_varint(1, 2, &mut appearance);
        field_bytes(13, &appearance, &mut raw);
        let old = FolderProto { guilds: vec![1, 2], id: Some(7), name: Some("Old".into()), color: None };
        let mut gf = Vec::new();
        field_bytes(1, &encode_folder(&old), &mut gf);
        let positions: Vec<u8> = [2u64, 1].iter().flat_map(|g| g.to_le_bytes()).collect();
        field_bytes(2, &positions, &mut gf);
        field_bytes(14, &gf, &mut raw);
        let new = FolderProto { guilds: vec![1, 2], id: Some(7), name: Some("Jeux".into()), color: Some(0x5865F2) };
        let next = with_guild_folders(&raw, &[new.clone(), FolderProto { guilds: vec![3], ..Default::default() }]);
        let top = fields(&next);
        assert_eq!(top.iter().filter(|f| f.0 == 14).count(), 1);
        assert!(top.iter().any(|f| f.0 == 13 && f.3 == appearance.as_slice()));
        let folders = top.iter().find(|f| f.0 == 14).map(|f| fields(f.3)).unwrap();
        let list: Vec<&[u8]> = folders.iter().filter(|f| f.0 == 1).map(|f| f.3).collect();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], encode_folder(&new).as_slice());
        assert!(folders.iter().any(|f| f.0 == 2 && f.3 == positions.as_slice()));
        let one = fields(list[0]);
        assert_eq!(one[0].3, [1u64.to_le_bytes(), 2u64.to_le_bytes()].concat().as_slice());
        assert_eq!(proto_read(list[0], &[3], true, true), Some(ProtoValue::Str("Jeux".into())));
        assert_eq!(proto_read(list[0], &[4], true, false), Some(ProtoValue::Int(0x5865F2)));
        assert!(proto_read(list[1], &[3], true, true).is_none());
    }

    #[test]
    fn huge_length_prefix_stops_parsing() {
        let mut b = vec![0x0a];
        b.extend_from_slice(&[0xff; 9]);
        b.push(0x01);
        assert!(fields(&b).is_empty());
        assert_eq!(fields(&[0x08, 0x01, 0x12, 0x80, 0x80, 0x80, 0x80, 0x10]).len(), 1);
    }

    #[test]
    fn roundtrip_and_unknown_fields() {
        let mut s = PreloadedUserSettings::patch_appearance(Theme::Onyx, true);
        s.text_and_images = PreloadedUserSettings::patch_compact(true).text_and_images;
        let enc = s.encode_base64();
        let d = PreloadedUserSettings::decode_base64(&enc).unwrap();
        assert_eq!(d.theme(), Theme::Onyx);
        assert!(d.developer_mode());
        assert!(d.compact());

        let mut bytes = STANDARD.decode(&enc).unwrap();
        bytes.extend_from_slice(&[0x72, 0x02, 0x08, 0x01]);
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(bytes)).unwrap();
        assert_eq!(d.theme(), Theme::Onyx);
    }

    #[test]
    fn theme_wire_values() {
        let bytes = [0x6a, 0x02, 0x08, 0x02];
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(bytes)).unwrap();
        assert_eq!(d.theme(), Theme::Light);
        assert!(!d.compact());
    }

    #[test]
    fn appended_changes_override_including_false() {
        let base = PreloadedUserSettings::patch_appearance(Theme::Light, true);
        let mut raw = prost::Message::encode_to_vec(&base);
        raw.extend(SettingChange::DeveloperMode(false).encode());
        raw.extend(SettingChange::Compact(true).encode());
        raw.extend(SettingChange::Locale("fr".into()).encode());
        raw.extend(SettingChange::DefaultGuildsRestricted(true).encode());
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(&raw)).unwrap();
        assert_eq!(d.theme(), Theme::Light, "untouched field kept");
        assert!(!d.developer_mode());
        assert!(d.compact());
        assert_eq!(d.locale(), Some("fr"));
        assert!(d.default_guilds_restricted());
        raw.extend(SettingChange::Theme(Theme::Ash).encode());
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(&raw)).unwrap();
        assert_eq!(d.theme(), Theme::Ash);
    }

    #[test]
    fn path_changes_roundtrip() {
        let base = PreloadedUserSettings::patch_appearance(Theme::Light, true);
        let mut raw = prost::Message::encode_to_vec(&base);
        let ch = |path: &[u32], wrapper, value| SettingChange::Path { path: path.to_vec(), wrapper, value };
        raw.extend(ch(&[6, 12], true, ProtoValue::Int(0)).encode());
        raw.extend(ch(&[6, 4], true, ProtoValue::Str("ALWAYS".into())).encode());
        raw.extend(ch(&[13, 9], false, ProtoValue::Int(2)).encode());
        assert_eq!(proto_read(&raw, &[6, 12], true, false), Some(ProtoValue::Int(0)));
        assert_eq!(proto_read(&raw, &[6, 4], true, true), Some(ProtoValue::Str("ALWAYS".into())));
        assert_eq!(proto_read(&raw, &[13, 9], false, false), Some(ProtoValue::Int(2)));
        assert_eq!(proto_read(&raw, &[6, 13], true, false), None);
        raw.extend(ch(&[6, 12], true, ProtoValue::Int(1)).encode());
        assert_eq!(proto_read(&raw, &[6, 12], true, false), Some(ProtoValue::Int(1)));
        let d = PreloadedUserSettings::decode_base64(&STANDARD.encode(&raw)).unwrap();
        assert_eq!(d.theme(), Theme::Light, "untouched field kept");
        assert!(d.developer_mode());
    }

    #[test]
    fn json_fallback() {
        let s = from_json(&json!({"theme": "darker", "message_display_compact": true, "locale": "fr"}));
        assert_eq!(s.theme(), Theme::Ash);
        assert!(s.compact());
        assert_eq!(s.locale(), Some("fr"));
    }

    #[test]
    fn restricted_guilds_from_json_and_proto() {
        let s = from_json(&json!({"restricted_guilds": ["12", "34"], "default_guilds_restricted": true}));
        let p = s.privacy.as_ref().unwrap();
        assert_eq!(p.restricted_guild_ids, vec![12, 34]);
        assert!(s.default_guilds_restricted());
        let mut raw = vec![0x42, 0x12, 0x1a, 0x10];
        raw.extend(1369382919948210248u64.to_le_bytes());
        raw.extend(7u64.to_le_bytes());
        let d = PreloadedUserSettings::decode_raw(&raw).unwrap();
        assert_eq!(d.privacy.unwrap().restricted_guild_ids, vec![1369382919948210248, 7]);
    }

    fn status_cat(status: &str, custom: &CustomStatusProto, created: Option<u64>) -> Vec<u8> {
        let mut st = Vec::new();
        let mut sv = Vec::new();
        field_bytes(1, status.as_bytes(), &mut sv);
        field_bytes(1, &sv, &mut st);
        field_bytes(2, &encode_custom_status(custom), &mut st);
        field_varint(3, 1, &mut st);
        if let Some(c) = created {
            let mut ts = Vec::new();
            field_varint(1, c, &mut ts);
            field_bytes(5, &ts, &mut st);
        }
        st
    }

    #[test]
    fn status_change_only_writes_the_status_category_and_keeps_the_custom_status() {
        let custom = CustomStatusProto { text: "Lunch".into(), emoji_id: 42, emoji_name: "dance".into(), expires_at_ms: 9_000, created_at_ms: 1_000 };
        let mut raw = Vec::new();
        field_bytes(5, &[0x32, 0x02, 0x08, 0x3c], &mut raw);
        field_bytes(11, &status_cat("online", &custom, Some(500)), &mut raw);
        field_bytes(14, &[0x0a, 0x00], &mut raw);
        let next = with_status(&raw, "dnd", 7_000);
        let delta = proto_delta(&raw, &next);
        let mut want = Vec::new();
        field_bytes(11, &status_cat("dnd", &custom, Some(7_000)), &mut want);
        assert_eq!(delta, want);
        let d = PreloadedUserSettings::decode_raw(&next).unwrap();
        assert_eq!(d.status(), Some("dnd"));
        let st = fields(&delta)[0].3;
        let cs = fields(st).into_iter().find(|f| f.0 == 2).unwrap().3;
        assert_eq!(cs, encode_custom_status(&custom).as_slice());
    }

    #[test]
    fn same_status_keeps_its_created_at() {
        let custom = CustomStatusProto::default();
        let mut raw = Vec::new();
        field_bytes(11, &status_cat("idle", &custom, Some(500)), &mut raw);
        assert!(proto_delta(&raw, &with_status(&raw, "idle", 7_000)).is_empty());
    }

    #[test]
    fn delta_sends_changed_categories_only() {
        let mut a = Vec::new();
        field_bytes(5, &[0x08, 0x01], &mut a);
        field_bytes(14, &[0x0a, 0x00], &mut a);
        let mut b = Vec::new();
        field_bytes(5, &[0x08, 0x01], &mut b);
        field_bytes(14, &[0x0a, 0x02, 0x08, 0x01], &mut b);
        assert_eq!(proto_delta(&a, &b), vec![0x72, 0x04, 0x0a, 0x02, 0x08, 0x01]);
        assert_eq!(proto_delta(&a, &a), Vec::<u8>::new());
        assert_eq!(proto_delta(&a, &replace_field(&a, 14, &[0x0a, 0x00])), Vec::<u8>::new());
    }
}
