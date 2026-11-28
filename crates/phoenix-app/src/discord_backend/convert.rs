use std::collections::{HashMap, HashSet};

use base64::Engine;
use gpui::SharedString;
use phoenix_api::{
    Cache, GatewayEvent,
    gateway::{GuildMemberListUpdate, MemberListItem, MemberListOp, Ready},
    models as api,
};
use serde_json::Value;

use crate::{
    backend::{AppEvent, ReadyData},
    model::{self, Id},
};

pub const AVATAR_SIZE: u32 = 128;
pub const ICON_SIZE: u32 = 96;

pub fn id(s: api::Snowflake) -> Id {
    Id(s.0)
}

pub fn sf(i: Id) -> api::Snowflake {
    api::Snowflake(i.0)
}

pub fn user(u: &api::User) -> model::User {
    let avatar = if u.avatar.is_some() {
        model::ImageRef::Url(u.avatar_url(AVATAR_SIZE).into())
    } else {
        let idx = if u.discriminator.is_empty() || u.discriminator == "0" {
            (u.id.0 >> 22) % 6
        } else {
            u.discriminator.parse::<u64>().unwrap_or(0) % 5
        };
        model::ImageRef::Default(idx as u8)
    };
    model::User {
        id: id(u.id),
        name: u.username.clone().into(),
        display_name: collapse_ws(u.display_name()).into(),
        avatar,
        bot: u.bot || u.system,
        verified_bot: u.system || u.public_flags.is_some_and(|f| f & (1 << 16) != 0),
        tag: u.primary_guild.as_ref().and_then(|p| Some((p.shown_tag()?.to_owned().into(), p.badge_url().map(Into::into)))),
        tag_guild: u.primary_guild.as_ref().and_then(|p| p.shown_tag().and(p.identity_guild_id).map(id)),
        nameplate: u.nameplate().map(|(url, pal)| (url.into(), pal.as_deref().and_then(crate::ui::nameplate::palette_dark))),
        name_style: u.display_name_styles.as_ref().map(|s| (s.font_id, s.effect_id, s.colors.clone())),
        decoration: decoration_url(u),
        flags: u.flags.unwrap_or(0),
    }
}

pub fn decoration_url(u: &api::User) -> Option<SharedString> {
    crate::ui::avatar_deco::url(u.avatar_decoration_data.as_ref()?)
}

pub fn member_avatar_url(g: api::GuildId, u: api::UserId, m: &api::Member) -> Option<SharedString> {
    let h = m.avatar.as_deref().filter(|h| !h.is_empty())?;
    let ext = if h.starts_with("a_") { "gif" } else { "png" };
    Some(format!("https://cdn.discordapp.com/guilds/{}/users/{}/avatars/{h}.{ext}?size={AVATAR_SIZE}", g.0, u.0).into())
}

pub fn member_decoration_url(m: &api::Member) -> Option<SharedString> {
    crate::ui::avatar_deco::url(m.avatar_decoration_data.as_ref()?)
}

pub fn member_nameplate(m: &api::Member) -> Option<(SharedString, Option<u32>)> {
    m.nameplate().map(|(url, pal)| (url.into(), pal.as_deref().and_then(crate::ui::nameplate::palette_dark)))
}

pub fn presence(status: &str) -> model::Presence {
    match status {
        "online" => model::Presence::Online,
        "idle" => model::Presence::Idle,
        "dnd" => model::Presence::Dnd,
        _ => model::Presence::Offline,
    }
}

pub fn activity(activities: &[Value]) -> Option<SharedString> {
    let a = activities.iter().find(|a| a.get("type").and_then(Value::as_u64) != Some(6))?;
    let kind = a.get("type").and_then(Value::as_u64).unwrap_or(0);
    let name = a.get("name").and_then(Value::as_str).unwrap_or_default();
    let text = match kind {
        4 => a.get("state").and_then(Value::as_str).unwrap_or_default().to_owned(),
        1 => format!("Streaming {name}"),
        2 => format!("Listening to {name}"),
        3 => format!("Watching {name}"),
        5 => format!("Competing in {name}"),
        _ => format!("Playing {name}"),
    };
    (!text.trim().is_empty()).then(|| text.into())
}

fn embed_media(m: &Option<api::EmbedMedia>, atts: &[api::Attachment]) -> Option<model::EmbedMedia> {
    let m = m.as_ref()?;
    let url = m.proxy_url.clone().filter(|u| u.starts_with("https://")).or_else(|| m.url.clone())?;
    let (mut w, mut h) = (m.width.unwrap_or(0), m.height.unwrap_or(0));
    if w == 0 || h == 0 {
        let path = |u: &str| u.split('?').next().unwrap_or("").rsplit('/').take(2).collect::<Vec<_>>().join("/");
        let want = path(&url);
        if let Some(a) = atts.iter().find(|a| path(&a.url) == want || path(&a.proxy_url) == want) {
            (w, h) = (a.width.unwrap_or(0), a.height.unwrap_or(0));
        }
    }
    Some(model::EmbedMedia { url: url.into(), width: w, height: h })
}

fn forward_snapshot(m: &api::Message) -> Option<(api::Message, model::Forward)> {
    let snap = m.message_snapshots.as_ref()?.get(0)?.get("message")?;
    let mut v = serde_json::to_value(m).ok()?;
    let o = v.as_object_mut()?;
    for k in ["content", "attachments", "embeds", "edited_timestamp", "sticker_items"] {
        o.insert(k.into(), snap.get(k).cloned().unwrap_or(serde_json::Value::Null));
    }
    for k in ["message_snapshots", "message_reference", "referenced_message"] {
        o.remove(k);
    }
    if o.get("content").is_some_and(|c| c.is_null()) {
        o.insert("content".into(), "".into());
    }
    for k in ["attachments", "embeds"] {
        if o.get(k).is_some_and(|c| c.is_null()) {
            o.insert(k.into(), serde_json::json!([]));
        }
    }
    let inner: api::Message = serde_json::from_value(v).ok()?;
    let r = m.message_reference.as_ref();
    let ts = snap
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.timestamp())
        .or_else(|| r.and_then(|r| r.message_id).map(|i| (i.timestamp_ms() / 1000) as i64))
        .unwrap_or(0);
    let f = model::Forward {
        guild: r.and_then(|r| r.guild_id).map(id),
        channel: r.and_then(|r| r.channel_id).map(id).unwrap_or(id(m.channel_id)),
        timestamp: ts,
    };
    Some((inner, f))
}

pub fn message(m: &api::Message) -> model::Message {
    if m.kind == api::MessageType::ThreadStarterMessage
        && let Some(r) = &m.referenced_message
    {
        let mut out = message(r);
        out.channel = id(m.channel_id);
        out.thread = None;
        return out;
    }
    if let Some((inner, f)) = forward_snapshot(m) {
        let mut out = message(&inner);
        out.forward = Some(f);
        return out;
    }
    crate::commands::store::record(m);
    let reply_to = m
        .message_reference
        .as_ref()
        .filter(|_| m.is_reply())
        .and_then(|r| r.message_id)
        .map(id);
    model::Message {
        id: id(m.id),
        channel: id(m.channel_id),
        author: id(m.author.id),
        timestamp: (m.id.timestamp_ms() / 1000) as i64,
        edited: m.edited_timestamp.is_some(),
        edited_at: m.edited_timestamp.as_deref().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map_or(0, |d| d.timestamp()),
        content: m.content.clone().into(),
        reply_to,
        reply_mentions_author: m
            .referenced_message
            .as_ref()
            .is_some_and(|r| m.mentions.iter().any(|u| u.user.id == r.author.id)),
        reply_preview: m.referenced_message.as_ref().filter(|_| reply_to.is_some()).map(|r| {
            let nick = r.member.as_ref().and_then(|mb| mb.nick.as_deref()).filter(|n| !n.is_empty()).map(|n| collapse_ws(n).into());
            (id(r.author.id), nick, r.content.clone().into())
        }),
        reply_voice: m.referenced_message.as_ref().is_some_and(|r| r.flags & phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE != 0),
        forward: None,
        author_nick: m.member.as_ref().and_then(|mb| mb.nick.as_deref()).filter(|n| !n.is_empty()).map(|n| collapse_ws(n).into()),
        embeds: model::merge_embeds_on_url(m
            .embeds
            .iter()
            .filter(|e| e.title.is_some() || e.description.is_some() || e.image.is_some() || e.thumbnail.is_some() || e.author.is_some() || !e.fields.is_empty())
            .map(|e| model::Embed {
                color: e.color,
                provider: e.provider.as_ref().and_then(|p| p.name.clone()).map(Into::into),
                title: e.title.clone().map(Into::into),
                description: e.description.clone().map(Into::into),
                url: e.url.clone().map(Into::into),
                kind: e.kind.clone().map(Into::into),
                thumbnail: embed_media(&e.thumbnail, &m.attachments),
                image: embed_media(&e.image, &m.attachments),
                images: Vec::new(),
                video: embed_media(&e.video, &m.attachments),
                author: e.author.as_ref().map(|a| model::EmbedAuthor {
                    name: a.name.clone().into(),
                    url: a.url.clone().map(Into::into),
                    icon: a.proxy_icon_url.clone().or_else(|| a.icon_url.clone()).map(Into::into),
                }),
                fields: e
                    .fields
                    .iter()
                    .map(|f| model::EmbedField { name: f.name.clone().into(), value: f.value.clone().into(), inline: f.inline })
                    .collect(),
                footer: e.footer.as_ref().map(|f| model::EmbedFooter {
                    text: f.text.clone().into(),
                    icon: f.proxy_icon_url.clone().or_else(|| f.icon_url.clone()).map(Into::into),
                }),
                timestamp: e
                    .timestamp
                    .as_deref()
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                    .map(|d| d.timestamp()),
            })
            .collect()),
        attachments: m
            .attachments
            .iter()
            .map(|a| {
                let ct = a.content_type.as_deref().unwrap_or("");
                let sized = a.width.is_some() && a.height.is_some();
                let kind = if ct.starts_with("image/") && sized {
                    model::AttachmentKind::Image
                } else if ct.starts_with("video/") && sized {
                    model::AttachmentKind::Video
                } else if ct.starts_with("audio/") {
                    model::AttachmentKind::Audio
                } else {
                    model::AttachmentKind::File
                };
                let proxy = if a.proxy_url.starts_with("https://") { &a.proxy_url } else { &a.url };
                model::Attachment {
                    filename: a.filename.clone().into(),
                    url: if matches!(kind, model::AttachmentKind::File | model::AttachmentKind::Audio) { a.url.clone() } else { proxy.clone() }.into(),
                    width: a.width.unwrap_or(0),
                    height: a.height.unwrap_or(0),
                    is_image: kind == model::AttachmentKind::Image,
                    size: a.size,
                    kind,
                    original: a.url.clone().into(),
                    spoiler: a.flags & 8 != 0 || a.filename.starts_with("SPOILER_"),
                    description: a.description.clone().filter(|d| !d.is_empty()).map(Into::into),
                    voice: (kind == model::AttachmentKind::Audio && (m.flags & phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE != 0 || a.waveform.is_some()))
                        .then(|| model::VoiceMeta {
                            duration_secs: a.duration_secs.unwrap_or(0.),
                            waveform: a.waveform.clone().unwrap_or_default().into(),
                        }),
                }
            })
            .chain(m.sticker_items.iter().map(|st| {
                use crate::media::StickerFormat as F;
                let format = match st.format_type {
                    api::StickerFormat::Apng => F::Apng,
                    api::StickerFormat::Lottie => F::Lottie,
                    api::StickerFormat::Gif => F::Gif,
                    _ => F::Png,
                };
                model::Attachment {
                    filename: st.name.clone().into(),
                    url: crate::media::sticker_play_url(st.id.0, format).unwrap_or_else(|| crate::media::lottie::url(st.id.0)),
                    width: 160,
                    height: 160,
                    kind: model::AttachmentKind::Sticker(format),
                    ..Default::default()
                }
            }))
            .collect(),
        soundboard_sounds: m.soundboard_sounds.iter().map(|s| model::SoundboardSound {
            id: id(s.sound_id),
            guild: s.guild_id.map(id),
            name: s.name.clone().filter(|n| !n.is_empty()).map(Into::into),
            emoji_id: s.emoji_id.map(id),
            emoji_name: s.emoji_name.clone().filter(|n| !n.is_empty()).map(Into::into),
            volume: 1.0,
            available: true,
            user: None,
        }).collect(),
        reactions: m
            .reactions
            .iter()
            .map(|r| model::Reaction {
                emoji: match (&r.emoji.id, &r.emoji.name) {
                    (Some(eid), Some(n)) => {
                        format!("<{}:{n}:{}>", if r.emoji.animated { "a" } else { "" }, eid.0).into()
                    }
                    (_, Some(n)) => n.clone().into(),
                    _ => "?".into(),
                },
                count: if r.count_details.normal + r.count_details.burst > 0 { r.count_details.normal } else { r.count },
                me: r.me,
                emoji_id: r.emoji.id.map(id),
                burst_count: r.count_details.burst,
                me_burst: r.me_burst,
                burst_colors: r.burst_colors.iter().map(|c| c.clone().into()).collect(),
            })
            .collect(),
        kind: match message_kind(m.kind) {
            model::MessageKind::RecipientAdd(_) => model::MessageKind::RecipientAdd(m.mentions.first().map(|x| id(x.user.id))),
            model::MessageKind::RecipientRemove(_) => model::MessageKind::RecipientRemove(m.mentions.first().map(|x| id(x.user.id))),
            model::MessageKind::PollResult(_) => model::MessageKind::PollResult(m.message_reference.as_ref().and_then(|r| r.message_id).map(id)),
            k => k,
        },
        pinned: m.pinned,
        mention_everyone: m.mention_everyone,
        thread: m.thread.as_ref().map(|t| model::ThreadRef {
            id: id(t.id),
            name: t.name.clone().unwrap_or_default().into(),
            message_count: t.message_count.unwrap_or(0),
        }),
        nonce: m.nonce.as_ref().and_then(|n| match n {
            api::Nonce::Int(i) => u64::try_from(*i).ok(),
            api::Nonce::Str(s) => s.parse().ok(),
        }),
        send_state: model::SendState::Sent,
        flags: m.flags,
        type_id: m.kind.value(),
        webhook_name: m.webhook_id.map(|_| m.author.username.clone().into()),
        application_id: m.application_id.map(id),
        has_mentions: !m.mentions.is_empty() || !m.mention_roles.is_empty() || m.mention_everyone,
        tts: m.tts,
        interaction: m.command_use().map(|(iid, u, name, command_type, target)| model::CommandUse {
            id: id(iid),
            user: id(u.id),
            name: name.into(),
            command_type,
            target: target.map(|t| id(t.id)),
        }),
        call: m.call.as_ref().map(|c| {
            let ms = |t: &str| chrono::DateTime::parse_from_rfc3339(t).ok().map(|d| d.timestamp_millis());
            model::CallInfo {
                participants: c.participants.iter().map(|p| id(*p)).collect(),
                duration_ms: c.ended_timestamp.as_deref().and_then(ms).map(|end| end - ms(m.timestamp.as_str()).unwrap_or(m.id.timestamp_ms() as i64)),
            }
        }),
        sys: sys_data(m),
    }
}

fn sys_data(m: &api::Message) -> Option<Box<model::SysData>> {
    let kind = m.kind.value();
    if matches!(kind, 0 | 19 | 20 | 23) {
        return None;
    }
    let text = |v: Option<&serde_json::Value>| v.and_then(|v| v.as_str()).map(|s| SharedString::from(s.to_owned()));
    let role = m.role_subscription_data.as_ref();
    let data = model::SysData {
        app_name: text(m.application.as_ref().and_then(|a| a.get("name"))),
        tier_name: text(role.and_then(|r| r.get("tier_name"))),
        months: role.and_then(|r| r.get("total_months_subscribed")).and_then(|v| v.as_u64()).map_or(0, |v| v as u32),
        renewal: role.and_then(|r| r.get("is_renewal")).and_then(|v| v.as_bool()).unwrap_or(false),
        product: text(m.purchase_notification.as_ref().and_then(|p| p.get("guild_product_purchase")).and_then(|p| p.get("product_name"))),
        ref_channel: m.message_reference.as_ref().and_then(|r| r.channel_id).map(id),
        ref_guild: m.message_reference.as_ref().and_then(|r| r.guild_id).map(id),
        mention: m.mentions.first().map(|x| id(x.user.id)),
    };
    Some(Box::new(data))
}

fn message_kind(k: api::MessageType) -> model::MessageKind {
    use api::MessageType as T;
    use model::MessageKind as K;
    match k {
        T::Default | T::ChatInputCommand | T::ContextMenuCommand | T::ThreadStarterMessage => K::Default,
        T::Unknown(26 | 41 | 45 | 47 | 68) => K::Default,
        T::Reply => K::Reply,
        T::UserJoin => K::Join,
        T::ChannelPinnedMessage => K::Pin,
        T::GuildBoost => K::Boost(0),
        T::GuildBoostTier1 => K::Boost(1),
        T::GuildBoostTier2 => K::Boost(2),
        T::GuildBoostTier3 => K::Boost(3),
        T::ThreadCreated => K::ThreadCreated,
        T::PollResult => K::PollResult(None),
        T::RecipientAdd => K::RecipientAdd(None),
        T::RecipientRemove => K::RecipientRemove(None),
        T::Call => K::Call,
        T::ChannelNameChange => K::ChannelNameChange,
        T::StageStart => K::StageStart,
        T::StageEnd => K::StageEnd,
        T::StageSpeaker => K::StageSpeaker,
        T::StageTopic => K::StageTopic,
        _ => K::OtherSystem,
    }
}

pub fn message_authors(m: &api::Message, out: &mut Vec<api::Snowflake>) {
    if m.webhook_id.is_none() && !out.contains(&m.author.id) {
        out.push(m.author.id);
    }
    if let Some(r) = &m.referenced_message
        && r.webhook_id.is_none()
        && !out.contains(&r.author.id)
    {
        out.push(r.author.id);
    }
}

pub fn message_users(m: &api::Message, out: &mut Vec<model::User>) {
    if !m.author.username.is_empty() {
        out.push(user(&m.author));
    }
    out.extend(m.mentions.iter().map(|x| user(&x.user)));
    if let Some((_, u, _, _, t)) = m.command_use() {
        out.extend([u].into_iter().chain(t).filter(|u| !u.username.is_empty()).map(user));
    }
    if let Some(r) = &m.referenced_message {
        message_users(r, out);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReadMark {
    pub last_read: u64,
    pub mentions: u32,
}

fn snowflake_value(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

pub fn parse_read_state(v: Option<&Value>) -> HashMap<u64, ReadMark> {
    let mut out = HashMap::new();
    let entries = match v {
        Some(Value::Array(a)) => a,
        Some(Value::Object(o)) => match o.get("entries") {
            Some(Value::Array(a)) => a,
            _ => return out,
        },
        _ => return out,
    };
    for e in entries {
        if e.get("read_state_type").and_then(Value::as_u64).unwrap_or(0) != 0 {
            continue;
        }
        let Some(cid) = e.get("id").and_then(snowflake_value) else { continue };
        let last_read = e.get("last_message_id").and_then(snowflake_value).unwrap_or(0);
        let mentions = e.get("mention_count").and_then(Value::as_u64).unwrap_or(0) as u32;
        out.insert(cid, ReadMark { last_read, mentions });
    }
    out
}

#[derive(Default, Debug)]
pub struct MuteSettings {
    pub guilds: HashSet<u64>,
    pub channels: HashSet<u64>,
    pub suppress_everyone: HashSet<u64>,
    pub suppress_roles: HashSet<u64>,
    pub opt_in: HashSet<u64>,
    pub opted_in: HashSet<u64>,
}

impl MuteSettings {
    pub fn apply_guild_entry(&mut self, g: u64, entry: &Value) {
        for set in [&mut self.guilds, &mut self.suppress_everyone, &mut self.suppress_roles, &mut self.opt_in] {
            set.remove(&g);
        }
        if let Some(Value::Array(ovs)) = entry.get("channel_overrides") {
            for c in ovs.iter().filter_map(|o| o.get("channel_id").and_then(snowflake_value)) {
                self.channels.remove(&c);
                self.opted_in.remove(&c);
            }
        }
        let new = parse_mutes(Some(&Value::Array(vec![entry.clone()])));
        self.guilds.extend(new.guilds);
        self.channels.extend(new.channels);
        self.suppress_everyone.extend(new.suppress_everyone);
        self.suppress_roles.extend(new.suppress_roles);
        self.opt_in.extend(new.opt_in);
        self.opted_in.extend(new.opted_in);
    }
}

pub fn parse_mutes(v: Option<&Value>) -> MuteSettings {
    let mut out = MuteSettings::default();
    let entries = match v {
        Some(Value::Array(a)) => a,
        Some(Value::Object(o)) => match o.get("entries") {
            Some(Value::Array(a)) => a,
            _ => return out,
        },
        _ => return out,
    };
    for e in entries {
        let muted = crate::notifications::mute_now(e);
        if muted && let Some(g) = e.get("guild_id").and_then(snowflake_value) {
            out.guilds.insert(g);
        }
        if let Some(g) = e.get("guild_id").and_then(snowflake_value) {
            if e.get("suppress_everyone").and_then(Value::as_bool).unwrap_or(false) {
                out.suppress_everyone.insert(g);
            }
            if e.get("suppress_roles").and_then(Value::as_bool).unwrap_or(false) {
                out.suppress_roles.insert(g);
            }
            if e.get("flags").and_then(Value::as_u64).unwrap_or(0) & 16384 != 0 {
                out.opt_in.insert(g);
            }
        }
        if let Some(Value::Array(ovs)) = e.get("channel_overrides") {
            for o in ovs {
                let Some(c) = o.get("channel_id").and_then(snowflake_value) else { continue };
                if crate::notifications::mute_now(o) {
                    out.channels.insert(c);
                }
                if o.get("flags").and_then(Value::as_u64).unwrap_or(0) & 4096 != 0 {
                    out.opted_in.insert(c);
                }
            }
        }
    }
    out
}

struct Pb<'a>(&'a [u8]);

impl<'a> Pb<'a> {
    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let (&b, rest) = self.0.split_first()?;
            self.0 = rest;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn field(&mut self) -> Option<(u64, u8, &'a [u8], u64)> {
        if self.0.is_empty() {
            return None;
        }
        let key = self.varint()?;
        let (num, wt) = (key >> 3, (key & 7) as u8);
        match wt {
            0 => Some((num, wt, &[][..], self.varint()?)),
            1 => {
                let b = self.take(8)?;
                Some((num, wt, b, u64::from_le_bytes(b.try_into().ok()?)))
            }
            2 => {
                let n = self.varint()? as usize;
                Some((num, wt, self.take(n)?, 0))
            }
            5 => {
                let b = self.take(4)?;
                Some((num, wt, b, u64::from(u32::from_le_bytes(b.try_into().ok()?))))
            }
            _ => None,
        }
    }
}

fn fixed64s(wt: u8, payload: &[u8], scalar: u64, out: &mut Vec<u64>) {
    match wt {
        1 => out.push(scalar),
        2 => out.extend(payload.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c))),
        _ => {}
    }
}

pub fn guild_order_from_proto(b64: &str) -> Option<Vec<u64>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()?;
    let mut top = Pb(&bytes);
    while let Some((num, wt, payload, _)) = top.field() {
        if num != 14 || wt != 2 {
            continue;
        }
        let (mut folders, mut positions) = (Vec::new(), Vec::new());
        let mut gf = Pb(payload);
        while let Some((n, w, p, s)) = gf.field() {
            match n {
                1 if w == 2 => {
                    let mut folder = Pb(p);
                    while let Some((fnum, fw, fp, fs)) = folder.field() {
                        if fnum == 1 {
                            fixed64s(fw, fp, fs, &mut folders);
                        }
                    }
                }
                2 => fixed64s(w, p, s, &mut positions),
                _ => {}
            }
        }
        let order = if folders.is_empty() { positions } else { folders };
        return (!order.is_empty()).then_some(order);
    }
    None
}

pub fn guild_order_from_json(v: Option<&Value>) -> Option<Vec<u64>> {
    let v = v?;
    if let Some(Value::Array(folders)) = v.get("guild_folders") {
        let ids: Vec<u64> = folders
            .iter()
            .filter_map(|f| f.get("guild_ids").and_then(Value::as_array))
            .flatten()
            .filter_map(snowflake_value)
            .collect();
        if !ids.is_empty() {
            return Some(ids);
        }
    }
    let ids: Vec<u64> = v.get("guild_positions")?.as_array()?.iter().filter_map(snowflake_value).collect();
    (!ids.is_empty()).then_some(ids)
}

pub fn order_guilds(guilds: &mut [api::Snowflake], order: Option<&[u64]>) {
    let Some(order) = order else { return };
    let pos: HashMap<u64, usize> = order.iter().enumerate().map(|(i, g)| (*g, i + 1)).collect();
    guilds.sort_by_key(|g| pos.get(&g.0).copied().unwrap_or(0));
}

fn channel_kind(k: api::ChannelType) -> Option<model::ChannelKind> {
    use api::ChannelType as T;
    Some(match k {
        T::GuildText | T::Dm | T::GroupDm => model::ChannelKind::Text,
        T::GuildVoice => model::ChannelKind::Voice,
        T::GuildAnnouncement => model::ChannelKind::Announcement,
        T::GuildForum | T::GuildMedia => model::ChannelKind::Forum,
        T::GuildStageVoice => model::ChannelKind::Stage,
        T::GuildCategory => model::ChannelKind::Category,
        _ => return None,
    })
}

pub struct ReadCtx<'a> {
    pub reads: &'a HashMap<u64, ReadMark>,
    pub mutes: &'a MuteSettings,
}

impl ReadCtx<'_> {
    pub fn unread(&self, c: &api::Channel) -> (bool, u32) {
        if c.kind.is_voice() || c.kind == api::ChannelType::GuildCategory || c.is_guild_resource() {
            return (false, 0);
        }
        let last = c.last_message_id.map(|m| m.0).unwrap_or(0);
        match self.reads.get(&c.id.0) {
            Some(r) if c.kind.is_private() && c.flags & (1 << 23) != 0 => (last > r.last_read, 0),
            Some(r) => (last > r.last_read, r.mentions),
            None => (c.kind.is_private() && last != 0, 0),
        }
    }
}

fn channel(c: &api::Channel, position: i32, rc: &ReadCtx, voice_users: Vec<Id>) -> Option<model::Channel> {
    let kind = channel_kind(c.kind)?;
    let (unread, mentions) = rc.unread(c);
    Some(model::Channel {
        id: id(c.id),
        guild: c.guild_id.map(id),
        parent: c.parent_id.map(id),
        name: c.display_name().into(),
        topic: c.topic.clone().filter(|t| !t.is_empty()).map(Into::into),
        kind,
        position,
        unread,
        muted: rc.mutes.channels.contains(&c.id.0),
        mentions,
        voice_users,
    })
}

pub fn guild_channels(cache: &Cache, guild: api::Snowflake, rc: &ReadCtx) -> Vec<model::Channel> {
    let perms_known = cache.current_user().is_some_and(|me| cache.member(guild, me.id).is_some());
    let all: Vec<_> = cache
        .guild_channels(guild)
        .into_iter()
        .filter(|c| !c.is_obfuscated() && !c.kind.is_thread() && channel_kind(c.kind).is_some())
        .filter(|c| !c.is_guild_resource() || !rc.mutes.opt_in.contains(&guild.0))
        .filter(|c| !perms_known || cache.permissions_in(c.id).contains(api::Permissions::VIEW_CHANNEL))
        .collect();
    let key = |c: &api::Channel| (c.kind.is_voice(), c.position.unwrap_or(0), c.id.0);
    let mut top: Vec<_> = all.iter().filter(|c| c.kind != api::ChannelType::GuildCategory && c.parent_id.is_none()).collect();
    top.sort_by_key(|c| key(c));
    let mut cats: Vec<_> = all.iter().filter(|c| c.kind == api::ChannelType::GuildCategory).collect();
    cats.sort_by_key(|c| (c.position.unwrap_or(0), c.id.0));

    let mut voice: HashMap<u64, Vec<Id>> = HashMap::new();
    for v in cache.voice_states(Some(guild)) {
        if let Some(ch) = v.channel_id {
            voice.entry(ch.0).or_default().push(id(v.user_id));
        }
    }

    let mut ordered: Vec<&api::Channel> = top.iter().map(|c| &***c).collect();
    for cat in cats {
        let mut kids: Vec<_> = all.iter().filter(|c| c.parent_id == Some(cat.id) && c.kind != api::ChannelType::GuildCategory).collect();
        if kids.is_empty() {
            continue;
        }
        kids.sort_by_key(|c| key(c));
        ordered.push(cat);
        ordered.extend(kids.into_iter().map(|c| &**c));
    }
    for c in &all {
        if let Some(p) = c.parent_id
            && c.kind != api::ChannelType::GuildCategory
            && !ordered.iter().any(|o| o.id == c.id)
            && !all.iter().any(|x| x.id == p)
        {
            ordered.push(c);
        }
    }
    ordered
        .into_iter()
        .enumerate()
        .filter_map(|(i, c)| channel(c, i as i32, rc, voice.remove(&c.id.0).unwrap_or_default()))
        .collect()
}

pub fn dm_channel(c: &api::Channel, rc: &ReadCtx) -> Option<model::Channel> {
    if !c.kind.is_private() || c.guild_id.is_some() {
        return None;
    }
    let recip = c.recipient_user_ids().first().copied().map(id).into_iter().collect();
    channel(c, 0, rc, recip)
}

pub fn dm_channels(cache: &Cache, rc: &ReadCtx) -> Vec<model::Channel> {
    let mut dms = cache.private_channels();
    dms.sort_by_key(|c| std::cmp::Reverse(c.last_message_id.unwrap_or(c.id).0));
    dms.iter().filter_map(|c| dm_channel(c, rc)).collect()
}

pub fn roles(g: &api::Guild) -> Vec<model::Role> {
    g.roles
        .iter()
        .inspect(|r| {
            let c = r.colors.as_ref();
            let primary = c.map(|c| c.primary_color).filter(|&v| v != 0).unwrap_or(r.color);
            crate::ui::role_gradient::set(id(r.id), primary, c.and_then(|c| c.secondary_color), c.and_then(|c| c.tertiary_color));
        })
        .map(|r| model::Role {
            id: id(r.id),
            name: r.name.clone().into(),
            color: Some(r.colors.as_ref().map(|c| c.primary_color).filter(|c| *c != 0).unwrap_or(r.color)).filter(|c| *c != 0),
            position: r.position,
            hoist: r.hoist,
            icon: r
                .icon
                .as_ref()
                .map(|h| format!("https://cdn.discordapp.com/role-icons/{}/{h}.webp?size=40&quality=lossless", r.id).into())
                .or_else(|| r.unicode_emoji.as_deref().and_then(crate::media::emoji::twemoji_url)),
            managed: r.managed,
            connection: r.tags.as_ref().is_some_and(|t| t.guild_connections),
        })
        .collect()
}

pub fn guild_with_threads(cache: &Cache, g: &api::Guild, channels: &[model::Channel], rc: &ReadCtx) -> model::Guild {
    let muted = rc.mutes.guilds.contains(&g.id.0);
    let mut out = guild(g, channels, muted);
    for t in cache.guild_channels(g.id) {
        let Some(m) = t.member.as_ref() else { continue };
        if !t.kind.is_thread() || t.thread_metadata.as_ref().is_some_and(|m| m.archived) {
            continue;
        }
        let (unread, mentions) = rc.unread(&t);
        let thread_muted = m.get("muted").and_then(Value::as_bool).unwrap_or(false);
        out.unread |= unread && !muted && !thread_muted;
        out.mentions += mentions;
    }
    out
}

pub fn guild(g: &api::Guild, channels: &[model::Channel], muted: bool) -> model::Guild {
    let muted_cats: HashSet<Id> = channels.iter().filter(|c| c.kind == model::ChannelKind::Category && c.muted).map(|c| c.id).collect();
    let visible = channels.iter().filter(|c| !c.muted && !c.parent.is_some_and(|p| muted_cats.contains(&p)));
    model::Guild {
        id: id(g.id),
        name: g.name().to_owned().into(),
        acronym: model::acronym(g.name()),
        icon: match g.icon_url(ICON_SIZE) {
            Some(u) => model::ImageRef::Url(u.into()),
            None => model::ImageRef::Default((g.id.0 % 5) as u8),
        },
        unread: !muted && visible.clone().any(|c| c.unread),
        mentions: channels.iter().map(|c| c.mentions).sum(),
        banner_color: None,
        owner: g.owner_id().map(id),
        discoverable: g.properties.features.iter().any(|f| f == "DISCOVERABLE"),
        community: g.properties.features.iter().any(|f| f == "COMMUNITY"),
        boosts: g.properties.premium_subscription_count.unwrap_or(0),
        boost_tier: g.properties.premium_tier.unwrap_or(0),
        afk_channel: g.properties.afk_channel_id.map(id),
        system_channel: g.properties.system_channel_id.map(id),
        rules_channel: g.properties.rules_channel_id.map(id),
        member_count: g.member_count,
    }
}

pub struct Prefs {
    pub reads: HashMap<u64, ReadMark>,
    pub mutes: MuteSettings,
    pub blocked: HashSet<u64>,
}

impl Prefs {
    pub fn from_ready(r: &Ready) -> Self {
        Self {
            reads: parse_read_state(r.read_state.as_ref()),
            mutes: parse_mutes(r.user_guild_settings.as_ref()),
            blocked: r
                .relationships
                .iter()
                .filter(|x| x.kind == api::RelationshipType::Blocked)
                .map(|x| x.id.0)
                .collect(),
        }
    }
    pub fn ctx(&self) -> ReadCtx<'_> {
        ReadCtx { reads: &self.reads, mutes: &self.mutes }
    }
}

pub fn build_ready(cache: &Cache, ready: &Ready, prefs: &Prefs) -> ReadyData {
    let rc = prefs.ctx();
    let me = cache.current_user().map(|u| user(&u)).unwrap_or_else(|| user(&ready.user));
    crate::ui::settings::account::set_from_user(cache.current_user().as_deref().unwrap_or(&ready.user));
    crate::ui::settings::sessions::set_current_hash(ready.auth_session_id_hash.clone());
    crate::ui::settings::account::set_connections(ready.connected_accounts.len());
    crate::ui::settings::synced_map::set_ready_flags(crate::ui::settings::synced_map::ReadyFlags::from_ready(
        ready.ad_personalization_toggles_disabled,
        ready.swp_message_promotion_locked,
        ready.swp_message_promotion_default_opted_out,
    ));

    let mut ids: Vec<api::Snowflake> = cache.guilds().iter().filter(|g| !g.unavailable).map(|g| g.id).collect();
    let ready_pos: HashMap<u64, usize> = ready.guilds.iter().enumerate().map(|(i, g)| (g.id.0, i)).collect();
    ids.sort_by_key(|g| ready_pos.get(&g.0).copied().unwrap_or(usize::MAX));
    let order = ready
        .user_settings_proto
        .as_deref()
        .and_then(guild_order_from_proto)
        .or_else(|| guild_order_from_json(ready.user_settings.as_ref()));
    order_guilds(&mut ids, order.as_deref());

    let mut guilds = Vec::new();
    let mut channels = Vec::new();
    let mut roles_out = Vec::new();
    for gid in ids {
        let Some(g) = cache.guild(gid) else { continue };
        let chs = guild_channels(cache, gid, &rc);
        guilds.push(guild_with_threads(cache, &g, &chs, &prefs.ctx()));
        roles_out.push((id(gid), roles(&g)));
        channels.extend(chs);
    }

    let mut users: HashMap<u64, model::User> = HashMap::new();
    for u in ready.users.iter().chain(ready.relationships.iter().filter_map(|r| r.user.as_ref())) {
        users.insert(u.id.0, user(u));
    }
    for c in cache.private_channels() {
        for u in &c.recipients {
            users.entry(u.id.0).or_insert_with(|| user(u));
        }
    }
    users.insert(me.id.0, me.clone());

    ReadyData {
        me,
        users: users.into_values().collect(),
        guilds,
        channels,
        roles: roles_out,
        dm_channels: dm_channels(cache, &rc),
        presences: Vec::new(),
    }
}

#[derive(Default)]
struct MemberList {
    items: Vec<Option<MemberListItem>>,
}

impl MemberList {
    fn ensure(&mut self, len: usize) {
        if self.items.len() < len {
            self.items.resize(len, None);
        }
    }
    fn apply(&mut self, ops: &[MemberListOp]) {
        for op in ops {
            match op {
                MemberListOp::Sync { range, items } => {
                    let start = range[0] as usize;
                    self.ensure(start + items.len());
                    for (i, it) in items.iter().enumerate() {
                        self.items[start + i] = Some(it.clone());
                    }
                }
                MemberListOp::Insert { index, item } => {
                    let i = (*index as usize).min(self.items.len());
                    self.items.insert(i, Some(item.clone()));
                }
                MemberListOp::Update { index, item } => {
                    let i = *index as usize;
                    self.ensure(i + 1);
                    self.items[i] = Some(item.clone());
                }
                MemberListOp::Delete { index } => {
                    let i = *index as usize;
                    if i < self.items.len() {
                        self.items.remove(i);
                    }
                }
                MemberListOp::Invalidate { range } => {
                    let end = (range[1] as usize + 1).min(self.items.len());
                    for it in self.items.iter_mut().take(end).skip(range[0] as usize) {
                        *it = None;
                    }
                }
            }
        }
    }
    fn members(&self, guild: api::GuildId) -> (Vec<model::Member>, Vec<model::User>) {
        let mut seen = HashSet::new();
        let (mut ms, mut us) = (Vec::new(), Vec::new());
        for it in self.items.iter().flatten() {
            let MemberListItem::Member(m) = it else { continue };
            let Some(u) = &m.member.user else { continue };
            if !seen.insert(u.id.0) {
                continue;
            }
            us.push(user(u));
            let p = m.presence.as_ref();
            if let Some(p) = p {
                crate::ui::mobile_status::note_value(u.id.0, p);
            }
            ms.push(model::Member {
                user: id(u.id),
                roles: m.member.roles.iter().copied().map(id).collect(),
                presence: presence(p.and_then(|p| p.get("status")).and_then(Value::as_str).unwrap_or("offline")),
                activity: p
                    .and_then(|p| p.get("activities"))
                    .and_then(Value::as_array)
                    .and_then(|a| activity(a)),
                nick: m.member.nick.as_deref().filter(|n| !n.is_empty()).map(|n| collapse_ws(n).into()),
                avatar: member_avatar_url(guild, u.id, &m.member),
                decoration: member_decoration_url(&m.member),
                nameplate: member_nameplate(&m.member),
                joined_at: joined_unix(&m.member),
                rejoined: m.member.flags & 1 != 0,
                premium_since: premium_unix(&m.member),
            });
        }
        (ms, us)
    }
}

pub struct Adapter {
    pub me: api::Snowflake,
    pub prefs: Prefs,
    lists: HashMap<u64, HashMap<String, MemberList>>,
    shown: HashMap<u64, String>,
    chunked: HashMap<u64, Vec<api::Snowflake>>,
    statuses: HashMap<u64, model::Presence>,
}

impl Adapter {
    pub fn new(me: api::Snowflake, prefs: Prefs) -> Self {
        Self { me, prefs, lists: HashMap::new(), shown: HashMap::new(), chunked: HashMap::new(), statuses: HashMap::new() }
    }

    pub fn mentions_me(&self, cache: &Cache, m: &api::Message) -> bool {
        if m.author.id == self.me || self.prefs.blocked.contains(&m.author.id.0) {
            return false;
        }
        let guild = m.guild_id.or_else(|| cache.channel(m.channel_id).and_then(|channel| channel.guild_id));
        let g = guild.map(|g| g.0);
        let sup_every = g.is_some_and(|g| self.prefs.mutes.suppress_everyone.contains(&g));
        let sup_roles = g.is_some_and(|g| self.prefs.mutes.suppress_roles.contains(&g));
        m.mentions_user(self.me)
            || (m.mention_everyone && !sup_every)
            || (!sup_roles
                && guild.and_then(|g| cache.member(g, self.me)).is_some_and(|me| m.mention_roles.iter().any(|r| me.roles.contains(r))))
    }

    pub fn recount_mentions(&mut self, cache: &Cache, channel: u64, latest_page: &[api::Message]) -> Option<AppEvent> {
        let last_read = self.prefs.reads.get(&channel).map(|r| r.last_read)?;
        let oldest = latest_page.iter().map(|m| m.id.0).min()?;
        if oldest > last_read {
            return None;
        }
        let n = latest_page.iter().filter(|m| m.id.0 > last_read && self.mentions_me(cache, m)).count() as u32;
        let e = self.prefs.reads.entry(channel).or_default();
        if e.mentions == n {
            return None;
        }
        e.mentions = n;
        let g = cache.channel(api::Snowflake(channel))?.guild_id?;
        let gg = cache.guild(g)?;
        let chs = guild_channels(cache, g, &self.prefs.ctx());
        let mg = guild_with_threads(cache, &gg, &chs, &self.prefs.ctx());
        Some(AppEvent::GuildUnread { guild: mg.id, unread: mg.unread, mentions: mg.mentions })
    }

    pub fn mark_read(&mut self, channel: u64, msg: u64) {
        let e = self.prefs.reads.entry(channel).or_default();
        e.last_read = e.last_read.max(msg);
        e.mentions = 0;
    }

    fn guild_state(&self, cache: &Cache, g: api::Snowflake, out: &mut Vec<AppEvent>) {
        let rc = self.prefs.ctx();
        let chs = guild_channels(cache, g, &rc);
        if let Some(gg) = cache.guild(g) {
            let m = guild_with_threads(cache, &gg, &chs, &self.prefs.ctx());
            out.push(AppEvent::GuildUnread { guild: m.id, unread: m.unread, mentions: m.mentions });
        }
        out.push(AppEvent::GuildChannels { guild: id(g), channels: chs });
    }

    fn list_update(&mut self, cache: &Cache, l: &GuildMemberListUpdate, out: &mut Vec<AppEvent>) {
        let has_full_sync = l.ops.iter().any(|o| matches!(o, MemberListOp::Sync { range, .. } if range[0] == 0));
        let lists = self.lists.entry(l.guild_id.0).or_default();
        if !lists.contains_key(&l.id) && !has_full_sync {
            return;
        }
        let list = lists.entry(l.id.clone()).or_default();
        list.apply(&l.ops);
        for it in list.items.iter_mut().flatten() {
            if let MemberListItem::Group { id: gid, count } = it
                && let Some(g) = l.groups.iter().find(|g| &g.id == gid)
            {
                *count = g.count;
            }
        }
        let total: usize = l.groups.iter().filter(|g| g.count > 0).map(|g| g.count as usize + 1).sum();
        if total > 0 {
            list.items.resize(total, None);
        }
        let shown = self.shown.entry(l.guild_id.0).or_insert_with(|| l.id.clone());
        if *shown != l.id {
            return;
        }
        self.emit_list(cache, l.guild_id, out);
    }

    pub fn show_list(&mut self, cache: &Cache, channel: api::ChannelId) -> Vec<AppEvent> {
        let mut out = Vec::new();
        let Some(g) = cache.channel(channel).and_then(|c| c.guild_id) else { return out };
        let Some(list_id) = super::member_list_id::for_channel(cache, channel) else {
            self.shown.remove(&g.0);
            return out;
        };
        if self.shown.get(&g.0) == Some(&list_id) {
            return out;
        }
        let synced = self.lists.get(&g.0).is_some_and(|l| l.contains_key(&list_id));
        self.shown.insert(g.0, list_id);
        if synced {
            self.emit_list(cache, g, &mut out);
        }
        out
    }

    fn shown_list(&self, guild: u64) -> Option<&MemberList> {
        self.lists.get(&guild)?.get(self.shown.get(&guild)?)
    }

    fn emit_list(&self, cache: &Cache, guild: api::GuildId, out: &mut Vec<AppEvent>) {
        let Some(list) = self.shown_list(guild.0) else { return };
        let (rows, acts) = crate::presence::list_rows(list.items.iter());
        self.emit_members(cache, guild, out);
        let me = id(self.me);
        out.extend(acts.into_iter().filter(|(user, _)| *user != me).map(|(user, activities)| AppEvent::Activities { user, activities }));
        out.push(AppEvent::MemberListRows { guild: id(guild), rows });
    }

    pub fn note_authors(&mut self, cache: &Cache, guild: api::GuildId, users: impl IntoIterator<Item = api::Snowflake>) -> (Vec<AppEvent>, Vec<api::Snowflake>) {
        let mut missing = Vec::new();
        let mut added = false;
        for u in users {
            if cache.member(guild, u).is_some() {
                let ids = self.chunked.entry(guild.0).or_default();
                if !ids.contains(&u) {
                    ids.push(u);
                    added = true;
                }
            } else if !missing.contains(&u) {
                missing.push(u);
            }
        }
        let mut out = Vec::new();
        if added {
            self.emit_members(cache, guild, &mut out);
        }
        (out, missing)
    }

    pub fn take_authors(&mut self) -> HashMap<u64, Vec<api::Snowflake>> {
        std::mem::take(&mut self.chunked)
    }

    fn emit_members(&self, cache: &Cache, guild: api::GuildId, out: &mut Vec<AppEvent>) {
        let (mut members, users) = self.shown_list(guild.0).map(|l| l.members(guild)).unwrap_or_default();
        let voice: Vec<api::Snowflake> = cache.voice_states(Some(guild)).iter().map(|v| v.user_id).collect();
        let extra = std::iter::once(self.me)
            .chain(self.chunked.get(&guild.0).into_iter().flatten().copied())
            .chain(voice);
        for u in extra {
            if members.iter().any(|m| m.user == id(u)) {
                continue;
            }
            if let Some(m) = cache.member(guild, u) {
                members.push(model::Member {
                    user: id(u),
                    roles: m.roles.iter().copied().map(id).collect(),
                    presence: if u == self.me {
                        model::Presence::Online
                    } else {
                        self.statuses.get(&u.0).copied().unwrap_or(model::Presence::Offline)
                    },
                    activity: None,
                    nick: m.nick.as_deref().filter(|n| !n.is_empty()).map(|n| collapse_ws(n).into()),
                    avatar: member_avatar_url(guild, u, &m),
                    decoration: member_decoration_url(&m),
                    nameplate: member_nameplate(&m),
                    joined_at: joined_unix(&m),
                    rejoined: m.flags & 1 != 0,
                    premium_since: premium_unix(&m),
                });
            }
        }
        out.push(AppEvent::Users(users));
        out.push(AppEvent::GuildMembers { guild: id(guild), members });
    }

    pub fn map(&mut self, cache: &Cache, ev: &GatewayEvent) -> Vec<AppEvent> {
        let mut out = Vec::new();
        match ev {
            GatewayEvent::MessageCreate(m) => {
                let mut users = Vec::new();
                message_users(m, &mut users);
                out.push(AppEvent::Users(users));
                let channel = cache.channel(m.channel_id);
                let guild = m.guild_id.or_else(|| channel.as_ref().and_then(|c| c.guild_id));
                let is_dm = guild.is_none() && channel.as_ref().is_some_and(|c| c.kind.is_private());
                let mentioned = self.mentions_me(cache, m)
                    || (is_dm
                        && m.author.id != self.me
                        && !self.prefs.blocked.contains(&m.author.id.0)
                        && !self.prefs.mutes.channels.contains(&m.channel_id.0));
                if m.author.id == self.me {
                    self.mark_read(m.channel_id.0, m.id.0);
                } else if mentioned {
                    self.prefs.reads.entry(m.channel_id.0).or_default().mentions += 1;
                }
                out.push(AppEvent::MessageCreate(message(m)));
                if let Some(c) = channel.as_deref()
                    && guild.is_some()
                {
                    let (unread, mentions) = self.prefs.ctx().unread(c);
                    out.push(AppEvent::ChannelUnread { channel: id(m.channel_id), unread, mentions });
                }
                if let Some(g) = guild
                    && let Some(gg) = cache.guild(g)
                {
                    let chs = guild_channels(cache, g, &self.prefs.ctx());
                    let mg = guild_with_threads(cache, &gg, &chs, &self.prefs.ctx());
                    out.push(AppEvent::GuildUnread { guild: mg.id, unread: mg.unread, mentions: mg.mentions });
                } else if is_dm
                    && let Some(dm) = channel.as_deref().and_then(|c| dm_channel(c, &self.prefs.ctx()))
                {
                    out.push(AppEvent::DmUpsert(dm));
                }
            }
            GatewayEvent::ChannelUnreads { guild_id, channels } => {
                let ctx = self.prefs.ctx();
                for c in channels.iter().filter_map(|p| cache.channel(p.id)) {
                    let (unread, mentions) = ctx.unread(&c);
                    out.push(AppEvent::ChannelUnread { channel: id(c.id), unread, mentions });
                }
                if let Some(gg) = cache.guild(*guild_id) {
                    let chs = guild_channels(cache, *guild_id, &ctx);
                    let mg = guild_with_threads(cache, &gg, &chs, &ctx);
                    out.push(AppEvent::GuildUnread { guild: mg.id, unread: mg.unread, mentions: mg.mentions });
                }
            }
            GatewayEvent::MessageUpdate(v) => {
                let cid = v.get("channel_id").and_then(snowflake_value);
                let mid = v.get("id").and_then(snowflake_value);
                if let (Some(c), Some(mid)) = (cid, mid)
                    && let Some(m) = cache.message(api::Snowflake(c), api::Snowflake(mid))
                {
                    out.push(AppEvent::MessageUpdate(message(&m)));
                }
            }
            GatewayEvent::MessageReactionAdd(r) | GatewayEvent::MessageReactionRemove(r) => {
                if let Some(m) = cache.message(r.channel_id, r.message_id) {
                    let conv = message(&m);
                    let burst = (r.burst && matches!(ev, GatewayEvent::MessageReactionAdd(_)))
                        .then(|| conv.reactions.iter().find(|x| burst_target(x, &r.emoji)).cloned())
                        .flatten();
                    out.push(AppEvent::MessageUpdate(conv));
                    if let Some(x) = burst {
                        out.push(AppEvent::BurstReaction { channel: id(r.channel_id), msg: id(r.message_id), emoji: x.emoji, emoji_id: x.emoji_id, colors: x.burst_colors });
                    }
                }
            }
            GatewayEvent::MessageReactionAddMany(r) => {
                if let Some(m) = cache.message(r.channel_id, r.message_id) {
                    out.push(AppEvent::MessageUpdate(message(&m)));
                }
            }
            GatewayEvent::MessageReactionRemoveAll(r) | GatewayEvent::MessageReactionRemoveEmoji(r) => {
                if let Some(m) = cache.message(r.channel_id, r.message_id) {
                    out.push(AppEvent::MessageUpdate(message(&m)));
                }
            }
            GatewayEvent::MessageDelete(d) => {
                out.push(AppEvent::MessageDelete { channel: id(d.channel_id), id: id(d.id) });
            }
            GatewayEvent::MessageDeleteBulk(d) => {
                out.extend(d.ids.iter().map(|m| AppEvent::MessageDelete { channel: id(d.channel_id), id: id(*m) }));
            }
            GatewayEvent::MessageAck(a) => {
                self.mark_read(a.channel_id.0, a.message_id.0);
                out.push(AppEvent::ChannelUnread { channel: id(a.channel_id), unread: false, mentions: 0 });
                if let Some(c) = cache.channel(a.channel_id)
                    && let Some(g) = c.guild_id
                    && let Some(gg) = cache.guild(g)
                {
                    let chs = guild_channels(cache, g, &self.prefs.ctx());
                    let mg = guild_with_threads(cache, &gg, &chs, &self.prefs.ctx());
                    out.push(AppEvent::GuildUnread { guild: mg.id, unread: mg.unread, mentions: mg.mentions });
                }
            }
            GatewayEvent::TypingStart(t) => {
                if let Some(u) = t.member.as_ref().and_then(|m| m.user.as_ref()) {
                    out.push(AppEvent::Users(vec![user(u)]));
                }
                let nick = t.member.as_ref().and_then(|m| m.nick.clone()).map(SharedString::from);
                out.push(AppEvent::TypingStart { channel: id(t.channel_id), user: id(t.user_id), nick });
            }
            GatewayEvent::PresenceUpdate(p) => {
                if p.user.username.is_some()
                    && let Some(u) = cache.user(p.user.id)
                {
                    out.push(AppEvent::Users(vec![user(&u)]));
                }
                if p.user.id != self.me {
                    self.statuses.insert(p.user.id.0, presence(&p.status));
                    crate::ui::mobile_status::note(p.user.id.0, &p.status, p.client_status.as_ref());
                    out.push(AppEvent::PresenceUpdate { user: id(p.user.id), presence: presence(&p.status) });
                    let activities = crate::presence::parse_activities(&p.activities);
                    out.push(AppEvent::Activities { user: id(p.user.id), activities });
                }
            }
            GatewayEvent::ReadySupplemental(r) => {
                let me = self.me.0;
                self.statuses.retain(|u, _| *u == me);
                if let Some(Value::Object(mp)) = &r.merged_presences {
                    let friends = mp.get("friends").and_then(Value::as_array).into_iter().flatten();
                    let guilds = mp.get("guilds").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_array).flatten();
                    for p in friends.chain(guilds) {
                        let uid = p.get("user_id").or_else(|| p.get("user").and_then(|u| u.get("id"))).and_then(snowflake_value);
                        if let Some(u) = uid.filter(|u| *u != me) {
                            let st = p.get("status").and_then(Value::as_str).unwrap_or("offline");
                            self.statuses.insert(u, presence(st));
                            crate::ui::mobile_status::note_value(u, p);
                            out.push(AppEvent::PresenceUpdate { user: Id(u), presence: presence(st) });
                            let a = p.get("activities").and_then(Value::as_array).map(|a| crate::presence::parse_activities(a));
                            out.push(AppEvent::Activities { user: Id(u), activities: a.unwrap_or_default() });
                        }
                    }
                }
                for g in &r.guilds {
                    out.push(AppEvent::GuildVoiceReset(id(g.id)));
                    for v in &g.voice_states {
                        if v.channel_id.is_some() {
                            out.push(AppEvent::VoiceStateUpdate { guild: Some(id(g.id)), channel: v.channel_id.map(id), user: id(v.user_id), own: false });
                            out.push(AppEvent::VoiceFlags { user: id(v.user_id), flags: voice_flags(v) });
                        }
                    }
                }
            }
            GatewayEvent::GuildMemberListUpdate(l) => self.list_update(cache, l, &mut out),
            GatewayEvent::GuildMembersChunk(c) => {
                out.push(AppEvent::Users(c.members.iter().filter_map(|m| m.user.as_ref()).map(user).collect()));
                for p in &c.presences {
                    let uid = p.get("user").and_then(|u| u.get("id")).or_else(|| p.get("user_id")).and_then(snowflake_value);
                    if let Some(u) = uid.filter(|u| *u != self.me.0) {
                        let st = presence(p.get("status").and_then(Value::as_str).unwrap_or("offline"));
                        self.statuses.insert(u, st);
                        crate::ui::mobile_status::note_value(u, p);
                        out.push(AppEvent::PresenceUpdate { user: Id(u), presence: st });
                        let a = p.get("activities").and_then(Value::as_array).map(|a| crate::presence::parse_activities(a));
                        out.push(AppEvent::Activities { user: Id(u), activities: a.unwrap_or_default() });
                    }
                }
                let ids = self.chunked.entry(c.guild_id.0).or_default();
                for u in c.members.iter().filter_map(|m| m.user_id()) {
                    if !ids.contains(&u) {
                        ids.push(u);
                    }
                }
                self.emit_members(cache, c.guild_id, &mut out);
            }
            GatewayEvent::UserUpdate(u) => out.push(AppEvent::Users(vec![user(u)])),
            GatewayEvent::VoiceChannelEffectSend(e) => out.push(AppEvent::VoiceEffect(crate::voice::soundboard::Effect {
                channel: Id(e.channel_id.0),
                user: Id(e.user_id.0),
                sound: e.sound_id.map(|x| Id(x.0)),
                sound_volume: e.sound_volume.unwrap_or(1.0) as f32,
                emoji_id: e.emoji.as_ref().and_then(|x| x.id).map(|x| Id(x.0)),
                emoji_name: e.emoji.as_ref().and_then(|x| x.name.clone()).map(Into::into),
                emoji_animated: e.emoji.as_ref().is_some_and(|x| x.animated),
                animation: e.animation_type.zip(e.animation_id),
            })),
            GatewayEvent::SoundboardSounds(s) => {
                let sounds = s
                    .soundboard_sounds
                    .iter()
                    .map(|x| model::SoundboardSound {
                        id: Id(x.sound_id.0),
                        guild: x.guild_id.map(|g| Id(g.0)).or(Some(Id(s.guild_id.0))),
                        name: Some(x.name.clone().into()),
                        emoji_id: x.emoji_id.map(|e| Id(e.0)),
                        emoji_name: x.emoji_name.clone().map(Into::into),
                        volume: x.volume as f32,
                        available: x.available,
                        user: x.user_id.map(|u| Id(u.0)),
                    })
                    .collect();
                out.push(AppEvent::SoundboardSounds { guild: Id(s.guild_id.0), sounds });
            }
            GatewayEvent::GuildCreate(g) => {
                let rc = self.prefs.ctx();
                let chs = guild_channels(cache, g.id, &rc);
                if let Some(gg) = cache.guild(g.id) {
                    out.push(AppEvent::GuildCreate {
                        guild: guild_with_threads(cache, &gg, &chs, &self.prefs.ctx()),
                        channels: chs,
                        roles: roles(&gg),
                    });
                }
                out.extend(voice_flag_events(cache, Some(g.id)));
            }
            GatewayEvent::GuildUpdate(g) => {
                if let Some(gg) = cache.guild(g.id) {
                    let chs = guild_channels(cache, g.id, &self.prefs.ctx());
                    out.push(AppEvent::GuildCreate {
                        guild: guild_with_threads(cache, &gg, &chs, &self.prefs.ctx()),
                        channels: chs,
                        roles: roles(&gg),
                    });
                }
            }
            GatewayEvent::GuildDelete(d) => {
                if !d.unavailable {
                    out.push(AppEvent::GuildDelete(id(d.id)));
                }
            }
            GatewayEvent::GuildRoleCreate(r) | GatewayEvent::GuildRoleUpdate(r) => {
                if let Some(gg) = cache.guild(r.guild_id) {
                    out.push(AppEvent::GuildRoles { guild: id(r.guild_id), roles: roles(&gg) });
                }
                self.guild_state(cache, r.guild_id, &mut out);
            }
            GatewayEvent::GuildRoleDelete(r) => {
                if let Some(gg) = cache.guild(r.guild_id) {
                    out.push(AppEvent::GuildRoles { guild: id(r.guild_id), roles: roles(&gg) });
                }
                self.guild_state(cache, r.guild_id, &mut out);
            }
            GatewayEvent::GuildMemberUpdate(m) if m.user_id() == Some(self.me) => {
                if let Some(g) = m.guild_id {
                    self.guild_state(cache, g, &mut out);
                }
            }
            GatewayEvent::GuildMemberUpdate(m) => {
                if let Some(u) = &m.user {
                    out.push(AppEvent::Users(vec![user(u)]));
                }
                if let (Some(g), Some(u)) = (m.guild_id, m.user_id())
                    && self.chunked.get(&g.0).is_some_and(|c| c.contains(&u))
                {
                    self.emit_members(cache, g, &mut out);
                }
            }
            GatewayEvent::ChannelCreate(c) | GatewayEvent::ChannelUpdate(c) | GatewayEvent::ChannelDelete(c) => {
                let cached = cache.channel(c.id);
                let current = cached.as_deref().unwrap_or(c);
                if let Some(guild) = c.guild_id.or(current.guild_id) {
                    self.guild_state(cache, guild, &mut out);
                } else if let Some(dm) = dm_channel(current, &self.prefs.ctx()) {
                    if matches!(ev, GatewayEvent::ChannelDelete(_)) {
                        out.push(AppEvent::DmDelete(dm.id));
                    } else {
                        out.push(AppEvent::Users(current.recipients.iter().map(user).collect()));
                        out.push(AppEvent::DmUpsert(dm));
                    }
                }
            }
            GatewayEvent::VoiceStateUpdate(v) => {
                if let Some(u) = v.member.as_ref().and_then(|m| m.user.as_ref()) {
                    out.push(AppEvent::Users(vec![user(u)]));
                }
                let own = v.user_id.0 == self.me.0 && !v.session_id.is_empty() && v.session_id == crate::commands::session_id();
                out.push(AppEvent::VoiceStateUpdate { guild: v.guild_id.map(id), channel: v.channel_id.map(id), user: id(v.user_id), own });
                out.push(AppEvent::VoiceFlags { user: id(v.user_id), flags: voice_flags(v) });
            }
            GatewayEvent::Unknown { name, data } if name == "CHANNEL_RECIPIENT_ADD" || name == "CHANNEL_RECIPIENT_REMOVE" => {
                if let Some(c) = data.get("channel_id").and_then(snowflake_value).and_then(|c| cache.channel(api::Snowflake(c))) {
                    out.push(AppEvent::Users(c.recipients.iter().map(user).collect()));
                    if let Some(dm) = dm_channel(&c, &self.prefs.ctx()) {
                        out.push(AppEvent::DmUpsert(dm));
                    }
                }
            }
            GatewayEvent::Unknown { name, data } if name == "SESSIONS_REPLACE" => {
                let all = data.as_array().into_iter().flatten().find(|s| s.get("session_id").and_then(Value::as_str) == Some("all"));
                if let Some(st) = all.and_then(|s| s.get("status")).and_then(Value::as_str) {
                    let p = presence(st);
                    self.statuses.insert(self.me.0, p);
                    out.push(AppEvent::PresenceUpdate { user: id(self.me), presence: p });
                }
                if let Some(acts) = all.and_then(|s| s.get("activities")).and_then(Value::as_array) {
                    out.push(AppEvent::Activities { user: id(self.me), activities: crate::presence::parse_activities(acts) });
                }
            }
            GatewayEvent::Unknown { name, data } if name == "USER_GUILD_SETTINGS_UPDATE" => {
                if let Some(g) = data.get("guild_id").and_then(snowflake_value) {
                    self.prefs.mutes.apply_guild_entry(g, data);
                    self.guild_state(cache, api::Snowflake(g), &mut out);
                }
            }
            GatewayEvent::Fatal { code, reason } => {
                out.push(AppEvent::Disconnected(fatal_message(*code, reason)));
            }
            GatewayEvent::SessionReset => out.push(AppEvent::Connection(false)),
            GatewayEvent::Resumed => out.push(AppEvent::Connection(true)),
            _ => {}
        }
        out
    }
}

pub fn fatal_message(code: u16, reason: &str) -> String {
    match code {
        4004 => "Your session is no longer valid. Please log in again.".into(),
        _ if reason.is_empty() => format!("Disconnected from Discord (code {code})."),
        _ => format!("Disconnected from Discord: {reason} (code {code})."),
    }
}


pub fn burst_target(x: &model::Reaction, e: &api::ReactionEmoji) -> bool {
    match (x.emoji_id, e.id) {
        (Some(a), Some(b)) => a == id(b),
        (None, None) => e.name.as_deref() == Some(x.emoji.as_ref()),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod live_tests;

pub fn forum_tags(c: &api::Channel) -> Vec<crate::commands::ForumTag> {
    c.available_tags
        .iter()
        .map(|t| crate::commands::ForumTag {
            id: id(t.id),
            name: t.name.clone().into(),
            emoji: match (t.emoji_id, &t.emoji_name) {
                (Some(e), n) => Some(format!("<:{}:{}>", n.as_deref().unwrap_or("e"), e.0)),
                (None, Some(n)) if !n.is_empty() => Some(n.clone()),
                _ => None,
            },
            moderated: t.moderated,
        })
        .collect()
}

pub fn voice_flag_events(cache: &Cache, guild: Option<api::Snowflake>) -> Vec<AppEvent> {
    cache
        .voice_states(guild)
        .iter()
        .filter(|v| v.channel_id.is_some())
        .map(|v| AppEvent::VoiceFlags { user: id(v.user_id), flags: voice_flags(v) })
        .collect()
}

fn voice_flags(v: &phoenix_api::models::VoiceState) -> crate::model::VoiceFlags {
    crate::model::VoiceFlags {
        self_mute: v.self_mute,
        self_deaf: v.self_deaf,
        server_mute: v.mute,
        server_deaf: v.deaf,
        stream: v.self_stream.unwrap_or(false),
        video: v.self_video,
        suppress: v.suppress,
        undiscoverable: v.discoverable == Some(false),
        hand: v.request_to_speak_timestamp.as_deref().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|d| d.timestamp_millis()),
    }
}

pub fn self_status(r: &phoenix_api::gateway::events::Ready) -> Option<AppEvent> {
    let proto = r.user_settings_proto.as_deref().and_then(|b| base64::engine::general_purpose::STANDARD.decode(b.trim()).ok());
    let from_proto = proto.as_deref().and_then(|b| {
        let st = super::nav::fields(b).into_iter().find(|f| f.0 == 11 && f.1 == 2)?.2;
        let sv = super::nav::fields(st).into_iter().find(|f| f.0 == 1 && f.1 == 2)?.2;
        let v = super::nav::fields(sv).into_iter().find(|f| f.0 == 1 && f.1 == 2)?.2;
        std::str::from_utf8(v).ok().filter(|t| !t.is_empty()).map(str::to_owned)
    });
    let status = from_proto.or_else(|| {
        let sessions = r.sessions.as_ref()?.as_array()?;
        let s = sessions.iter().find(|s| s.get("session_id").and_then(Value::as_str) == Some("all")).or_else(|| sessions.first())?;
        s.get("status").and_then(Value::as_str).map(str::to_owned)
    })?;
    Some(AppEvent::PresenceUpdate { user: id(r.user.id), presence: presence(&status) })
}

pub fn self_activities(r: &phoenix_api::gateway::events::Ready) -> Option<AppEvent> {
    let proto = r.user_settings_proto.as_deref().and_then(|b| base64::engine::general_purpose::STANDARD.decode(b.trim()).ok());
    let from_proto = proto.as_deref().and_then(|b| {
        let st = super::nav::fields(b).into_iter().find(|f| f.0 == 11 && f.1 == 2)?.2;
        let cs = super::nav::fields(st).into_iter().find(|f| f.0 == 2 && f.1 == 2)?.2;
        let f = super::nav::fields(cs);
        let get = |n| f.iter().find(|x| x.0 == n && x.1 == 2).and_then(|x| std::str::from_utf8(x.2).ok()).filter(|t| !t.is_empty()).map(str::to_owned);
        let (text, emoji) = (get(1), get(3));
        let emoji_id = f.iter().find(|x| x.0 == 2 && (x.1 == 1 || x.1 == 0)).map(|x| x.3).filter(|v| *v != 0);
        (text.is_some() || emoji.is_some()).then_some((text, emoji, emoji_id))
    });
    if let Some((text, emoji, emoji_id)) = from_proto {
        let emoji = emoji.map(|e| match emoji_id {
            Some(i) => serde_json::json!({"name": e, "id": i.to_string(), "animated": true}),
            None => serde_json::json!({"name": e}),
        });
        let a = serde_json::json!({"type": 4, "name": "Custom Status", "state": text, "emoji": emoji});
        return Some(AppEvent::Activities { user: id(r.user.id), activities: crate::presence::parse_activities(&[a]) });
    }
    let sessions = r.sessions.as_ref()?.as_array()?;
    let s = sessions.iter().find(|s| s.get("session_id").and_then(Value::as_str) == Some("all")).or_else(|| sessions.first())?;
    let acts = s.get("activities").and_then(Value::as_array)?;
    Some(AppEvent::Activities { user: id(r.user.id), activities: crate::presence::parse_activities(acts) })
}

pub(super) fn premium_unix(m: &api::Member) -> Option<i64> {
    m.premium_since.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.timestamp())
}

pub(super) fn joined_unix(m: &api::Member) -> Option<i64> {
    m.joined_at.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.timestamp())
}

pub(crate) fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut sp = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            sp = true;
        } else {
            if sp && !out.is_empty() {
                out.push(' ');
            }
            sp = false;
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod collapse_tests {
    #[test]
    fn collapses_name_spaces() {
        assert_eq!(super::collapse_ws("!      Slyto  "), "! Slyto");
        assert_eq!(super::collapse_ws("a\u{3164}b"), "a\u{3164}b");
    }
}
