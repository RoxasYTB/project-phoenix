use std::collections::HashMap;

use base64::Engine;
use phoenix_api::{
    Cache, RestClient,
    gateway::{GatewayEvent, Ready},
    models::{self as api, ChannelType, RelationshipType},
};
use reqwest::Method;
use serde_json::{Value, json};

use super::convert::{self, id, sf};
use crate::{
    backend::{AppEvent, BackendError},
    model::{self, Id, ImageRef},
    ui::home::data::{
        ChannelExtra, GuildFolder, InviteKind, InvitePreview, NavEvent, NavReply, NavRequest, RelAction, Relationship,
        RelationshipKind,
    },
};

fn rel_kind(k: RelationshipType) -> Option<RelationshipKind> {
    Some(match k {
        RelationshipType::Friend => RelationshipKind::Friend,
        RelationshipType::Blocked => RelationshipKind::Blocked,
        RelationshipType::IncomingRequest => RelationshipKind::Incoming,
        RelationshipType::OutgoingRequest => RelationshipKind::Outgoing,
        _ => return None,
    })
}

fn relationship(r: &api::Relationship) -> Option<Relationship> {
    let kind = match rel_kind(r.kind) {
        Some(k) => k,
        None if r.user_ignored => RelationshipKind::None,
        None => return None,
    };
    Some(Relationship {
        user: id(r.user.as_ref().map(|u| u.id).or(r.user_id).unwrap_or(r.id)),
        kind,
        nickname: r.nickname.clone().filter(|n| !n.is_empty()).map(Into::into),
        since: r.since.as_deref().and_then(crate::presence::iso_to_unix),
        ignored: r.user_ignored,
        spam: r.is_spam_request,
        note: r.note.clone().filter(|n| !n.is_empty()).map(Into::into),
    })
}

fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i)?;
        *i += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

pub(crate) fn fields(b: &[u8]) -> Vec<(u64, u8, &[u8], u64)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let Some(key) = varint(b, &mut i) else { break };
        let (num, wt) = (key >> 3, (key & 7) as u8);
        match wt {
            0 => {
                let Some(v) = varint(b, &mut i) else { break };
                out.push((num, wt, &b[0..0], v));
            }
            1 => {
                let Some(s) = b.get(i..i + 8) else { break };
                i += 8;
                out.push((num, wt, s, u64::from_le_bytes(s.try_into().unwrap_or_default())));
            }
            2 => {
                let Some(len) = varint(b, &mut i) else { break };
                let Some(s) = b.get(i..i + len as usize) else { break };
                i += len as usize;
                out.push((num, wt, s, 0));
            }
            5 => {
                let Some(s) = b.get(i..i + 4) else { break };
                i += 4;
                out.push((num, wt, s, u64::from(u32::from_le_bytes(s.try_into().unwrap_or_default()))));
            }
            _ => break,
        }
    }
    out
}

const SOUNDBOARD_NEW_SOUNDS_BADGE: u64 = 870;
const NEW_SOUNDS_SINCE_MS: u64 = 1_793_491_200_000;

pub fn sounds_dismissed(proto: &[u8]) -> HashMap<u64, u64> {
    let mut out = HashMap::new();
    for (n, w, guilds, _) in fields(proto) {
        if n != 3 || w != 2 {
            continue;
        }
        for (n, w, entry, _) in fields(guilds) {
            if n != 1 || w != 2 {
                continue;
            }
            let ef = fields(entry);
            let Some(g) = ef.iter().find(|f| f.0 == 1).map(|f| f.3) else { continue };
            let Some(gs) = ef.iter().find(|f| f.0 == 2 && f.1 == 2).map(|f| f.2) else { continue };
            for (n, w, st, _) in fields(gs) {
                if n != 12 || w != 2 {
                    continue;
                }
                let sf = fields(st);
                if sf.iter().find(|f| f.0 == 1).map(|f| f.3) != Some(SOUNDBOARD_NEW_SOUNDS_BADGE) {
                    continue;
                }
                let Some(v) = sf.iter().find(|f| f.0 == 2 && f.1 == 2).map(|f| f.2) else { continue };
                if let Some(o) = fields(v).iter().find(|f| f.0 == 4 && f.1 == 0).map(|f| f.3).filter(|o| *o != 0) {
                    out.insert(g, o);
                }
            }
        }
    }
    out
}

pub fn quest_onboarding_dismissed(proto: &[u8]) -> bool {
    let Some(uc) = fields(proto).into_iter().find(|f| f.0 == 4 && f.1 == 2).map(|f| f.2) else { return false };
    let contents = fields(uc).into_iter().find(|f| f.0 == 1 && f.1 == 2).map(|f| f.2).unwrap_or_default();
    crate::ui::quests_page::dismissed(contents, crate::ui::quests_page::QUEST_HOME_ENTRYPOINT_ONBOARDING)
}

pub fn sounds_seen(cache: &Cache, proto: &[u8]) -> Vec<(Id, u64)> {
    let dismissed = sounds_dismissed(proto);
    let since = crate::ui::inbox_unreads::snowflake_from_ms(NEW_SOUNDS_SINCE_MS);
    cache
        .guilds()
        .iter()
        .filter_map(|g| {
            let at = g.joined_at.as_deref()?;
            let ms = chrono::DateTime::parse_from_rfc3339(at).ok()?.timestamp_millis().max(0) as u64;
            let joined = crate::ui::inbox_unreads::snowflake_from_ms(ms);
            Some((id(g.id), since.max(joined).max(dismissed.get(&g.id.0).copied().unwrap_or(0))))
        })
        .collect()
}

fn wrapped_u64(b: &[u8]) -> Option<u64> {
    fields(b).into_iter().find(|f| f.0 == 1).map(|f| f.3)
}

fn wrapped_str(b: &[u8]) -> Option<String> {
    fields(b).into_iter().find(|f| f.0 == 1 && f.1 == 2).map(|f| String::from_utf8_lossy(f.2).into_owned())
}

pub fn folders_from_proto(b64: &str) -> Vec<GuildFolder> {
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) else { return Vec::new() };
    let mut out = Vec::new();
    for (num, wt, payload, _) in fields(&bytes) {
        if num != 14 || wt != 2 {
            continue;
        }
        for (n, w, p, _) in fields(payload) {
            if n != 1 || w != 2 {
                continue;
            }
            let mut f = GuildFolder::default();
            for (fnum, fw, fp, fs) in fields(p) {
                match fnum {
                    1 if fw == 2 => {
                        f.guilds.extend(fp.as_chunks::<8>().0.iter().map(|c| Id(u64::from_le_bytes(*c))))
                    }
                    1 if fw == 1 => f.guilds.push(Id(fs)),
                    2 => f.id = wrapped_u64(fp).filter(|v| *v != 0),
                    3 => f.name = wrapped_str(fp).filter(|s| !s.is_empty()).map(Into::into),
                    4 => f.color = wrapped_u64(fp).map(|c| c as u32),
                    _ => {}
                }
            }
            if !f.guilds.is_empty() {
                out.push(f);
            }
        }
    }
    out
}

fn snowflake(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

pub fn folders_from_json(v: Option<&Value>) -> Vec<GuildFolder> {
    let Some(Value::Array(fs)) = v.and_then(|v| v.get("guild_folders")) else { return Vec::new() };
    fs.iter()
        .map(|f| GuildFolder {
            id: f.get("id").and_then(snowflake).filter(|v| *v != 0),
            name: f.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| s.to_owned().into()),
            color: f.get("color").and_then(Value::as_u64).map(|c| c as u32),
            guilds: f.get("guild_ids").and_then(Value::as_array).into_iter().flatten().filter_map(snowflake).map(Id).collect(),
        })
        .filter(|f| !f.guilds.is_empty())
        .collect()
}

pub(super) fn thread_member_mute(c: &api::Channel) -> (bool, Option<i64>) {
    let Some(m) = c.member.as_ref() else { return (false, None) };
    let end = m.get("mute_config").and_then(|c| c.get("end_time")).and_then(Value::as_str);
    let end = end.and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).map(|t| t.timestamp_millis());
    if !m.get("muted").and_then(Value::as_bool).unwrap_or(false) {
        return (false, end);
    }
    (end.is_none_or(|t| t > chrono::Utc::now().timestamp_millis()), end)
}

fn thread_member_muted(c: &api::Channel) -> bool {
    thread_member_mute(c).0
}

fn thread(c: &api::Channel) -> Option<model::Channel> {
    if !c.kind.is_thread() || c.thread_metadata.as_ref().is_some_and(|m| m.archived) {
        return None;
    }
    Some(model::Channel {
        id: id(c.id),
        guild: c.guild_id.map(id),
        parent: c.parent_id.map(id),
        name: c.display_name().into(),
        topic: None,
        kind: model::ChannelKind::Text,
        position: 0,
        unread: false,
        muted: thread_member_muted(c),
        mentions: 0,
        voice_users: Vec::new(),
    })
}

pub(crate) fn is_everyone_denied(
    kind: ChannelType,
    everyone_base: api::Permissions,
    everyone_ow: Option<&api::PermissionOverwrite>,
) -> bool {
    if kind.is_voice() {
        let mut p = everyone_base;
        if let Some(o) = everyone_ow {
            p = (p - o.deny) | o.allow;
        }
        if !p.contains(api::Permissions::CONNECT) {
            return true;
        }
    }
    everyone_ow.is_some_and(|o| o.deny.contains(api::Permissions::VIEW_CHANNEL))
}

fn extra(cache: &Cache, c: &api::Channel) -> ChannelExtra {
    let everyone_denied = |ch: &api::Channel| {
        ch.guild_id.is_some_and(|g| {
            let ow = ch.permission_overwrites.iter().find(|o| o.id == g);
            let base = cache
                .guild(g)
                .and_then(|gd| gd.roles.iter().find(|r| r.id == g).map(|r| r.permissions))
                .unwrap_or_default();
            is_everyone_denied(ch.kind, base, ow)
        })
    };
    let group = c.kind == ChannelType::GroupDm;
    let perms = cache.permissions_in(c.id);
    let locked_thread = c.kind.is_thread() && c.thread_metadata.as_ref().is_some_and(|m| m.locked);
    let send_bit = if c.kind.is_thread() { api::Permissions::SEND_MESSAGES_IN_THREADS } else { api::Permissions::SEND_MESSAGES };
    ChannelExtra {
        slowmode: c.rate_limit_per_user.unwrap_or(0),
        bypass_slowmode: perms.contains(api::Permissions::BYPASS_SLOWMODE),
        send_blocked: c.guild_id.is_some()
            && !perms.is_empty()
            && (!perms.contains(send_bit) || (locked_thread && !perms.contains(api::Permissions::MANAGE_THREADS))),
        thread_archived: c.thread_metadata.as_ref().is_some_and(|m| m.archived),
        thread_locked: locked_thread,
        can_manage_threads: perms.contains(api::Permissions::MANAGE_THREADS),
        connect_denied: c.kind.is_voice() && !perms.is_empty() && !perms.contains(api::Permissions::CONNECT),
        media: c.kind == api::ChannelType::GuildMedia,
        nsfw: c.nsfw,
        locked: everyone_denied(c),
        group_icon: c
            .icon
            .as_ref()
            .filter(|_| group)
            .map(|h| ImageRef::Url(format!("{}/channel-icons/{}/{h}.png?size=64", api::CDN_BASE, c.id).into())),
        recipients: if c.kind.is_private() { c.recipient_user_ids().len() } else { 0 },
        group_name: group.then(|| group_name(cache, c)).flatten(),
        group_named: group && c.name.as_deref().is_some_and(|n| !n.is_empty()),
        members: if group { c.recipient_user_ids().into_iter().map(id).collect() } else { Vec::new() },
        owner: c.owner_id.filter(|_| group || c.kind.is_thread()).map(id),
    }
}

fn recent_of(m: &api::Message) -> crate::thread_recent::Recent {
    let name = m.member.as_ref().and_then(|mb| mb.nick.as_deref()).filter(|n| !n.is_empty()).unwrap_or(m.author.display_name());
    crate::thread_recent::Recent {
        author: id(m.author.id),
        name: convert::collapse_ws(name).into(),
        avatar: convert::user(&m.author).avatar,
        content: m.content.clone().into(),
        ts: (m.id.timestamp_ms() / 1000) as i64,
    }
}

fn guild_extras(cache: &Cache, g: api::Snowflake) -> Vec<(Id, ChannelExtra)> {
    cache.guild_channels(g).iter().map(|c| (id(c.id), extra(cache, c))).collect()
}

fn group_name(cache: &Cache, c: &api::Channel) -> Option<gpui::SharedString> {
    if c.name.as_deref().is_some_and(|n| !n.is_empty()) {
        return None;
    }
    let names: Vec<String> = c
        .recipient_user_ids()
        .into_iter()
        .filter_map(|u| {
            let nick = cache.relationship(u).and_then(|r| r.nickname.clone()).filter(|n| !n.is_empty());
            nick.or_else(|| c.recipients.iter().find(|r| r.id == u).map(|r| r.display_name().to_owned()))
                .or_else(|| cache.user(u).map(|r| r.display_name().to_owned()))
        })
        .collect();
    (!names.is_empty()).then(|| names.join(", ").into())
}

pub fn snapshot(cache: &Cache, ready: &Ready) -> Vec<AppEvent> {
    let relationships = ready.relationships.iter().filter_map(relationship).collect();
    let mut folders = ready.user_settings_proto.as_deref().map(folders_from_proto).unwrap_or_default();
    if folders.is_empty() {
        folders = folders_from_json(ready.user_settings.as_ref());
    }
    let mut threads = Vec::new();
    let mut extras = Vec::new();
    let mutes = convert::parse_mutes(ready.user_guild_settings.as_ref());
    for g in cache.guilds() {
        for c in cache.guild_channels(g.id) {
            if let Some(mut t) = thread(&c) {
                t.muted |= mutes.channels.contains(&c.id.0);
                threads.push(t);
            }
        }
        extras.extend(guild_extras(cache, g.id));
    }
    let banners = cache.guilds().iter().map(|g| (id(g.id), g.properties.banner.clone())).collect();
    extras.extend(cache.private_channels().iter().map(|c| (id(c.id), extra(cache, c))));
    let message_requests = cache.private_channels().iter().filter(|c| c.is_message_request && !c.is_spam).map(|c| id(c.id)).collect();
    let spam_requests = cache.private_channels().iter().filter(|c| c.is_spam).map(|c| id(c.id)).collect();
    let proto = ready.user_settings_proto.as_deref().and_then(|b| base64::engine::general_purpose::STANDARD.decode(b.trim()).ok()).unwrap_or_default();
    let games = api::GameRelationship::parse_all(ready.game_relationships.as_ref())
        .into_iter()
        .filter(|g| g.kind == RelationshipType::IncomingRequest)
        .map(|g| (id(g.id), g.application_id.map_or(Id(0), id)))
        .collect();
    vec![
        AppEvent::Nav(NavEvent::Snapshot { relationships, folders, threads, extras, message_requests, spam_requests }),
        AppEvent::Nav(NavEvent::GameRequests(games)),
        AppEvent::Nav(NavEvent::Banners(banners)),
        AppEvent::Nav(NavEvent::SoundsSeen(sounds_seen(cache, &proto))),
        AppEvent::Nav(NavEvent::QuestOnboardingDismissed(quest_onboarding_dismissed(&proto))),
    ]
}

pub fn on_gateway(cache: &Cache, ev: &GatewayEvent) -> Vec<AppEvent> {
    let mut out = Vec::new();
    match ev {
        GatewayEvent::RelationshipAdd(r) => {
            if let Some(u) = &r.user {
                out.push(AppEvent::Users(vec![convert::user(u)]));
            }
            if let Some(r) = relationship(r) {
                out.push(AppEvent::Nav(NavEvent::RelationshipUpsert(r)));
            }
        }
        GatewayEvent::RelationshipRemove(r) => {
            out.push(AppEvent::Nav(NavEvent::RelationshipRemove(id(r.user_id.unwrap_or(r.id)))));
        }
        GatewayEvent::Unknown { name, data } if name == "RELATIONSHIP_UPDATE" => {
            if let Some(r) = data.get("id").and_then(Value::as_str).and_then(|x| x.parse().ok()).and_then(|u| cache.relationship(api::Snowflake(u)))
                && let Some(r) = relationship(&r)
            {
                out.push(AppEvent::Nav(NavEvent::RelationshipUpsert(r)));
            }
        }
        GatewayEvent::Unknown { name, data } if name == "GAME_RELATIONSHIP_ADD" || name == "GAME_RELATIONSHIP_REMOVE" => {
            if let (Some(user), app) = (data.get("id").and_then(snowflake), data.get("application_id").and_then(snowflake)) {
                let incoming = name == "GAME_RELATIONSHIP_ADD" && data.get("type").and_then(Value::as_u64) == Some(u64::from(RelationshipType::IncomingRequest.value()));
                out.push(AppEvent::Nav(NavEvent::GameRequest { user: Id(user), app: Id(app.unwrap_or(0)), incoming }));
            }
        }
        GatewayEvent::ThreadCreate(c) | GatewayEvent::ThreadUpdate(c) => {
            out.push(AppEvent::Nav(NavEvent::Extras(vec![(id(c.id), extra(cache, c))])));
            out.push(AppEvent::Nav(match thread(c) {
                Some(t) => NavEvent::ThreadUpsert(t),
                None => NavEvent::ThreadDelete(id(c.id)),
            }));
        }
        GatewayEvent::ThreadDelete(c) => out.push(AppEvent::Nav(NavEvent::ThreadDelete(id(c.id)))),
        GatewayEvent::Unknown { name, data } if name == "THREAD_MEMBER_UPDATE" || name == "THREAD_MEMBERS_UPDATE" => {
            if let Some(c) = data.get("id").and_then(snowflake).and_then(|t| cache.channel(api::Snowflake(t)))
                && let Some(t) = thread(&c)
            {
                out.push(AppEvent::Nav(NavEvent::ThreadUpsert(t)));
            }
        }
        GatewayEvent::Unknown { name, data } if name == "USER_SETTINGS_PROTO_UPDATE" => {
            let s = data.get("settings");
            if s.and_then(|s| s.get("type")).and_then(Value::as_u64) == Some(1)
                && let Some(p) = s.and_then(|s| s.get("proto")).and_then(Value::as_str)
            {
                out.push(AppEvent::SettingsChanged);
                let f = folders_from_proto(p);
                if !f.is_empty() {
                    out.push(AppEvent::Nav(NavEvent::Folders(f)));
                }
                let bytes = base64::engine::general_purpose::STANDARD.decode(p.trim()).ok();
                let seen = bytes.as_deref().map(|b| sounds_seen(cache, b)).unwrap_or_default();
                if !seen.is_empty() {
                    out.push(AppEvent::Nav(NavEvent::SoundsSeen(seen)));
                }
                if let Some(b) = bytes.as_deref().filter(|b| fields(b).iter().any(|f| f.0 == 4)) {
                    out.push(AppEvent::Nav(NavEvent::QuestOnboardingDismissed(quest_onboarding_dismissed(b))));
                }
            }
        }
        GatewayEvent::MessageCreate(m) if cache.channel(m.channel_id).is_some_and(|c| c.kind.is_thread()) => {
            crate::thread_recent::set(id(m.channel_id), recent_of(m));
        }
        GatewayEvent::Unknown { name, data } if name == "THREAD_LIST_SYNC" => {
            for m in data.get("most_recent_messages").and_then(Value::as_array).into_iter().flatten() {
                if let Ok(m) = serde_json::from_value::<api::Message>(m.clone()) {
                    crate::thread_recent::set(id(m.channel_id), recent_of(&m));
                }
            }
            let joined: Vec<u64> =
                data.get("members").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.get("id").and_then(snowflake)).collect();
            for t in data.get("threads").and_then(Value::as_array).into_iter().flatten() {
                if let Ok(c) = serde_json::from_value::<api::Channel>(t.clone())
                    && joined.contains(&c.id.0)
                    && let Some(t) = thread(&c)
                {
                    out.push(AppEvent::Nav(NavEvent::ThreadUpsert(t)));
                }
            }
        }
        GatewayEvent::ChannelCreate(c) | GatewayEvent::ChannelUpdate(c) => {
            out.push(AppEvent::Nav(NavEvent::Extras(vec![(id(c.id), extra(cache, c))])));
        }
        GatewayEvent::GuildUpdate(g) => {
            out.push(AppEvent::Nav(NavEvent::Banners(vec![(id(g.id), g.properties.banner.clone())])));
        }
        GatewayEvent::GuildCreate(g) => {
            out.push(AppEvent::Nav(NavEvent::Banners(vec![(id(g.id), g.properties.banner.clone())])));
            out.push(AppEvent::Nav(NavEvent::Extras(guild_extras(cache, g.id))));
            for c in cache.guild_channels(g.id) {
                if let Some(t) = thread(&c) {
                    out.push(AppEvent::Nav(NavEvent::ThreadUpsert(t)));
                }
            }
        }
        GatewayEvent::PresenceUpdate(p) if p.guild_id.is_none() => {
            out.push(AppEvent::Nav(NavEvent::Activity { user: id(p.user.id), activity: convert::activity(&p.activities) }));
        }
        GatewayEvent::ReadySupplemental(r) => {
            out.push(AppEvent::PresenceReset);
            let friends = r.merged_presences.as_ref().and_then(|m| m.get("friends")).and_then(Value::as_array);
            for f in friends.into_iter().flatten() {
                let Some(u) = f.get("user_id").and_then(snowflake) else { continue };
                let acts = f.get("activities").and_then(Value::as_array).cloned().unwrap_or_default();
                if let Some(a) = convert::activity(&acts) {
                    out.push(AppEvent::Nav(NavEvent::Activity { user: Id(u), activity: Some(a) }));
                }
                if let Some(s) = f.get("status").and_then(Value::as_str) {
                    out.push(AppEvent::PresenceUpdate { user: Id(u), presence: convert::presence(s) });
                }
            }
        }
        _ => {}
    }
    out
}

fn api_err(e: phoenix_api::Error) -> BackendError {
    let s = e.to_string();
    BackendError(if s.contains("captcha") || s.contains("Captcha") {
        "Discord asked for a captcha. Join from the official client once.".into()
    } else {
        s
    })
}

fn template_refused(e: &phoenix_api::Error) -> bool {
    matches!(e, phoenix_api::Error::Api { status: 403, body, .. } if body.code == 10008)
}

fn invite_preview(v: &Value) -> InvitePreview {
    let s = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
    let g = v.get("guild");
    let gid = g.and_then(|g| g.get("id")).and_then(snowflake);
    let icon = match (gid, s(g.and_then(|g| g.get("icon")))) {
        (Some(gid), Some(h)) => Some(ImageRef::Url(format!("{}/icons/{gid}/{h}.png?size=128", api::CDN_BASE).into())),
        _ => None,
    };
    let splash = match (gid, s(g.and_then(|g| g.get("splash")))) {
        (Some(gid), Some(h)) => Some(format!("{}/splashes/{gid}/{h}.jpg?size=512", api::CDN_BASE).into()),
        _ => None,
    };
    let u = v.get("inviter");
    let inviter = u.map(|u| s(u.get("global_name")).or_else(|| s(u.get("username"))).unwrap_or_default());
    let inviter_id = u.and_then(|u| u.get("id")).and_then(snowflake);
    let inviter_avatar = match (inviter_id, s(u.and_then(|u| u.get("avatar")))) {
        (Some(uid), Some(h)) => Some(ImageRef::Url(format!("{}/avatars/{uid}/{h}.png?size=128", api::CDN_BASE).into())),
        _ => None,
    };
    let ch = v.get("channel");
    let channel_id = ch.and_then(|c| c.get("id")).and_then(snowflake);
    let channel_type = ch.and_then(|c| c.get("type")).and_then(Value::as_u64).map(|t| t as u8);
    let channel_icon = match (channel_id, s(ch.and_then(|c| c.get("icon")))) {
        (Some(cid), Some(h)) => Some(ImageRef::Url(format!("{}/channel-icons/{cid}/{h}.png?size=128", api::CDN_BASE).into())),
        _ => None,
    };
    let kind = match v.get("type").and_then(Value::as_u64) {
        Some(1) => InviteKind::GroupDm,
        Some(2) => InviteKind::Friend,
        Some(_) => InviteKind::Guild,
        None if channel_type == Some(3) => InviteKind::GroupDm,
        None if g.is_none() && u.is_some() => InviteKind::Friend,
        None => InviteKind::Guild,
    };
    let recipients = ch
        .and_then(|c| c.get("recipients"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|r| s(r.get("global_name")).or_else(|| s(r.get("username")))).map(Into::into).collect())
        .unwrap_or_default();
    let recipient_users = ch
        .and_then(|c| c.get("recipients"))
        .and_then(Value::as_array)
        .map(|a| a.iter().map(invite_user).collect())
        .unwrap_or_default();
    let ev = v.get("guild_scheduled_event").filter(|e| e.is_object());
    InvitePreview {
        code: s(v.get("code")).unwrap_or_default().into(),
        guild: gid.map(Id),
        guild_name: s(g.and_then(|g| g.get("name")))
            .or_else(|| s(v.get("channel").and_then(|c| c.get("name"))))
            .unwrap_or_else(|| "Group DM".into())
            .into(),
        guild_icon: icon,
        channel_name: s(v.get("channel").and_then(|c| c.get("name"))).map(Into::into),
        inviter: inviter.filter(|i| !i.is_empty()).map(Into::into),
        online: v.get("approximate_presence_count").and_then(Value::as_u64).unwrap_or(0) as u32,
        members: v.get("approximate_member_count").and_then(Value::as_u64).unwrap_or(0) as u32,
        kind,
        channel_id: channel_id.map(Id),
        channel_type,
        features: g
            .and_then(|g| g.get("features"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
        profile: v.get("profile").filter(|p| p.is_object()).map(crate::commands::guide::parse_profile),
        splash,
        inviter_id: inviter_id.map(Id),
        inviter_username: s(u.and_then(|u| u.get("username"))).map(Into::into),
        inviter_avatar,
        channel_icon,
        recipients,
        event_name: s(v.get("guild_scheduled_event").and_then(|e| e.get("name"))).map(Into::into),
        recipient_users,
        event: ev.and_then(crate::commands::parse_event),
        event_creator: ev.and_then(|e| e.get("creator")).filter(|c| c.is_object()).map(invite_user),
        target_type: v.get("target_type").and_then(Value::as_u64).map(|t| t as u8),
        target_user: v.get("target_user").filter(|u| u.is_object()).map(invite_user),
    }
}

fn invite_user(u: &Value) -> crate::ui::home::data::InviteUser {
    let s = |k: &str| u.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_owned);
    let uid = u.get("id").and_then(snowflake);
    crate::ui::home::data::InviteUser {
        id: uid.map(Id),
        name: s("global_name").or_else(|| s("username")).unwrap_or_default().into(),
        avatar: match (uid, s("avatar")) {
            (Some(uid), Some(h)) => Some(ImageRef::Url(format!("{}/avatars/{uid}/{h}.png?size=128", api::CDN_BASE).into())),
            _ => None,
        },
    }
}

fn top_emoji_ids(v: &Value) -> Vec<Id> {
    let mut items: Vec<(u64, u64)> = v
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|it| Some((it.get("emoji_rank").and_then(Value::as_u64).unwrap_or(u64::MAX), it.get("emoji_id").and_then(snowflake)?)))
        .collect();
    items.sort_by_key(|(rank, _)| *rank);
    items.into_iter().map(|(_, id)| Id(id)).collect()
}

fn enc(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

pub async fn request(rest: RestClient, req: NavRequest) -> Result<NavReply, BackendError> {
    let call = |m: Method, p: String, b: Option<Value>| {
        let rest = rest.clone();
        async move { rest.request_json(m, &p, b).await.map_err(api_err) }
    };
    Ok(match req {
        NavRequest::FetchInvite(code) => {
            let v = call(Method::GET, format!("/invites/{}?with_counts=true&with_expiration=true", enc(&code)), None).await?;
            let mut p = invite_preview(&v);
            if let Some(prof) = p.profile.as_mut().filter(|p| !p.games.is_empty()) {
                let mut fetched = Vec::new();
                for chunk in prof.games.chunks(20) {
                    let q: Vec<String> = chunk.iter().map(|g| format!("game_ids={}", g.id)).collect();
                    if let Ok(Value::Array(a)) = call(Method::GET, format!("/games?{}", q.join("&")), None).await {
                        fetched.extend(a);
                    }
                }
                crate::commands::guide::apply_games(&mut prof.games, &Value::Array(fetched));
            }
            NavReply::Invite(Box::new(p))
        }
        NavRequest::FetchInviteEvent(code, event) => {
            let v = call(
                Method::GET,
                format!("/invites/{}?with_counts=true&with_expiration=true&guild_scheduled_event_id={}", enc(&code), sf(event)),
                None,
            )
            .await?;
            NavReply::Invite(Box::new(invite_preview(&v)))
        }
        NavRequest::TopEmojis(g) => {
            let v = call(Method::GET, format!("/guilds/{}/top-emojis", sf(g)), None).await?;
            NavReply::Ids(top_emoji_ids(&v))
        }
        NavRequest::AcceptInvite(code) => {
            call(Method::POST, format!("/invites/{}", enc(&code)), Some(json!({}))).await?;
            NavReply::Done
        }
        NavRequest::CreateGuild { name, icon, template, community } => {
            let body = crate::ui::add_server::create_guild_body(&name, icon.as_deref(), template.as_deref(), community);
            match rest.request_json(Method::POST, "/guilds", Some(body)).await {
                Ok(guild) => NavReply::Json(guild),
                Err(e) if template_refused(&e) => {
                    let plain = crate::ui::add_server::plain_create_guild_body(&name, icon.as_deref());
                    NavReply::Json(call(Method::POST, "/guilds".into(), Some(plain)).await?)
                }
                Err(e) => return Err(api_err(e)),
            }
        }
        NavRequest::FetchOnboarding(g) => {
            let v = call(Method::GET, format!("/guilds/{}/onboarding", sf(g)), None).await?;
            NavReply::Json(v)
        }
        NavRequest::SubmitOnboarding { guild, options, prompts } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let body = crate::ui::join_flow::onboarding_response_body(&options, &prompts, now);
            call(Method::POST, format!("/guilds/{}/onboarding-responses", sf(guild)), Some(body)).await?;
            NavReply::Done
        }
        NavRequest::FetchMemberVerification(g) => {
            let v = call(Method::GET, format!("/guilds/{}/member-verification?with_guild=false", sf(g)), None).await?;
            NavReply::Json(v)
        }
        NavRequest::SubmitMemberVerification { guild, version, rules } => {
            let body = crate::ui::join_flow::member_verification_submit_body(&version, &rules);
            call(Method::PUT, format!("/guilds/{}/requests/@me", sf(guild)), Some(body)).await?;
            NavReply::Done
        }
        NavRequest::JoinGuild(g) => {
            call(Method::PUT, format!("/guilds/{}/members/@me", sf(g)), Some(json!({}))).await?;
            NavReply::Done
        }
        NavRequest::CreateInvite(ch) => {
            let v = call(
                Method::POST,
                format!("/channels/{}/invites", sf(ch)),
                Some(json!({ "max_age": 604800, "max_uses": 0, "temporary": false })),
            )
            .await?;
            let code = v.get("code").and_then(Value::as_str).unwrap_or_default();
            NavReply::Text(format!("https://discord.gg/{code}").into())
        }
        NavRequest::LeaveGuild { guild, lurking } => {
            call(Method::DELETE, format!("/users/@me/guilds/{}", sf(guild)), Some(json!({ "lurking": lurking }))).await?;
            NavReply::Done
        }
        NavRequest::FetchRelationships => {
            let records = rest.get_relationships().await.map_err(api_err)?;
            NavReply::Relationships {
                users: records.iter().filter_map(|r| r.user.as_ref().map(convert::user)).collect(),
                records: records.iter().filter_map(relationship).collect(),
            }
        }
        NavRequest::Relationship { user, action } => {
            let (m, path, body) = rel_request(user.0, &action);
            if matches!(action, RelAction::AddByTag { .. } | RelAction::Accept) {
                return match rest.request_json(m, &path, body).await {
                    Ok(_) => Ok(NavReply::Done),
                    Err(e) => Err(BackendError(add_friend_failure(&e).encode())),
                };
            }
            call(m, path, body).await?;
            NavReply::Done
        }
        NavRequest::OpenDm(user) => {
            let c = rest.create_dm(sf(user)).await.map_err(api_err)?;
            NavReply::Channel(model::Channel {
                id: id(c.id),
                guild: None,
                parent: None,
                name: c.display_name().into(),
                topic: None,
                kind: model::ChannelKind::Text,
                position: 0,
                unread: false,
                muted: false,
                mentions: 0,
                voice_users: c.recipient_user_ids().first().copied().map(id).into_iter().collect(),
            })
        }
        NavRequest::MessageRequest { channel, accept } => {
            let (m, path, body) = message_request_call(channel.0, accept);
            call(m, path, body).await?;
            NavReply::Done
        }
        NavRequest::RejectMessageRequests(ids) => {
            for (m, path, body) in reject_batches(&ids) {
                call(m, path, body).await?;
            }
            NavReply::Done
        }
        NavRequest::CloseDm(ch) => {
            call(Method::DELETE, format!("/channels/{}?silent=false", sf(ch)), None).await?;
            NavReply::Done
        }
        NavRequest::AckChannel(ch) if !super::ack_allowed(ch.0) => NavReply::Done,
        NavRequest::AckGuild(_) if super::acks_disabled() => NavReply::Done,
        NavRequest::AckChannel(ch) => {
            let v = call(Method::GET, format!("/channels/{}", sf(ch)), None).await?;
            if let Some(last) = v.get("last_message_id").and_then(snowflake) {
                rest.ack_message(sf(ch), sf(Id(last))).await.map_err(api_err)?;
            }
            NavReply::Done
        }
        NavRequest::AckGuild(g) => {
            call(Method::POST, format!("/guilds/{}/ack", sf(g)), Some(json!({}))).await?;
            NavReply::Done
        }
        NavRequest::Mute { guild, channel, muted, secs } => {
            let end = secs.map(|s| (chrono::Utc::now() + chrono::Duration::seconds(s as i64)).to_rfc3339());
            let cfg = json!({ "muted": muted, "mute_config": muted.then(|| json!({
                "selected_time_window": secs.map(|s| s as i64).unwrap_or(-1),
                "end_time": end,
            })) });
            call(Method::PATCH, settings_path(guild), Some(settings_body(channel, cfg))).await?;
            NavReply::Done
        }
        NavRequest::GuildNotify { guild, fields } => {
            call(Method::PATCH, settings_path(Some(guild)), Some(fields)).await?;
            NavReply::Done
        }
        NavRequest::NotifyLevel { guild, channel, level } => {
            let body = settings_body(channel, json!({ "message_notifications": level }));
            call(Method::PATCH, settings_path(guild), Some(body)).await?;
            NavReply::Done
        }
    })
}

pub(crate) fn rel_request(user: u64, action: &RelAction) -> (Method, String, Option<Value>) {
    let one = format!("/users/@me/relationships/{user}");
    let all = "/users/@me/relationships".to_owned();
    match action {
        RelAction::Accept => (Method::PUT, one, Some(json!({}))),
        RelAction::AcceptStranger => (Method::PUT, one, Some(json!({ "confirm_stranger_request": true }))),
        RelAction::Block => (Method::PUT, one, Some(json!({ "type": 2 }))),
        RelAction::Remove => (Method::DELETE, one, None),
        RelAction::Ignore => (Method::PUT, format!("/users/@me/relationships/{user}/ignore"), None),
        RelAction::ClearPending => (Method::DELETE, format!("{all}?relationship_type=3"), None),
        RelAction::ClearSpamAndIgnored => (Method::DELETE, format!("{all}?relationship_type=3"), Some(json!({ "filters": [1, 2] }))),
        RelAction::AddByTag { tag, note } => (Method::POST, all, Some(crate::ui::home::add_friend::request_body(tag, note.as_deref()))),
    }
}

pub(crate) fn message_request_call(channel: u64, accept: bool) -> (Method, String, Option<Value>) {
    let path = format!("/channels/{channel}/recipients/@me");
    if accept { (Method::PUT, path, Some(json!({ "consent_status": 2 }))) } else { (Method::DELETE, path, None) }
}

pub(crate) fn reject_batches(ids: &[Id]) -> Vec<(Method, String, Option<Value>)> {
    ids.chunks(50)
        .map(|c| {
            let ids: Vec<String> = c.iter().map(|i| i.0.to_string()).collect();
            (Method::PUT, "/channels/recipients/@me/batch-reject".to_owned(), Some(json!({ "channel_ids": ids })))
        })
        .collect()
}

fn add_friend_failure(e: &phoenix_api::Error) -> crate::ui::home::add_friend::Failure {
    use crate::ui::home::add_friend::Failure;
    match e {
        phoenix_api::Error::Api { status, body, raw } => Failure {
            status: *status,
            code: (body.code != 0).then_some(body.code),
            note_rejected: serde_json::from_str::<Value>(raw).ok().is_some_and(|v| v.get("note").is_some_and(|n| !n.is_null())),
        },
        phoenix_api::Error::RateLimited { .. } => Failure { status: 429, code: None, note_rejected: false },
        _ => Failure { status: 0, code: None, note_rejected: false },
    }
}

fn settings_path(guild: Option<Id>) -> String {
    match guild {
        Some(g) => format!("/users/@me/guilds/{}/settings", sf(g)),
        None => "/users/@me/guilds/@me/settings".into(),
    }
}

fn settings_body(channel: Option<Id>, fields: Value) -> Value {
    match channel {
        Some(ch) => json!({ "channel_overrides": { sf(ch).to_string(): fields } }),
        None => fields,
    }
}

#[cfg(test)]
mod rel_tests {
    use super::*;

    #[test]
    fn relationship_requests_match_the_client() {
        let (m, p, b) = rel_request(5, &RelAction::Accept);
        assert_eq!((m, p.as_str(), b), (Method::PUT, "/users/@me/relationships/5", Some(json!({}))));
        let (_, _, b) = rel_request(5, &RelAction::AcceptStranger);
        assert_eq!(b, Some(json!({ "confirm_stranger_request": true })));
        let (m, p, b) = rel_request(5, &RelAction::Ignore);
        assert_eq!((m, p.as_str(), b), (Method::PUT, "/users/@me/relationships/5/ignore", None));
        let (m, p, b) = rel_request(5, &RelAction::Remove);
        assert_eq!((m, p.as_str(), b), (Method::DELETE, "/users/@me/relationships/5", None));
        let (m, p, b) = rel_request(0, &RelAction::ClearPending);
        assert_eq!((m, p.as_str(), b), (Method::DELETE, "/users/@me/relationships?relationship_type=3", None));
        let (_, _, b) = rel_request(0, &RelAction::ClearSpamAndIgnored);
        assert_eq!(b, Some(json!({ "filters": [1, 2] })));
        let (m, p, b) = rel_request(0, &RelAction::AddByTag { tag: "wumpus".into(), note: Some("hey".into()) });
        assert_eq!((m, p.as_str()), (Method::POST, "/users/@me/relationships"));
        assert_eq!(b, Some(json!({ "username": "wumpus", "discriminator": null, "note": "hey" })));
    }

    #[test]
    fn message_request_calls_match_the_client() {
        let (m, p, b) = message_request_call(7, true);
        assert_eq!((m, p.as_str(), b), (Method::PUT, "/channels/7/recipients/@me", Some(json!({ "consent_status": 2 }))));
        let (m, p, b) = message_request_call(7, false);
        assert_eq!((m, p.as_str(), b), (Method::DELETE, "/channels/7/recipients/@me", None));
        let ids: Vec<Id> = (1..=120).map(Id).collect();
        let batches = reject_batches(&ids);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].1, "/channels/recipients/@me/batch-reject");
        assert_eq!(batches[2].2.as_ref().unwrap()["channel_ids"].as_array().unwrap().len(), 20);
        assert_eq!(batches[0].2.as_ref().unwrap()["channel_ids"][0], json!("1"));
    }

    #[test]
    fn add_friend_errors_keep_status_and_code() {
        let e = phoenix_api::Error::Api {
            status: 400,
            body: Box::new(phoenix_api::error::ApiErrorBody { code: 80007, ..Default::default() }),
            raw: r#"{"code":80007,"message":"x"}"#.into(),
        };
        let f = add_friend_failure(&e);
        assert_eq!((f.status, f.code, f.note_rejected), (400, Some(80007), false));
        let e = phoenix_api::Error::Api { status: 400, body: Box::default(), raw: r#"{"note":["bad"]}"#.into() };
        assert!(add_friend_failure(&e).note_rejected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_unknown_message_refusal_falls_back_to_the_plain_body() {
        let api = |status: u16, code: u32| phoenix_api::Error::Api {
            status,
            body: Box::new(phoenix_api::error::ApiErrorBody { code, ..Default::default() }),
            raw: String::new(),
        };
        assert!(template_refused(&api(403, 10008)));
        assert!(!template_refused(&api(403, 50013)));
        assert!(!template_refused(&api(400, 10008)));
        assert!(!template_refused(&phoenix_api::rest::guard::blocked(&Method::POST, "/guilds")));
    }

    #[test]
    fn unnamed_group_names_use_friend_nicknames() {
        let cache = Cache::default();
        let ready = phoenix_api::gateway::parse_dispatch(
            "READY",
            &json!({
                "v": 9,
                "user": {"id": "1", "username": "me"},
                "session_id": "s",
                "guilds": [],
                "private_channels": [
                    {"id": "50", "type": 3, "recipients": [{"id": "2", "username": "alice"}, {"id": "3", "username": "bob"}]},
                    {"id": "51", "type": 3, "name": "Team", "recipients": [{"id": "2", "username": "alice"}, {"id": "3", "username": "bob"}]}
                ],
                "relationships": [{"id": "2", "type": 1, "nickname": "Ali", "user": {"id": "2", "username": "alice"}}]
            })
            .to_string(),
        );
        cache.apply(&ready);
        let unnamed = cache.channel(api::Snowflake(50)).unwrap();
        let named = cache.channel(api::Snowflake(51)).unwrap();
        assert_eq!(group_name(&cache, &unnamed).as_deref(), Some("Ali, bob"));
        assert!(extra(&cache, &unnamed).group_name.is_some() && !extra(&cache, &unnamed).group_named);
        assert_eq!(group_name(&cache, &named), None);
        assert!(extra(&cache, &named).group_named);
    }

    #[test]
    fn thread_list_sync_lists_only_joined_threads() {
        let t = |i: u64| json!({"id": i.to_string(), "type": 11, "guild_id": "1", "parent_id": "2", "name": format!("t{i}"),
            "thread_metadata": {"archived": false, "locked": false, "auto_archive_duration": 1440, "archive_timestamp": "2026-11-24T00:00:00+00:00"}});
        let data = json!({"guild_id": "1", "threads": [t(10), t(11)], "members": [{"id": "11", "user_id": "5", "flags": 1, "join_timestamp": "2026-11-24T00:00:00+00:00"}]});
        let ev = GatewayEvent::Unknown { name: "THREAD_LIST_SYNC".into(), data };
        let out = on_gateway(&Cache::default(), &ev);
        let ids: Vec<Id> = out.iter().filter_map(|e| match e { AppEvent::Nav(NavEvent::ThreadUpsert(c)) => Some(c.id), _ => None }).collect();
        assert_eq!(ids, vec![Id(11)]);
    }

    #[test]
    fn folder_proto() {
        let mut folder = vec![0x0a, 16];
        folder.extend(1u64.to_le_bytes());
        folder.extend(2u64.to_le_bytes());
        folder.extend([0x12, 2, 0x08, 9]);
        folder.extend([0x1a, 4, 0x0a, 2, b'a', b'b']);
        folder.extend([0x22, 3, 0x08, 0xff, 0x01]);
        let mut gf = vec![0x0a, folder.len() as u8];
        gf.extend(folder);
        let mut top = vec![0x72, gf.len() as u8];
        top.extend(gf);
        let b64 = base64::engine::general_purpose::STANDARD.encode(top);
        let f = folders_from_proto(&b64);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].guilds, vec![Id(1), Id(2)]);
        assert_eq!(f[0].id, Some(9));
        assert_eq!(f[0].name.as_deref(), Some("ab"));
        assert_eq!(f[0].color, Some(255));
        let upd = |t: u64| GatewayEvent::Unknown {
            name: "USER_SETTINGS_PROTO_UPDATE".into(),
            data: json!({"settings": {"type": t, "proto": b64}, "partial": true}),
        };
        let out = on_gateway(&Cache::default(), &upd(1));
        assert!(matches!(out.as_slice(), [AppEvent::SettingsChanged, AppEvent::Nav(NavEvent::Folders(f))] if f.len() == 1));
        assert!(on_gateway(&Cache::default(), &upd(2)).is_empty());
    }

    #[test]
    fn invite_json() {
        let v = json!({"code":"x","guild":{"id":"5","name":"G","icon":"h"},"approximate_member_count":10,"approximate_presence_count":3,"inviter":{"username":"u"}});
        let p = invite_preview(&v);
        assert_eq!(p.guild, Some(Id(5)));
        assert_eq!(p.members, 10);
        assert_eq!(p.inviter.as_deref(), Some("u"));
        assert_eq!(p.kind, InviteKind::Guild);
        assert!(p.profile.is_none());
    }

    #[test]
    fn invite_kinds_and_profile() {
        let gdm = invite_preview(&json!({"code":"a","type":1,"channel":{"id":"9","type":3,"name":null,"recipients":[{"username":"x"},{"username":"y"}]},"inviter":{"id":"2","username":"x"}}));
        assert_eq!(gdm.kind, InviteKind::GroupDm);
        assert_eq!(gdm.recipients.len(), 2);
        let friend = invite_preview(&json!({"code":"b","type":2,"inviter":{"id":"3","username":"z","avatar":"h"}}));
        assert_eq!(friend.kind, InviteKind::Friend);
        assert_eq!(friend.inviter_id, Some(Id(3)));
        assert!(friend.inviter_avatar.is_some());
        let g = invite_preview(&json!({"code":"c","type":0,"guild":{"id":"7","name":"G","features":["HUB"]},"channel":{"id":"8","type":2,"name":"v"},
            "profile":{"id":"7","name":"G","member_count":12,"online_count":1,"tag":"GLAD","traits":[]},
            "guild_scheduled_event":{"id":"5","name":"Soirée"}}));
        assert_eq!(g.channel_type, Some(2));
        assert_eq!(g.features, vec!["HUB".to_owned()]);
        assert_eq!(g.profile.as_ref().map(|p| p.member_count), Some(12));
        assert_eq!(g.event_name.as_deref(), Some("Soirée"));
        assert_eq!(g.event.as_ref().map(|e| e.name.as_str()), Some("Soirée"));
        let st = invite_preview(&json!({"code":"d","type":0,"target_type":1,"guild":{"id":"7","name":"G"},"channel":{"id":"8","type":2,"name":"v"},
            "target_user":{"id":"4","username":"live","global_name":"Live","avatar":"a"}}));
        assert_eq!(st.target_type, Some(1));
        let tu = st.target_user.unwrap();
        assert_eq!((tu.id, tu.name.as_ref()), (Some(Id(4)), "Live"));
        assert!(tu.avatar.is_some());
        assert_eq!(gdm.recipient_users.len(), 2);
        assert_eq!(gdm.recipient_users[0].id, None);
    }

    #[test]
    fn top_emojis_sorted_by_rank() {
        let v = json!({"items":[{"emoji_id":"30","emoji_rank":3},{"emoji_id":"10","emoji_rank":1},{"emoji_id":"20","emoji_rank":2}]});
        assert_eq!(top_emoji_ids(&v), vec![Id(10), Id(20), Id(30)]);
    }

    #[test]
    fn sounds_badge_reads_the_guild_dismissed_state() {
        let varint = |mut v: u64, out: &mut Vec<u8>| loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                break;
            }
            out.push(b | 0x80);
        };
        let ld = |n: u64, body: Vec<u8>, out: &mut Vec<u8>| {
            varint((n << 3) | 2, out);
            varint(body.len() as u64, out);
            out.extend(body);
        };
        let mut st = Vec::new();
        varint(4 << 3, &mut st);
        varint(1_600_000_000_000_000_000, &mut st);
        let mut entry = Vec::new();
        varint(1 << 3, &mut entry);
        varint(870, &mut entry);
        ld(2, st, &mut entry);
        let mut gs = Vec::new();
        ld(12, entry, &mut gs);
        let mut ge = Vec::new();
        varint((1 << 3) | 1, &mut ge);
        ge.extend(42u64.to_le_bytes());
        ld(2, gs, &mut ge);
        let mut all = Vec::new();
        ld(1, ge, &mut all);
        let mut top = Vec::new();
        ld(3, all, &mut top);
        let d = sounds_dismissed(&top);
        assert_eq!(d.get(&42), Some(&1_600_000_000_000_000_000));
        assert!(sounds_dismissed(&[]).is_empty());
    }

    #[test]
    fn voice_and_text_channel_locks_match_client() {
        let ow = |allow: api::Permissions, deny: api::Permissions| api::PermissionOverwrite {
            id: api::Snowflake(1),
            kind: api::OverwriteType::Role,
            allow,
            deny,
        };
        let connect = api::Permissions::CONNECT;
        let view = api::Permissions::VIEW_CHANNEL;

        assert!(!is_everyone_denied(ChannelType::GuildVoice, connect | view, None));
        assert!(is_everyone_denied(ChannelType::GuildVoice, view, None));
        assert!(!is_everyone_denied(ChannelType::GuildVoice, view, Some(&ow(connect, api::Permissions::empty()))));
        assert!(is_everyone_denied(ChannelType::GuildVoice, connect | view, Some(&ow(api::Permissions::empty(), connect))));
        assert!(is_everyone_denied(ChannelType::GuildVoice, connect | view, Some(&ow(api::Permissions::empty(), view))));
        assert!(is_everyone_denied(ChannelType::GuildStageVoice, view, None));
        assert!(!is_everyone_denied(ChannelType::GuildStageVoice, connect | view, None));
        assert!(!is_everyone_denied(ChannelType::GuildText, view, None));
        assert!(is_everyone_denied(ChannelType::GuildText, view, Some(&ow(api::Permissions::empty(), view))));
    }
}
