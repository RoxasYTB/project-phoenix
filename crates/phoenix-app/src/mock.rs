use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    backend::*,
    demo_media::PEOPLE as NAMES,
    model::*,
};

pub const ME: Id = Id(1);

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[(self.next() as usize) % v.len()]
    }
}

const LINES: &[&str] = &[
    "hey everyone 👋",
    "has anyone tried the new **GPUI** release?",
    "it's *really* fast, like 120fps fast",
    "`cargo build --release` takes forever on my laptop though",
    "lol",
    "same",
    "check this out https://github.com/zed-industries/zed",
    "> quoting for posterity\nthat's a hot take",
    "```rust\nfn main() {\n    println!(\"hello from phoenix\");\n}\n```",
    "||spoiler: it was the borrow checker all along||",
    "anyone up for voice later? <@1>",
    "I pushed a fix to <#1003>, take a look",
    "__underlined__ and ~~struck~~ text works too",
    "brb",
    "that's wild",
    "the member list virtualization is smooth",
    "@everyone meeting in 10",
    "did you see the [docs](https://docs.rs/gpui)?",
    "ok I'm convinced, rewriting everything in rust 🦀",
    "gm",
    "can't reproduce on wayland, works fine here",
    "nvidia moment",
];

struct Inner {
    sink: Option<EventSink>,
    next_id: u64,
    history: HashMap<Id, Vec<Message>>,
    users: Vec<User>,
    guilds: Vec<Guild>,
    channels: Vec<Channel>,
    members: HashMap<Id, Vec<Member>>,
    roles: Vec<(Id, Vec<Role>)>,
    sent_uploads: Vec<Vec<Upload>>,
    mfa: Option<crate::auth::MfaOptions>,
    settings: crate::ui::settings::SyncedSettings,
}

#[derive(Clone)]
pub struct MockBackend {
    inner: Arc<Mutex<Inner>>,
    latency: Duration,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn mock_unreads() -> bool {
    std::env::var_os("PHX_MOCK_UNREADS").is_some()
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn dm_peer_state() -> Option<String> {
    std::env::var("PHX_MOCK_DM_PEER").ok()
}

fn user(i: usize) -> User {
    let p = &NAMES[i % NAMES.len()];
    let (n, d) = (p.name, p.display);
    User {
        id: Id(i as u64 + 1),
        name: n.into(),
        display_name: d.into(),
        avatar: ImageRef::Url(p.avatar_url().into()),
        bot: n.ends_with("bot") || n == "clyde",
        verified_bot: n == "clyde",
        tag: p.tag.map(|t| (t.1.into(), p.badge_url().map(Into::into))),
        tag_guild: p.tag.map(|t| Id(t.0)),
        nameplate: None,
        name_style: match i {
            2 => Some((11, 3, vec![0x691ced])),
            3 => Some((11, 4, vec![0xf42098])),
            4 => Some((11, 5, vec![0x0fcf86])),
            5 => Some((11, 8, vec![0xe9a9d5, 0xa9ddf0, 0xb9a7ee, 0xb9eacb])),
            _ => (i % 5 == 1).then(|| (17, 1, Vec::new())),
        },
        decoration: std::env::var("PHX_MOCK_DECO").ok().map(Into::into),
        flags: if i == 1 && dm_peer_state().as_deref() == Some("provisional") { User::PROVISIONAL_ACCOUNT } else { 0 },
    }
}

impl MockBackend {
    pub fn new() -> Self {
        let mut rng = Lcg(0x5eed);
        let users: Vec<User> = (0..NAMES.len()).map(user).collect();

        let guild_names = [
            ("Rust Programming", Some(0xCE422B)),
            ("GPUI Devs", None),
            ("Discord Phoenix", Some(0xCE422B)),
            ("Linux Gaming", None),
            ("KDE Community", Some(0x1D99F3)),
            ("Wayland Club", None),
            ("Pixel Art", None),
            ("Lo-fi Beats", None),
            ("Open Source Café", None),
            ("Mechanical Keyboards", None),
        ];
        let mut guilds = Vec::new();
        let mut channels = Vec::new();
        let mut roles = Vec::new();
        let mut members = HashMap::new();
        let mut cid = 1000u64;
        for (gi, (name, banner)) in guild_names.iter().enumerate() {
            let gid = Id(100 + gi as u64);
            guilds.push(Guild {
                id: gid,
                name: (*name).into(),
                acronym: acronym(name),
                icon: ImageRef::Default((gi % 5) as u8),
                unread: gi % 3 == 1,
                mentions: if gi == 3 { 4 } else if gi == 5 { 1 } else { 0 },
                banner_color: *banner,
                owner: Some(Id(2)),
                discoverable: gi == 0,
                community: true,
                boosts: [18, 3, 0, 7, 1, 14][gi % 6],
                boost_tier: [2, 1, 0, 2, 0, 3][gi % 6],
                afk_channel: None,
                system_channel: None,
                rules_channel: None,
                member_count: None,
            });
            let layout: &[(&str, &[(&str, ChannelKind)])] = &[
                (
                    "Information",
                    &[
                        ("welcome", ChannelKind::Text),
                        ("announcements", ChannelKind::Announcement),
                        ("rules", ChannelKind::Text),
                    ],
                ),
                (
                    "Text Channels",
                    &[
                        ("general", ChannelKind::Text),
                        ("help", ChannelKind::Forum),
                        ("showcase", ChannelKind::Text),
                        ("off-topic", ChannelKind::Text),
                        ("memes", ChannelKind::Text),
                    ],
                ),
                (
                    "Voice Channels",
                    &[
                        ("Lounge", ChannelKind::Voice),
                        ("Pair Programming", ChannelKind::Voice),
                        ("Stage", ChannelKind::Stage),
                        ("AFK", ChannelKind::Voice),
                    ],
                ),
            ];
            let mut pos = 0;
            for (cat, chans) in layout {
                cid += 1;
                let cat_id = Id(cid);
                channels.push(Channel {
                    id: cat_id,
                    guild: Some(gid),
                    parent: None,
                    name: (*cat).into(),
                    topic: None,
                    kind: ChannelKind::Category,
                    position: pos,
                    unread: false,
                    muted: false,
                    mentions: 0,
                    voice_users: vec![],
                });
                pos += 1;
                for (cn, kind) in chans.iter() {
                    cid += 1;
                    let voice_users = if *cn == "Lounge" {
                        vec![Id(3), Id(4), Id(7)]
                    } else if *cn == "Pair Programming" && gi == 0 {
                        vec![Id(6)]
                    } else if *cn == "Stage" && gi == 0 && crate::mock_stage::enabled() {
                        crate::mock_stage::USERS.to_vec()
                    } else {
                        vec![]
                    };
                    channels.push(Channel {
                        id: Id(cid),
                        guild: Some(gid),
                        parent: Some(cat_id),
                        name: (*cn).into(),
                        topic: match *cn {
                            "general" => Some(format!("General chat for {name}. Be nice! https://example.com/rules").into()),
                            "help" => Some("Ask questions, get answers".into()),
                            _ => None,
                        },
                        kind: *kind,
                        position: pos,
                        unread: matches!(*cn, "showcase" | "memes" | "announcements"),
                        muted: *cn == "memes",
                        mentions: if *cn == "showcase" && gi == 0 { 2 } else { 0 },
                        voice_users,
                    });
                    pos += 1;
                }
            }
            let role_list = vec![
                Role { id: Id(gid.0 * 10 + 1), name: "Admin".into(), color: Some(0xE74C3C), position: 3, hoist: true, icon: None, managed: false, connection: false },
                Role { id: Id(gid.0 * 10 + 2), name: "Moderator".into(), color: Some(0x3498DB), position: 2, hoist: true, icon: None, managed: false, connection: false },
                Role { id: Id(gid.0 * 10 + 3), name: "Contributor".into(), color: Some(0x2ECC71), position: 1, hoist: true, icon: None, managed: false, connection: false },
            ];
            let mut ms = Vec::new();
            for u in &users {
                let r = rng.next() % 10;
                let mut role_ids = Vec::new();
                if u.id.0 == 2 {
                    role_ids.push(role_list[0].id);
                } else if r < 2 {
                    role_ids.push(role_list[1].id);
                } else if r < 5 {
                    role_ids.push(role_list[2].id);
                }
                let presence = match rng.next() % 6 {
                    0 => Presence::Idle,
                    1 => Presence::Dnd,
                    2 => Presence::Offline,
                    _ => Presence::Online,
                };
                let activity = match rng.next() % 5 {
                    0 => Some("Playing Rust".into()),
                    1 => Some("Listening to Spotify".into()),
                    _ => None,
                };
                ms.push(Member {
                    user: u.id,
                    roles: role_ids,
                    presence: if u.id == ME { Presence::Online } else { presence },
                    activity,
                    nick: None,
                    avatar: None,
                    decoration: None,
                    nameplate: None,
                    joined_at: None,
                    rejoined: false,
                    premium_since: None,
                });
            }
            members.insert(gid, ms);
            roles.push((gid, role_list));
        }

        Self {
            inner: Arc::new(Mutex::new(Inner {
                sink: None,
                next_id: 10_000_000,
                history: HashMap::new(),
                users,
                guilds,
                channels,
                members,
                roles,
                sent_uploads: Vec::new(),
                mfa: None,
                settings: Default::default(),
            })),
            latency: Duration::from_millis(120),
        }
    }

    fn stage_channel(&self) -> Option<Id> {
        let inner = self.inner.lock().ok()?;
        crate::mock_stage::channel(&inner.channels)
    }

    pub fn sent_uploads(&self) -> Vec<Vec<Upload>> {
        self.inner.lock().map(|i| i.sent_uploads.clone()).unwrap_or_default()
    }

    pub fn synchronous() -> Self {
        Self { latency: Duration::ZERO, ..Self::new() }
    }

    fn history(inner: &mut Inner, channel: Id) -> &[Message] {
        if !inner.history.contains_key(&channel) {
            let h = Self::generate_history(inner, channel);
            inner.history.insert(channel, h);
        }
        &inner.history[&channel]
    }

    fn generate_history(inner: &mut Inner, channel: Id) -> Vec<Message> {
        let mut rng = Lcg(channel.0.wrapping_mul(2654435761));
        let n = std::env::var("PHOENIX_MOCK_HISTORY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120 + (rng.next() % 200) as usize);
        let mut ts = now() - n as i64 * 150 - 86_400;
        let mut out: Vec<Message> = Vec::with_capacity(n);
        let mut author = Id(2);
        for i in 0..n {
            ts += (rng.next() % 300) as i64 + 5;
            if rng.next().is_multiple_of(3) {
                author = rng.pick(&inner.users).id;
            }
            inner.next_id += 1;
            let mut m = Message {
                id: Id(inner.next_id),
                channel,
                author,
                timestamp: ts,
                edited: rng.next().is_multiple_of(17),
                content: (*rng.pick(LINES)).into(),
                reply_to: None,
                embeds: vec![],
                attachments: vec![],
                reactions: vec![],
                ..Default::default()
            };
            if i > 3 && rng.next().is_multiple_of(11) {
                m.reply_to = Some(out[i - 2 - (rng.next() as usize % 2)].id);
                m.reply_mentions_author = rng.next().is_multiple_of(2);
            }
            if rng.next().is_multiple_of(9) {
                m.reactions.push(Reaction { emoji: "👍".into(), count: 1 + (rng.next() % 5) as u32, me: rng.next().is_multiple_of(2), emoji_id: None, ..Default::default() });
                if rng.next().is_multiple_of(2) {
                    m.reactions.push(Reaction { emoji: "🔥".into(), count: 1 + (rng.next() % 3) as u32, me: false, emoji_id: None, ..Default::default() });
                }
            }
            if m.content.contains("github.com") {
                m.embeds.push(Embed {
                    color: Some(0x24292F),
                    provider: Some("GitHub".into()),
                    title: Some("zed-industries/zed".into()),
                    description: Some("Code at the speed of thought – Zed is a high-performance, multiplayer code editor.".into()),
                    url: Some("https://github.com/zed-industries/zed".into()),
                    ..Default::default()
                });
            }
            if rng.next().is_multiple_of(23) {
                m.content = "".into();
                m.attachments.push(Attachment {
                    filename: "screenshot.png".into(),
                    url: "https://example.invalid/screenshot.png".into(),
                    width: 1280,
                    height: 720,
                    is_image: true,
                    size: 345_678,
                    ..Default::default()
                });
            } else if rng.next().is_multiple_of(41) {
                m.attachments.push(Attachment {
                    filename: "phoenix-0.1.0.tar.gz".into(),
                    url: "https://example.invalid/phoenix.tar.gz".into(),
                    width: 0,
                    height: 0,
                    is_image: false,
                    size: 4_213_337,
                    ..Default::default()
                });
            }
            out.push(m);
        }
        if std::env::var_os("PHX_MOCK_COMPONENTS").is_some()
            && let Some(last) = out.last_mut()
        {
            last.content = "".into();
            let v = serde_json::json!({
                "id": last.id.0.to_string(), "channel_id": channel.0.to_string(), "flags": 32768,
                "author": {"id": last.author.0.to_string(), "username": "bot", "discriminator": "0"},
                "content": "", "timestamp": "2026-01-01T00:00:00+00:00", "type": 0,
                "components": [{"type": 17, "accent_color": 0xF1C40F, "components": [
                    {"type": 10, "content": "## Rust Programming"},
                    {"type": 14, "divider": true, "spacing": 1},
                    {"type": 10, "content": "**Members**\n1358"},
                    {"type": 10, "content": "**Created**\n6 May 2025"},
                    {"type": 10, "content": "**Boost**\nLevel 2"},
                    {"type": 14, "divider": true, "spacing": 1},
                    {"type": 10, "content": "-# Requested by wumpus"}
                ]}]
            });
            if let Ok(m) = serde_json::from_value::<phoenix_api::models::Message>(v) {
                crate::commands::store::record(&m);
            }
        }
        if let Some(ch) = inner.channels.iter().find(|c| c.id == channel && c.name == "showcase" && c.guild == inner.guilds.first().map(|g| g.id)) {
            let (guild, general) = (ch.guild.unwrap_or_default(), inner.channels.iter().find(|c| c.guild == ch.guild && c.name == "general").map_or(channel, |c| c.id));
            out.extend(crate::mock_showcase::messages(channel, guild, general, &mut inner.next_id, ts));
        }
        out
    }

    fn delayed<T: Send + 'static>(&self, f: impl FnOnce() -> T + Send + 'static) -> BoxFuture<T> {
        let latency = self.latency;
        if latency.is_zero() {
            return Box::pin(futures::future::ready(Ok(f())));
        }
        Box::pin(async move {
            let (tx, rx) = futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                std::thread::sleep(latency);
                let _ = tx.send(f());
            });
            rx.await.map_err(|_| BackendError("mock canceled".into()))
        })
    }

    fn emit_later(&self, delay: Duration, ev: impl FnOnce() -> AppEvent + Send + 'static) {
        let inner = self.inner.clone();
        if self.latency.is_zero() {
            let sink = inner.lock().ok().and_then(|i| i.sink.clone());
            if let Some(sink) = sink {
                sink(ev());
            }
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            let sink = inner.lock().ok().and_then(|i| i.sink.clone());
            if let Some(sink) = sink {
                sink(ev());
            }
        });
    }
}

pub fn mock_mfa(login: &str) -> Option<crate::auth::MfaOptions> {
    use crate::auth::MfaOptions;
    let (name, domain) = login.split_once('@')?;
    if domain != "phoenix.demo" {
        return None;
    }
    match name {
        "mfa" => Some(MfaOptions { totp: true, sms: true, backup: true, webauthn: true }),
        "totp" => Some(MfaOptions { totp: true, backup: true, ..Default::default() }),
        "sms" => Some(MfaOptions { sms: true, ..Default::default() }),
        "key" => Some(MfaOptions { webauthn: true, ..Default::default() }),
        _ => None,
    }
}

pub fn mock_mfa_check(method: crate::auth::MfaMethod, code: &str) -> Result<(), String> {
    use crate::auth::MfaMethod;
    match (method, code) {
        (MfaMethod::Totp | MfaMethod::Sms, "123456") | (MfaMethod::Backup, "abcd1234") => Ok(()),
        (_, "999999") => Err("Invalid two-factor auth ticket".into()),
        _ => Err("Invalid two-factor code".into()),
    }
}

impl Backend for MockBackend {
    fn load_settings(&self) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        let settings = self.inner.lock().unwrap().settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn save_settings(&self, patch: crate::ui::settings::SettingsPatch) -> BoxFuture<crate::ui::settings::SyncedSettings> {
        let mut inner = self.inner.lock().unwrap();
        inner.settings.apply(&patch);
        let settings = inner.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn voice_regions(&self) -> BoxFuture<Vec<(gpui::SharedString, gpui::SharedString)>> {
        let regions = ["Atlanta", "Brazil", "Hong Kong", "India", "Japan", "Rotterdam", "Russia", "Singapore", "South Africa", "Sydney", "US Central", "US East", "US South", "US West"];
        Box::pin(async move { Ok(regions.iter().map(|n| (gpui::SharedString::from(n.to_lowercase().replace(' ', "-")), gpui::SharedString::from(*n))).collect()) })
    }
    fn app_directory_collections(&self, _locale: String) -> BoxFuture<serde_json::Value> {
        let app = |id: u64, name: &str, desc: &str, pos: i64| {
            serde_json::json!({"position": pos, "application": {"id": id.to_string(), "name": name, "description": desc}})
        };
        let v = serde_json::json!([
            {"title": "Promoted", "type": 5, "position": 1, "application_directory_collection_items": [
                app(9001, "Pixel Garden", "Grow a shared garden with friends.", 1),
                app(9002, "Word Ladder", "One daily word puzzle for the whole server.", 2),
                app(9003, "Tiny Racers", "Quick races for up to eight players.", 3),
                app(9004, "Watch Party", "Watch videos together in sync.", 4),
            ]},
            {"title": "Puzzle Games", "type": 4, "position": 2, "application_directory_collection_items": [
                app(9002, "Word Ladder", "One daily word puzzle for the whole server.", 1),
                app(9005, "Hex Tiles", "Slide hexagons into place.", 2),
                app(9006, "Sudoku Club", "Solve the daily grid together.", 3),
                app(9007, "Block Drop", "Clear lines before the stack fills.", 4),
                app(9008, "Maze Run", "Find the exit first.", 5),
            ]},
            {"title": "Chill Together", "type": 4, "position": 3, "application_directory_collection_items": [
                app(9004, "Watch Party", "Watch videos together in sync.", 1),
                app(9009, "Sketchboard", "Draw on one canvas.", 2),
            ]},
        ]);
        Box::pin(async move { Ok(v) })
    }
    fn notify_prefs(&self) -> Option<crate::notifications::NotifyPrefs> {
        let mut p = crate::notifications::NotifyPrefs::default();
        p.update(&serde_json::json!({
            "guild_id": "100",
            "message_notifications": 1,
            "channel_overrides": [
                {"channel_id": "1003", "message_notifications": 0, "muted": false},
                {"channel_id": "1006", "message_notifications": 1, "muted": false},
                {"channel_id": "1010", "message_notifications": 2, "muted": true},
            ],
        }));
        Some(p)
    }
    fn game_widgets(&self, _user: Id) -> BoxFuture<Vec<crate::ui::profile_games::GameWidget>> {
        use crate::ui::profile_games::{Game, GameWidget, Kind};
        let g = |id: u64, n: &str| Game { id, name: n.to_owned().into(), cover: None, icon: None };
        Box::pin(async move {
            Ok(vec![
                GameWidget { kind: Kind::Favorite, games: vec![g(1, "Ferris Quest")], order: 1 },
                GameWidget { kind: Kind::Played, games: vec![g(2, "Crab Kart"), g(3, "Borrow Checker"), g(4, "Rustlings")], order: 2 },
            ])
        })
    }
    fn nitro_tier(&self) -> Option<u8> {
        if std::env::var_os("PHX_NO_PREMIUM").is_some() {
            Some(0)
        } else {
            Some(2)
        }
    }
    fn soundboard_default_sounds(&self) -> BoxFuture<Vec<SoundboardSound>> {
        Box::pin(async {
            Ok(vec![SoundboardSound {
                id: Id(1),
                name: Some("Quack".into()),
                available: true,
                volume: 1.,
                ..Default::default()
            }])
        })
    }

    fn user_profile(&self, user: Id, _guild: Option<Id>) -> BoxFuture<crate::presence::Profile> {
        let private = user == Id(2) && dm_peer_state().as_deref() == Some("private");
        Box::pin(async move {
            if private {
                Ok(crate::presence::Profile { user, private: true, created: 1_600_000_000, ..Default::default() })
            } else {
                Err(BackendError(crate::t!("backend.profiles_unavailable").to_string()))
            }
        })
    }
    fn premium_tier2(&self) -> bool {
        std::env::var_os("PHX_NO_PREMIUM").is_none()
    }
    fn has_premium(&self) -> bool {
        std::env::var_os("PHX_NO_PREMIUM").is_none()
    }
    fn subscriptions(&self) -> BoxFuture<Option<crate::ui::settings::subscriptions::Subs>> {
        use crate::ui::settings::subscriptions as sub;
        let has_nitro = self.nitro_tier() == Some(2);
        let users: Vec<User> = self.inner.lock().map(|i| i.users.iter().take(6).cloned().collect()).unwrap_or_default();
        Box::pin(async move {
            let mut members: Vec<sub::Member> = users.into_iter().enumerate().map(|(k, user)| sub::Member { owner: k == 1, me: user.id == ME, user }).collect();
            members.sort_by_key(|m| !m.owner);
            let credits = vec![sub::Credit { plan: "sp.su.plan_basic_month", tier: 0, months: 1 }];
            let personal = has_nitro.then(|| sub::Subscription {
                plan: "sp.su.plan_nitro_month",
                tier: 2,
                period_end: Some((chrono::Utc::now() + chrono::Duration::days(30)).fixed_offset()),
                canceled: false,
            }).into_iter().collect();
            Ok(Some(sub::Subs { squad: has_nitro.then_some(sub::Squad { members }), credits, personal: Some(personal) }))
        })
    }
    fn update_profile(&self, _guild: Option<Id>, _patch: crate::ui::settings::profile_editor::ProfilePatch) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn set_event_sink(&self, sink: EventSink) {
        if let Ok(mut i) = self.inner.lock() {
            i.sink = Some(sink);
        }
    }

    fn needs_mfa(&self) -> bool {
        self.inner.lock().is_ok_and(|i| i.mfa.is_some())
    }

    fn mfa_options(&self) -> crate::auth::MfaOptions {
        self.inner.lock().ok().and_then(|i| i.mfa).unwrap_or_default()
    }

    fn reset_mfa(&self) {
        if let Ok(mut i) = self.inner.lock() {
            i.mfa = None;
        }
    }

    fn send_mfa_sms(&self) -> BoxFuture<String> {
        self.delayed(|| "+*******1234".to_owned())
    }

    fn login_mfa(&self, method: crate::auth::MfaMethod, code: String) -> BoxFuture<ReadyData> {
        if let Err(e) = mock_mfa_check(method, &code) {
            let wait = self.delayed(|| ());
            return Box::pin(async move {
                wait.await?;
                Err(BackendError(e))
            });
        }
        self.reset_mfa();
        self.login(Credentials::Token("mock-token".into()))
    }

    fn restore_session(&self) -> Option<BoxFuture<ReadyData>> {
        std::env::var("PHOENIX_MOCK_AUTOLOGIN")
            .is_ok_and(|v| !v.is_empty() && v != "0")
            .then(|| self.login(Credentials::Token("mock-token".into())))
    }

    fn login(&self, creds: Credentials) -> BoxFuture<ReadyData> {
        let inner = self.inner.clone();
        if let Credentials::Password { login, password } = &creds
            && !password.is_empty()
            && let Some(opts) = mock_mfa(login)
        {
            if let Ok(mut i) = inner.lock() {
                i.mfa = Some(opts);
            }
            let wait = self.delayed(|| ());
            return Box::pin(async move {
                wait.await?;
                Err(BackendError(String::new()))
            });
        }
        let ok = match &creds {
            Credentials::Token(t) => !t.trim().is_empty(),
            Credentials::Password { login, password } => !login.is_empty() && !password.is_empty(),
        };
        if ok {
            let peer = dm_peer_state();
            let rels: Vec<_> = (1..NAMES.len().min(15) as u64)
                .filter(|n| *n != 1 || peer.is_none() || peer.as_deref() == Some("incoming"))
                .map(|n| crate::ui::home::data::Relationship {
                    user: Id(n + 1),
                    kind: match n {
                        1 if peer.as_deref() == Some("incoming") => crate::ui::home::data::RelationshipKind::Incoming,
                        12 => crate::ui::home::data::RelationshipKind::Incoming,
                        13 => crate::ui::home::data::RelationshipKind::Outgoing,
                        _ => crate::ui::home::data::RelationshipKind::Friend,
                    },
                    nickname: None,
                    since: Some(1_700_000_000),
                    ignored: false,
                    spam: false,
                    note: None,
                })
                .collect();
            self.emit_later(Duration::from_millis(30), move || {
                AppEvent::Nav(crate::ui::home::data::NavEvent::Snapshot {
                    relationships: rels,
                    folders: if std::env::var_os("PHX_MOCK_FOLDERS").is_some() {
                        vec![
                            crate::ui::home::data::GuildFolder {
                                id: Some(1),
                                name: Some("Desktop".into()),
                                color: Some(0x5865F2),
                                guilds: (104..108).map(Id).collect(),
                            },
                            crate::ui::home::data::GuildFolder { id: Some(2), name: None, color: None, guilds: vec![Id(108), Id(109)] },
                        ]
                    } else {
                        vec![]
                    },
                    threads: vec![],
                    extras: vec![],
                    message_requests: vec![],
                    spam_requests: vec![],
                })
            });
        }
        let stage = if ok && crate::mock_stage::enabled() { self.stage_channel() } else { None };
        if let Some(stage) = stage {
            for (user, flags) in crate::mock_stage::seed(stage) {
                self.emit_later(Duration::from_millis(40), move || AppEvent::VoiceFlags { user, flags });
            }
        }
        Box::pin(async move {
            if !ok {
                return Err(BackendError("Login or password is invalid.".into()));
            }
            let i = inner.lock().map_err(|_| BackendError("poisoned".into()))?;
            let dm_channels = i.users[1..8]
                .iter()
                .enumerate()
                .map(|(n, u)| Channel {
                    id: Id(900 + n as u64),
                    guild: None,
                    parent: None,
                    name: u.display_name.clone(),
                    topic: None,
                    kind: ChannelKind::Text,
                    position: n as i32,
                    unread: n == 1,
                    muted: false,
                    mentions: 0,
                    voice_users: vec![u.id],
                })
                .collect();
            Ok(ReadyData {
                me: i.users[0].clone(),
                users: i.users.clone(),
                guilds: i.guilds.clone(),
                channels: i.channels.clone(),
                roles: i.roles.clone(),
                dm_channels,
                presences: i
                    .users
                    .iter()
                    .map(|u| {
                        let p = match u.id.0 % 5 {
                            _ if u.id == ME => Presence::Online,
                            0 => Presence::Idle,
                            3 => Presence::Dnd,
                            4 => Presence::Offline,
                            _ => Presence::Online,
                        };
                        (u.id, p)
                    })
                    .collect(),
            })
        })
    }

    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        if let Some(g) = guild {
            let members = self.inner.lock().ok().and_then(|i| i.members.get(&g).cloned());
            if let Some(members) = members {
                let playing: Vec<Id> = members.iter().filter(|m| m.presence != Presence::Offline).map(|m| m.user).take(3).collect();
                self.emit_later(Duration::from_millis(60), move || AppEvent::GuildMembers { guild: g, members });
                let games = ["Hearts of Iron IV", "Fortnite", "VALORANT"];
                for (k, user) in playing.into_iter().enumerate() {
                    let activities = vec![crate::presence::Activity {
                        kind: crate::presence::ActivityKind::Playing,
                        name: games[k].into(),
                        details: None,
                        state: None,
                        emoji: None,
                        url: None,
                        display_type: None,
                        image: None,
                        start: Some((now() - 86_400 * (2 - k as i64).max(1)) * 1000),
                        application_id: None,
                        party: None,
                        platform: None,
                        flags: 0,
                    }];
                    self.emit_later(Duration::from_millis(80), move || AppEvent::Activities { user, activities });
                }
            }
        }
        Box::pin(async { Ok(()) })
    }

    fn select_channel(&self, _channel: Id) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        let inner = self.inner.clone();
        self.delayed(move || {
            let Ok(mut i) = inner.lock() else { return vec![] };
            let h = Self::history(&mut i, channel);
            let end = before
                .and_then(|b| h.iter().position(|m| m.id == b))
                .unwrap_or(h.len());
            let start = end.saturating_sub(limit as usize);
            h[start..end].to_vec()
        })
    }

    fn load_messages_at(&self, channel: Id, at: HistoryAt, limit: u32) -> BoxFuture<Vec<Message>> {
        if !mock_unreads() {
            return Box::pin(async { Ok(Vec::new()) });
        }
        let inner = self.inner.clone();
        self.delayed(move || {
            let Ok(mut i) = inner.lock() else { return vec![] };
            let h = Self::history(&mut i, channel);
            let (HistoryAt::Around(id) | HistoryAt::After(id)) = at;
            let pos = h.iter().position(|m| m.id.0 >= id.0).unwrap_or(h.len());
            let start = if matches!(at, HistoryAt::Around(_)) { pos.saturating_sub(limit as usize / 2) } else { pos };
            h[start..(start + limit as usize).min(h.len())].to_vec()
        })
    }

    fn inbox_unreads(&self, guild_order: &[Id]) -> Vec<crate::ui::inbox_unreads::Entry> {
        if !mock_unreads() {
            return Vec::new();
        }
        let Ok(mut i) = self.inner.lock() else { return Vec::new() };
        let picks: Vec<(Id, Option<Id>)> = guild_order
            .iter()
            .flat_map(|g| i.channels.iter().filter(move |c| c.guild == Some(*g) && c.kind == ChannelKind::Text).take(2).map(|c| (c.id, c.guild)))
            .take(5)
            .collect();
        picks
            .into_iter()
            .zip([4usize, 9, 3, 12, 6])
            .filter_map(|((channel, guild), n)| {
                let h = Self::history(&mut i, channel);
                Some(crate::ui::inbox_unreads::Entry {
                    channel,
                    guild,
                    kind: crate::ui::inbox_unreads::Kind::Messages,
                    oldest_read: h.get(h.len().checked_sub(n + 1)?)?.id,
                    newest: h.last()?.id,
                    mentions: 0,
                    sort: 5,
                })
            })
            .collect()
    }

    fn send_message_ex(
        &self,
        channel: Id,
        content: String,
        reply: Option<(Id, bool)>,
        files: Vec<Upload>,
        nonce: u64,
        progress: Option<UploadProgress>,
    ) -> BoxFuture<Message> {
        let total: u64 = files.iter().map(|f| f.bytes.len() as u64).sum();
        if let Some(p) = &progress
            && !(p(0, total) && p(total, total))
        {
            return Box::pin(async { Err(BackendError("upload cancelled".into())) });
        }
        let atts: Vec<Attachment> = files
            .iter()
            .map(|f| {
                let image = crate::ui::chat::state::is_image_name(&f.filename).is_some();
                Attachment {
                    filename: f.filename.clone().into(),
                    size: f.bytes.len() as u64,
                    is_image: image,
                    kind: if image { AttachmentKind::Image } else { AttachmentKind::File },
                    ..Default::default()
                }
            })
            .collect();
        if let Ok(mut i) = self.inner.lock() {
            i.sent_uploads.push(files);
        }
        let fut = self.send_message(channel, content, reply.map(|r| r.0));
        let inner = self.inner.clone();
        Box::pin(async move {
            let mut m = fut.await?;
            m.attachments = atts;
            m.nonce = Some(nonce);
            if let Ok(mut i) = inner.lock()
                && let Some(h) = i.history.get_mut(&channel).and_then(|h| h.iter_mut().find(|x| x.id == m.id))
            {
                h.attachments = m.attachments.clone();
            }
            Ok(m)
        })
    }

    fn send_message(&self, channel: Id, content: String, reply_to: Option<Id>) -> BoxFuture<Message> {
        let inner = self.inner.clone();
        let msg = {
            let Ok(mut i) = inner.lock() else {
                return Box::pin(async { Err(BackendError("poisoned".into())) });
            };
            i.next_id += 1;
            let m = Message {
                id: Id(i.next_id),
                channel,
                author: ME,
                timestamp: now(),
                edited: false,
                content: content.clone().into(),
                reply_to,
                embeds: vec![],
                attachments: vec![],
                reactions: vec![],
                ..Default::default()
            };
            i.history.entry(channel).or_default().push(m.clone());
            m
        };
        if content.to_lowercase().contains("wumpus") || content.ends_with('?') {
            self.emit_later(Duration::from_millis(400), move || AppEvent::TypingStart { channel, user: Id(2), nick: None });
            let inner2 = self.inner.clone();
            let reply_to = msg.id;
            self.emit_later(Duration::from_millis(2600), move || {
                let id = inner2.lock().map(|mut i| {
                    i.next_id += 1;
                    i.next_id
                });
                AppEvent::MessageCreate(Message {
                    id: Id(id.unwrap_or(1)),
                    channel,
                    author: Id(2),
                    timestamp: now(),
                    edited: false,
                    content: "*wumpus noises* — this is a **mock** reply from the fake backend".into(),
                    reply_to: Some(reply_to),
                    embeds: vec![],
                    attachments: vec![],
                    reactions: vec![Reaction { emoji: "💜".into(), count: 1, me: false, emoji_id: None, ..Default::default() }],
                    ..Default::default()
                })
            });
        }
        let out = msg.clone();
        self.delayed(move || out)
    }

    fn edit_message(&self, channel: Id, id: Id, content: String) -> BoxFuture<Message> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let mut i = inner.lock().map_err(|_| BackendError("poisoned".into()))?;
            let m = i
                .history
                .get_mut(&channel)
                .and_then(|h| h.iter_mut().find(|m| m.id == id))
                .ok_or_else(|| BackendError("Unknown Message".into()))?;
            m.content = content.into();
            m.edited = true;
            Ok(m.clone())
        })
    }

    fn trigger_typing(&self, _channel: Id) -> BoxFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()> {
        self.emit_later(Duration::from_millis(50), move || AppEvent::VoiceStateUpdate {
            guild: None,
            channel,
            user: ME,
            own: true,
        });
        if channel.is_some() && crate::mock_stage::enabled() && channel == self.stage_channel() {
            let flags = crate::mock_stage::join(ME);
            self.emit_later(Duration::from_millis(55), move || AppEvent::VoiceFlags { user: ME, flags });
        }
        if channel.is_some() {
            self.emit_later(Duration::from_millis(60), || {
                AppEvent::Voice(crate::voice::VoiceUiEvent::Status(crate::voice::VoiceStatus::Connected))
            });
            self.emit_later(Duration::from_millis(70), || AppEvent::Voice(crate::voice::VoiceUiEvent::Ping(38)));
        }
        Box::pin(async { Ok(()) })
    }
    fn delete_message(&self, channel: Id, id: Id) -> BoxFuture<()> {
        if let Some(h) = self.inner.lock().unwrap().history.get_mut(&channel) {
            h.retain(|m| m.id != id);
        }
        self.emit_later(Duration::from_millis(20), move || AppEvent::MessageDelete { channel, id });
        Box::pin(async { Ok(()) })
    }
    fn action(&self, req: crate::actions::ActionRequest) -> BoxFuture<crate::actions::ActionReply> {
        if let crate::actions::ActionRequest::Stage(call) = &req
            && crate::mock_stage::enabled()
        {
            for (user, flags) in crate::mock_stage::on_call(ME, call) {
                self.emit_later(Duration::from_millis(30), move || AppEvent::VoiceFlags { user, flags });
            }
        }
        let users = self.inner.lock().unwrap().users.clone();
        Box::pin(async move { crate::actions::mock::reply(req, users) })
    }

    fn channel_permissions(&self, channel: Id) -> Option<u64> {
        (crate::mock_stage::enabled() && self.stage_channel() == Some(channel)).then_some(crate::ui::stage::STAGE_MODERATOR | crate::ui::stage::REQUEST_TO_SPEAK)
    }

    fn guild_features(&self, _guild: Id) -> Vec<String> {
        vec!["COMMUNITY".into()]
    }

    fn load_pins(&self, channel: Id) -> BoxFuture<Vec<Message>> {
        let inner = self.inner.clone();
        self.delayed(move || {
            let Ok(mut i) = inner.lock() else { return vec![] };
            if !i.history.contains_key(&channel) {
                let h = Self::generate_history(&mut i, channel);
                i.history.insert(channel, h);
            }
            let h = &i.history[&channel];
            h.iter()
                .rev()
                .filter(|m| !m.content.is_empty())
                .step_by(4)
                .take(3)
                .map(|m| Message { pinned: true, ..m.clone() })
                .collect()
        })
    }

    fn search_messages(&self, req: crate::ui::search::SearchRequest) -> BoxFuture<crate::ui::search::SearchPage> {
        let inner = self.inner.clone();
        self.delayed(move || {
            let Ok(i) = inner.lock() else { return Default::default() };
            let q = req.content.unwrap_or_default().to_lowercase();
            let mut hits: Vec<Message> =
                i.history.values().flatten().filter(|m| q.is_empty() || m.content.to_lowercase().contains(&q)).cloned().collect();
            hits.sort_by_key(|m| std::cmp::Reverse(m.id.0));
            let total = hits.len() as u32;
            hits.truncate(25);
            crate::ui::search::SearchPage { total, messages: hits, indexing: None, threads: Vec::new() }
        })
    }

    fn parity(&self, req: crate::commands::ParityRequest) -> BoxFuture<crate::commands::ParityReply> {
        use crate::commands::{ForumPost, ParityReply, ParityRequest};
        match req {
            ParityRequest::ArchivedThreads { .. } => {
                let inner = self.inner.clone();
                self.delayed(move || {
                    let Ok(mut i) = inner.lock() else { return ParityReply::ForumPosts { posts: vec![], has_more: false } };
                    let users: Vec<Id> = i.users.iter().map(|u| u.id).collect();
                    let titles = ["wayland fractional scaling", "release checklist", "gpui text rendering"];
                    let mut posts = Vec::new();
                    for (k, title) in titles.iter().enumerate() {
                        i.next_id += 1;
                        posts.push(ForumPost {
                            thread: Id(i.next_id),
                            title: (*title).to_string(),
                            tags: vec![],
                            message_count: 12 - 4 * k as u32,
                            first_message: None,
                            owner: users.get(k + 1).copied(),
                            last_message: None,
                            member_ids_preview: users.iter().skip(k).take(3 - k).copied().collect(),
                            member_count: 3 - k as u32,
                            pinned: false,
                            archived: true,
                            locked: false,
                        });
                    }
                    ParityReply::ForumPosts { posts, has_more: false }
                })
            }
            ParityRequest::ScheduledEvents { .. } => Box::pin(async {
                let now = chrono::Utc::now().timestamp();
                Ok(ParityReply::Events(vec![
                    crate::commands::GuildEvent {
                        id: Id(9_000_001),
                        name: "Community game night".into(),
                        description: "Bring a friend, we play until midnight.".into(),
                        start: now + 2 * 86_400,
                        status: 1,
                        location: Some("Voice lounge".into()),
                        user_count: 12,
                        ..Default::default()
                    },
                    crate::commands::GuildEvent {
                        id: Id(9_000_002),
                        name: "Release party".into(),
                        start: now - 600,
                        status: 2,
                        user_count: 31,
                        ..Default::default()
                    },
                ]))
            }),
            ParityRequest::ServerGuide { guild } => {
                let chans: Vec<Id> = self.inner.lock().map(|i| i.channels.iter().filter(|c| c.guild == Some(guild) && c.kind == crate::model::ChannelKind::Text).map(|c| c.id).take(2).collect()).unwrap_or_default();
                Box::pin(async move {
                    use crate::commands::guide::{GuideEmoji, GuideEntry, GuideTrait, GuildGuide, GuildProfile};
                    let entry = |channel: Id, title: &str, description: &str| GuideEntry { channel, title: title.into(), description: description.into(), ..Default::default() };
                    let mut resources = Vec::new();
                    if let Some(&c) = chans.first() {
                        resources.push(entry(c, "Start here", "Welcome to **Phoenix Lounge**. Read the rules and pick your roles to unlock the rest of the server."));
                    }
                    if let Some(&c) = chans.get(1) {
                        resources.push(entry(c, "Frequently asked questions", "Answers to the questions newcomers ask the most."));
                    }
                    let tr = |label: &str, emoji: &str| GuideTrait { label: label.into(), emoji: GuideEmoji { name: Some(emoji.into()), ..Default::default() } };
                    Ok(ParityReply::Guide(Box::new(GuildGuide {
                        enabled: true,
                        resources,
                        profile: Some(GuildProfile {
                            member_count: 12_480,
                            online_count: 1_932,
                            description: "A friendly place to talk about native apps, Rust and GPU user interfaces.".into(),
                            traits: vec![tr("Rust", "🦀"), tr("Design", "🎨"), tr("Open source", "📦")],
                            features: vec!["COMMUNITY".into(), "DISCOVERABLE".into()],
                            ..Default::default()
                        }),
                        ..Default::default()
                    })))
                })
            }
            _ => Box::pin(async { Err(crate::backend::BackendError("Not available offline.".into())) }),
        }
    }

    fn guild_profile(&self, guild: Id) -> BoxFuture<crate::commands::guide::GuildProfile> {
        let g = self.inner.lock().ok().and_then(|i| i.guilds.iter().find(|g| g.id == guild).cloned());
        Box::pin(async move {
            use crate::commands::guide::{GuideEmoji, GuideTrait, GuildProfile};
            let tr = |label: &str, emoji: &str| GuideTrait {
                label: label.into(),
                emoji: GuideEmoji { name: Some(emoji.into()), ..Default::default() },
            };
            let (name, tag, desc, members, online, color) = if let Some(g) = g {
                (
                    g.name.to_string(),
                    Some("PHX".to_owned()),
                    "A native Rust + GPUI Discord client community.".to_owned(),
                    g.member_count.unwrap_or(1_240) as u64,
                    312,
                    g.banner_color.map(|c| format!("#{c:06x}")),
                )
            } else {
                let p = NAMES.iter().find(|p| p.tag.is_some_and(|t| t.0 == guild.0));
                let tag_str = p.and_then(|p| p.tag).map(|t| t.1.to_owned()).unwrap_or_else(|| "TAG".to_owned());
                let badge_str = p.and_then(|p| p.tag).map(|t| t.2.to_owned());
                return Ok(GuildProfile {
                    id: guild,
                    name: format!("{tag_str} Community"),
                    visibility: 1,
                    member_count: 14_820,
                    online_count: 2_340,
                    description: "Official community server for fans, builders, and friends.".into(),
                    tag: Some(tag_str),
                    badge_hash: badge_str,
                    traits: vec![tr("Gaming", "🎮"), tr("Creative", "🎨"), tr("Chill", "✨")],
                    brand_color_primary: Some("#5865F2".into()),
                    features: vec!["COMMUNITY".into(), "DISCOVERABLE".into()],
                    ..Default::default()
                });
            };
            Ok(GuildProfile {
                id: guild,
                name,
                visibility: 1,
                member_count: members,
                online_count: online,
                description: desc,
                tag,
                traits: vec![tr("Rust", "🦀"), tr("GPUI", "⚡"), tr("Open source", "📦")],
                brand_color_primary: color,
                features: vec!["COMMUNITY".into(), "DISCOVERABLE".into()],
                ..Default::default()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::{SettingsPatch, synced_map};

    #[test]
    fn settings_writes_apply_offline() {
        let b = MockBackend::new();
        let m = Default::default();
        let s = futures::executor::block_on(b.save_settings(SettingsPatch::Proto("data-sponsored", synced_map::bool_wire(&m, "data-sponsored", false)))).unwrap();
        assert!(!s.flag("data-sponsored"));
        assert!(synced_map::disabled(&s.proto, s.compact, "data-third"));
        let s = futures::executor::block_on(b.save_settings(SettingsPatch::Proto("share-profile", synced_map::pick_wire("share-profile", 2)))).unwrap();
        assert_eq!(s.pick("share-profile"), 2);
        assert_eq!(s.proto.get("share-profile"), Some(&1), "FRIENDS_ONLY on the wire");
        let s = futures::executor::block_on(b.save_settings(SettingsPatch::Proto("push-delay", synced_map::pick_wire("push-delay", 9)))).unwrap();
        assert_eq!(s.proto.get("push-delay"), Some(&600));
        let s = futures::executor::block_on(b.save_settings(SettingsPatch::Rest("e-tips", true))).unwrap();
        assert!(s.flag("e-tips"));
    }

    #[test]
    fn mock_mfa_walks_the_login_flow() {
        use crate::auth::{MfaMethod, MfaOptions};
        let b = MockBackend::synchronous();
        let pw = |login: &str| Credentials::Password { login: login.into(), password: "pw".into() };
        let first = futures::executor::block_on(b.login(pw("totp@phoenix.demo")));
        assert_eq!(first.err().map(|e| e.0), Some(String::new()));
        assert!(b.needs_mfa());
        assert_eq!(b.mfa_options(), MfaOptions { totp: true, backup: true, ..Default::default() });
        let wrong = futures::executor::block_on(b.login_mfa(MfaMethod::Totp, "000000".into()));
        assert_eq!(wrong.err().map(|e| e.0).as_deref(), Some("Invalid two-factor code"));
        assert!(b.needs_mfa());
        assert_eq!(futures::executor::block_on(b.send_mfa_sms()).unwrap(), "+*******1234");
        assert!(futures::executor::block_on(b.login_mfa(MfaMethod::Backup, "abcd1234".into())).is_ok());
        assert!(!b.needs_mfa());
        assert!(mock_mfa("someone@example.com").is_none());
        assert_eq!(mock_mfa("key@phoenix.demo").map(|o| o.methods()), Some(vec![MfaMethod::Webauthn]));
    }
}
