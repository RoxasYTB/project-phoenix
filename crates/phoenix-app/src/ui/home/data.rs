use std::collections::{HashMap, HashSet};

use gpui::SharedString;

use crate::model::{Channel, Id, ImageRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipKind {
    None,
    Friend,
    Blocked,
    Incoming,
    Outgoing,
}

#[derive(Clone, Debug)]
pub struct Relationship {
    pub user: Id,
    pub kind: RelationshipKind,
    pub nickname: Option<SharedString>,
    pub since: Option<i64>,
    pub ignored: bool,
    pub spam: bool,
    pub note: Option<SharedString>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuildFolder {
    pub id: Option<u64>,
    pub name: Option<SharedString>,
    pub color: Option<u32>,
    pub guilds: Vec<Id>,
}

#[derive(Clone, Debug, Default)]
pub struct ChannelExtra {
    pub nsfw: bool,
    pub locked: bool,
    pub group_icon: Option<ImageRef>,
    pub recipients: usize,
    pub group_name: Option<SharedString>,
    pub group_named: bool,
    pub members: Vec<Id>,
    pub owner: Option<Id>,
    pub slowmode: u32,
    pub bypass_slowmode: bool,
    pub send_blocked: bool,
    pub thread_archived: bool,
    pub thread_locked: bool,
    pub can_manage_threads: bool,
    pub connect_denied: bool,
    pub media: bool,
}

#[derive(Clone, Debug, Default)]
pub struct InvitePreview {
    pub code: SharedString,
    pub guild: Option<Id>,
    pub guild_name: SharedString,
    pub guild_icon: Option<ImageRef>,
    pub channel_name: Option<SharedString>,
    pub inviter: Option<SharedString>,
    pub online: u32,
    pub members: u32,
    pub kind: InviteKind,
    pub channel_id: Option<Id>,
    pub channel_type: Option<u8>,
    pub features: Vec<String>,
    pub profile: Option<crate::commands::guide::GuildProfile>,
    pub splash: Option<SharedString>,
    pub inviter_id: Option<Id>,
    pub inviter_username: Option<SharedString>,
    pub inviter_avatar: Option<ImageRef>,
    pub channel_icon: Option<ImageRef>,
    pub recipients: Vec<SharedString>,
    pub recipient_users: Vec<InviteUser>,
    pub event_name: Option<SharedString>,
    pub event: Option<crate::commands::GuildEvent>,
    pub event_creator: Option<InviteUser>,
    pub target_type: Option<u8>,
    pub target_user: Option<InviteUser>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InviteUser {
    pub id: Option<Id>,
    pub name: SharedString,
    pub avatar: Option<ImageRef>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InviteKind {
    #[default]
    Guild,
    GroupDm,
    Friend,
}

#[derive(Debug)]
pub enum NavEvent {
    Snapshot {
        relationships: Vec<Relationship>,
        folders: Vec<GuildFolder>,
        threads: Vec<Channel>,
        extras: Vec<(Id, ChannelExtra)>,
        message_requests: Vec<Id>,
        spam_requests: Vec<Id>,
    },
    RelationshipUpsert(Relationship),
    RelationshipRemove(Id),
    GameRequests(Vec<(Id, Id)>),
    GameRequest { user: Id, app: Id, incoming: bool },
    Activity { user: Id, activity: Option<SharedString> },
    ThreadUpsert(Channel),
    ThreadDelete(Id),
    Extras(Vec<(Id, ChannelExtra)>),
    Banners(Vec<(Id, Option<String>)>),
    Folders(Vec<GuildFolder>),
    SoundsSeen(Vec<(Id, u64)>),
    QuestOnboardingDismissed(bool),
}

#[derive(Clone, Debug)]
pub enum RelAction {
    Accept,
    AcceptStranger,
    Remove,
    Block,
    AddByTag { tag: String, note: Option<String> },
    Ignore,
    ClearPending,
    ClearSpamAndIgnored,
}

#[derive(Clone, Debug)]
pub enum NavRequest {
    FetchInvite(String),
    FetchInviteEvent(String, Id),
    TopEmojis(Id),
    AcceptInvite(String),
    CreateGuild { name: String, icon: Option<String>, template: Option<String>, community: Option<bool> },
    FetchOnboarding(Id),
    SubmitOnboarding { guild: Id, options: Vec<String>, prompts: Vec<String> },
    FetchMemberVerification(Id),
    SubmitMemberVerification { guild: Id, version: String, rules: Vec<String> },
    JoinGuild(Id),
    CreateInvite(Id),
    LeaveGuild { guild: Id, lurking: bool },
    Relationship { user: Id, action: RelAction },
    FetchRelationships,
    OpenDm(Id),
    CloseDm(Id),
    MessageRequest { channel: Id, accept: bool },
    RejectMessageRequests(Vec<Id>),
    AckChannel(Id),
    AckGuild(Id),
    Mute { guild: Option<Id>, channel: Option<Id>, muted: bool, secs: Option<u32> },
    NotifyLevel { guild: Option<Id>, channel: Option<Id>, level: u8 },
    GuildNotify { guild: Id, fields: serde_json::Value },
}

#[derive(Clone, Debug)]
pub enum NavReply {
    Done,
    Invite(Box<InvitePreview>),
    Ids(Vec<Id>),
    Text(SharedString),
    Channel(Channel),
    Json(serde_json::Value),
    Relationships { records: Vec<Relationship>, users: Vec<crate::model::User> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FriendsTab {
    #[default]
    Online,
    All,
    Pending,
    Blocked,
    AddFriend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HomeView {
    #[default]
    Friends,
    MessageRequests,
    Dm,
    Shop,
    Nitro,
    Quests,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InviteDialog {
    pub guild: Id,
    pub channel: Option<Id>,
    pub request: u64,
    pub link: Option<Result<SharedString, SharedString>>,
    pub copied: bool,
    pub sending: HashSet<Id>,
    pub sent: HashSet<Id>,
    pub send_error: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    JoinServer,
    InviteLink(InviteDialog),
    LeaveGuild(Id),
    Stub(SharedString),
    Topic(Id),
}

#[derive(Default)]
pub struct NavState {
    pub scroll_positions: HashMap<Id, ScrollPosition>,
    pub relationships: Vec<Relationship>,
    pub relationship_revision: u64,
    pub pending_friend_actions: HashSet<Id>,
    pub game_requests: Vec<(Id, Id)>,
    pub folders: Vec<GuildFolder>,
    pub sounds_seen: HashMap<Id, u64>,
    pub quest_onboarding_dismissed: bool,
    pub expanded_folders: HashSet<u64>,
    pub threads: HashMap<Id, Vec<Channel>>,
    pub extras: HashMap<Id, ChannelExtra>,
    pub activities: HashMap<Id, SharedString>,
    pub message_requests: Vec<Id>,
    pub spam_requests: Vec<Id>,
    pub requests_spam_tab: bool,
    pub home: HomeView,
    pub friends_tab: FriendsTab,
    pub guild_menu: bool,
    pub invite_request: u64,
    pub invite_friend_search: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub invite_friend_search_sub: Option<gpui::Subscription>,
    pub hide_muted: HashSet<Id>,
    pub overlay: Option<Overlay>,
    pub invite_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub add_friend_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub friends_search: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub invite_preview: Option<Result<InvitePreview, SharedString>>,
    pub invite_code: Option<String>,
    pub add_friend: crate::ui::home::add_friend::FormState,
    pub add_friend_note: Option<gpui::Entity<gpui_component::input::TextareaState>>,
    pub add_friend_reset: bool,
    pub revealed_notes: HashSet<Id>,
    pub tab_initialized: bool,
    pub add_friend_success_tag: Option<SharedString>,
    pub busy: bool,
    pub dragging: Option<Id>,
    pub prefs_loaded: bool,
    pub subs: Vec<gpui::Subscription>,
    pub banners: HashMap<Id, String>,
}

impl NavState {
    pub fn relationship(&self, user: Id) -> Option<&Relationship> {
        self.relationships.iter().find(|r| r.user == user)
    }

    pub fn is_blocked(&self, user: Id) -> bool {
        self.relationship(user).is_some_and(|r| r.kind == RelationshipKind::Blocked)
    }

    pub fn is_ignored(&self, user: Id) -> bool {
        self.relationship(user).is_some_and(|r| r.ignored && r.kind != RelationshipKind::Blocked)
    }

    pub fn pending_requests(&self) -> u32 {
        let skipped = |u: Id| self.relationships.iter().any(|r| r.user == u && (r.spam || r.ignored));
        let games = self.game_requests.iter().filter(|(u, _)| !skipped(*u)).count();
        (crate::ui::home::friends::pending_badge(&self.relationships) + games) as u32
    }

    pub fn count(&self, kind: RelationshipKind) -> usize {
        self.relationships.iter().filter(|r| r.kind == kind).count()
    }

    pub fn thread_parent(&self, id: Id) -> Option<Id> {
        self.threads.iter().find(|(_, v)| v.iter().any(|t| t.id == id)).map(|(p, _)| *p)
    }

    pub fn find_thread(&self, id: Id) -> Option<&Channel> {
        self.threads.values().flatten().find(|t| t.id == id)
    }

    pub fn replace_relationships(&mut self, relationships: Vec<Relationship>) {
        self.relationships = relationships;
        self.relationship_revision = self.relationship_revision.wrapping_add(1);
    }

    pub fn apply(&mut self, ev: NavEvent) {
        match ev {
            NavEvent::Snapshot { relationships, folders, threads, extras, message_requests, spam_requests } => {
                self.spam_requests = spam_requests;
                self.replace_relationships(relationships);
                self.folders = folders;
                self.threads.clear();
                for t in threads {
                    self.upsert_thread(t);
                }
                self.extras = extras.into_iter().collect();
                self.message_requests = message_requests;
                if !self.tab_initialized {
                    self.tab_initialized = true;
                    self.friends_tab = crate::ui::home::friends::initial_tab(&self.relationships);
                }
            }
            NavEvent::RelationshipUpsert(r) => {
                self.relationships.retain(|x| x.user != r.user);
                self.relationships.push(r);
                self.relationship_revision = self.relationship_revision.wrapping_add(1);
            }
            NavEvent::RelationshipRemove(u) => {
                self.relationships.retain(|x| x.user != u);
                self.relationship_revision = self.relationship_revision.wrapping_add(1);
            }
            NavEvent::GameRequests(g) => self.game_requests = g,
            NavEvent::GameRequest { user, app, incoming } => {
                self.game_requests.retain(|k| *k != (user, app));
                if incoming {
                    self.game_requests.push((user, app));
                }
            }
            NavEvent::Activity { user, activity } => match activity {
                Some(a) => {
                    self.activities.insert(user, a);
                }
                None => {
                    self.activities.remove(&user);
                }
            },
            NavEvent::ThreadUpsert(t) => self.upsert_thread(t),
            NavEvent::Folders(f) => self.folders = f,
            NavEvent::SoundsSeen(s) => self.sounds_seen.extend(s),
            NavEvent::QuestOnboardingDismissed(d) => self.quest_onboarding_dismissed = d,
            NavEvent::ThreadDelete(id) => {
                for v in self.threads.values_mut() {
                    v.retain(|t| t.id != id);
                }
            }
            NavEvent::Extras(e) => self.extras.extend(e),
            NavEvent::Banners(b) => {
                for (g, h) in b {
                    match h {
                        Some(h) => {
                            self.banners.insert(g, h);
                        }
                        None => {
                            self.banners.remove(&g);
                        }
                    }
                }
            }
        }
    }

    fn upsert_thread(&mut self, t: Channel) {
        for v in self.threads.values_mut() {
            v.retain(|x| x.id != t.id);
        }
        if let Some(p) = t.parent {
            let v = self.threads.entry(p).or_default();
            v.push(t);
            v.sort_by_key(|x| std::cmp::Reverse(x.id));
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct NavPrefs {
    pub last_channel: Vec<(Option<u64>, u64)>,
    pub scroll_positions: Vec<(Id, ScrollPosition)>,
    pub collapsed: Vec<u64>,
    pub folders: Vec<u64>,
    pub hide_muted: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollPosition {
    pub message: Id,
    pub offset: u32,
}

impl NavPrefs {
    pub fn path() -> Option<std::path::PathBuf> {
        crate::paths::config_file("navigation.txt")
    }

    pub fn parse(s: &str) -> Self {
        let mut p = Self::default();
        for line in s.lines() {
            let mut it = line.split_whitespace();
            let key = it.next().unwrap_or_default();
            let nums: Vec<u64> = it.filter_map(|x| x.parse().ok()).collect();
            match (key, nums.as_slice()) {
                ("last", [g, c]) => p.last_channel.push(((*g != 0).then_some(*g), *c)),
                ("scroll", [channel, message, offset]) if u32::try_from(*offset).is_ok() => {
                    p.scroll_positions.push((Id(*channel), ScrollPosition { message: Id(*message), offset: *offset as u32 }));
                }
                ("collapsed", ids) => p.collapsed.extend(ids),
                ("folders", ids) => p.folders.extend(ids),
                ("hide_muted", ids) => p.hide_muted.extend(ids),
                _ => {}
            }
        }
        p
    }

    pub fn serialize(&self) -> String {
        let join = |v: &[u64]| v.iter().map(u64::to_string).collect::<Vec<_>>().join(" ");
        let mut out = String::new();
        for (g, c) in &self.last_channel {
            out.push_str(&format!("last {} {c}\n", g.unwrap_or(0)));
        }
        for (channel, position) in &self.scroll_positions {
            out.push_str(&format!("scroll {} {} {}\n", channel.0, position.message.0, position.offset));
        }
        out.push_str(&format!("collapsed {}\n", join(&self.collapsed)));
        out.push_str(&format!("folders {}\n", join(&self.folders)));
        out.push_str(&format!("hide_muted {}\n", join(&self.hide_muted)));
        out
    }

    pub fn load() -> Self {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).map(|s| Self::parse(&s)).unwrap_or_default()
    }

    pub fn save(&self) {
        if phoenix_api::rest::guard::agent_session() {
            return;
        }
        let Some(p) = Self::path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, self.serialize());
    }
}

pub fn invite_code(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    if s.is_empty() {
        return None;
    }
    let s = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    let code = ["discord.gg/", "discord.com/invite/", "discordapp.com/invite/", "ptb.discord.com/invite/", "canary.discord.com/invite/"]
        .iter()
        .find_map(|p| s.strip_prefix(p))
        .unwrap_or(s);
    let code = code.split(['?', '#']).next().unwrap_or_default();
    (!code.is_empty() && code.len() <= 32 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_codes() {
        assert_eq!(invite_code("https://discord.gg/rust").as_deref(), Some("rust"));
        assert_eq!(invite_code("discord.com/invite/abc-DEF?event=1").as_deref(), Some("abc-DEF"));
        assert_eq!(invite_code("  xyz123 ").as_deref(), Some("xyz123"));
        assert_eq!(invite_code("hello world"), None);
        assert_eq!(invite_code(""), None);
    }

    #[test]
    fn prefs_roundtrip() {
        let p = NavPrefs {
            last_channel: vec![(None, 5), (Some(7), 9)],
            scroll_positions: vec![(Id(9), ScrollPosition { message: Id(42), offset: 13 })],
            collapsed: vec![1, 2],
            folders: vec![3],
            hide_muted: vec![],
        };
        assert_eq!(NavPrefs::parse(&p.serialize()), p);
    }

    #[test]
    fn threads_group_by_parent() {
        let mut s = NavState::default();
        let t = Channel {
            id: Id(10),
            guild: Some(Id(1)),
            parent: Some(Id(2)),
            name: "t".into(),
            topic: None,
            kind: crate::model::ChannelKind::Text,
            position: 0,
            unread: false,
            muted: false,
            mentions: 0,
            voice_users: vec![],
        };
        s.apply(NavEvent::ThreadUpsert(t));
        assert_eq!(s.thread_parent(Id(10)), Some(Id(2)));
        s.apply(NavEvent::ThreadDelete(Id(10)));
        assert!(s.threads[&Id(2)].is_empty());
    }
}
