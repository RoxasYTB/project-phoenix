use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{Local, TimeZone};
use futures::StreamExt;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    ListAlignment, ListState, ParentElement, Render, SharedString, Styled, Subscription, Window,
    actions, div, prelude::*, px,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    backend::{AppEvent, Backend, Credentials, EventSink, ReadyData},
    grouping,
    markdown::MentionResolver,
    model::*,
    richtext::{self, RenderedMessage},
    theme::{Palette, c},
    ui,
};

actions!(phoenix, [QuickSwitcher, Escape, EditLast, ToggleMembers]);

pub const CONTEXT: &str = "PhoenixApp";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-k", QuickSwitcher, Some(CONTEXT)),
        KeyBinding::new("ctrl-,", ui::settings::OpenSettings, Some(CONTEXT)),
        KeyBinding::new("escape", Escape, Some(CONTEXT)),
        KeyBinding::new("ctrl-u", ToggleMembers, Some(CONTEXT)),
    ]);
    crate::shortcuts::bind_keys(cx);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Main,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SidebarRow {
    Category(Id),
    Channel(Id),
    Thread(Id),
    VoiceUser { user: Id },
    StageAudience(Id),
}

#[derive(Clone, Debug)]
pub enum MemberRow {
    Header(SharedString),
    Member(usize),
}

const MAX_CACHED_CHANNELS: usize = 32;
const MAX_BG_CHANNEL_MSGS: usize = 200;

#[derive(Default)]
pub struct ChannelMessages {
    pub msgs: Vec<Message>,
    pub rendered: Vec<RenderedMessage>,
    pub loading: bool,
    pub loaded_once: bool,
    pub reached_start: bool,
    pub detached: bool,
    pub last_used: u64,
}

impl ChannelMessages {
    pub fn index_of(&self, id: Id) -> Option<usize> {
        match self.msgs.binary_search_by_key(&id.0, |m| m.id.0) {
            Ok(i) if self.msgs[i].id == id => Some(i),
            _ => self.msgs.iter().rposition(|m| m.id == id),
        }
    }
}

pub use crate::ui::switcher::Switcher;

pub struct PhoenixApp {
    pub backend: Arc<dyn Backend>,
    pub screen: Screen,
    pub focus: FocusHandle,
    pub(crate) profile_fx_clock: ui::profile_fx::PlaybackClock,

    pub login_email: Entity<InputState>,
    pub login_password: Entity<InputState>,
    pub login_token: Entity<InputState>,
    pub login_with_token: bool,
    pub login_mfa: Entity<InputState>,
    pub login_mfa_step: bool,
    pub login_error: Option<SharedString>,
    pub logging_in: bool,
    pub boot: crate::ui::boot::Boot,

    pub me: User,
    pub users: HashMap<Id, User>,
    pub guilds: Vec<Guild>,
    pub channels: HashMap<Id, Channel>,
    pub guild_channels: HashMap<Id, Vec<Id>>,
    pub dms: Vec<Id>,
    pub roles: HashMap<Id, Vec<Role>>,
    pub members: HashMap<Id, Vec<Member>>,
    pub pending_voice: HashMap<Id, Id>,
    pub pending_own_voice: Option<Id>,
    pub presences: HashMap<Id, Presence>,
    pub messages: HashMap<Id, ChannelMessages>,

    pub selected_guild: Option<Id>,
    pub selected_channel: Option<Id>,
    pub last_channel: HashMap<Option<Id>, Id>,
    pub collapsed: HashSet<Id>,
    pub favorites_view: bool,
    pub favorites_last: Option<Id>,
    pub favorites_seeded: HashSet<Id>,
    pub nav: crate::ui::home::data::NavState,

    pub sidebar_rows: Vec<SidebarRow>,
    pub sidebar_rows_guild: Option<Id>,
    pub member_rows: Vec<MemberRow>,

    pub guild_list: ListState,
    pub sidebar_list: ListState,
    pub message_list: ListState,
    pub member_list: ListState,

    pub composer: Entity<gpui_component::input::TextareaState>,
    pub replying_to: Option<Id>,
    pub editing: Option<Id>,
    pub typing: HashMap<Id, Vec<(Id, Instant)>>,
    pub typing_label: Option<SharedString>,
    pub typing_nicks: HashMap<(Id, Id), SharedString>,

    pub presence: Presence,
    pub mic_muted: bool,
    pub deafened: bool,
    pub voice_channel: Option<Id>,
    pending_entrance_sound: Option<Id>,
    pub clips: crate::clips::Clips,
    pub voice: crate::voice::VoiceUi,
    pub show_members: bool,
    pub people: crate::presence::People,

    pub switcher: Option<Switcher>,
    pub settings: Option<ui::settings::SettingsState>,
    pub prefs: ui::settings::Prefs,
    pub login_ui: crate::auth::LoginUi,
    pub chat: crate::ui::chat::ChatState,
    pub parity: crate::commands::ParityUi,
    pub revealed: HashSet<(Id, usize)>,
    pub utc_offset: i64,
    _subs: Vec<Subscription>,
}

pub(crate) struct Resolver<'a> {
    users: &'a HashMap<Id, User>,
    channels: &'a HashMap<Id, Channel>,
    roles: &'a HashMap<Id, Vec<Role>>,
    members: &'a HashMap<Id, Vec<Member>>,
    soundboard_sounds: &'a [crate::model::SoundboardSound],
    me: Id,
    guild: Option<Id>,
    guilds: &'a [Guild],
    nav: &'a crate::ui::home::data::NavState,
}
impl Resolver<'_> {
    fn find_role(&self, id: u64) -> Option<&Role> {
        self.roles.values().flatten().find(|r| r.id.0 == id)
    }
}
impl MentionResolver for Resolver<'_> {
    fn role(&self, id: u64) -> Option<String> {
        self.find_role(id).map(|r| r.name.to_string())
    }
    fn role_color(&self, id: u64) -> Option<u32> {
        self.find_role(id).and_then(|r| r.color)
    }
    fn has_role(&self, id: u64) -> bool {
        let Some((g, _)) = self.roles.iter().find(|(_, rs)| rs.iter().any(|r| r.id.0 == id)) else { return false };
        self.members
            .get(g)
            .and_then(|ms| ms.iter().find(|m| m.user == self.me))
            .is_some_and(|m| m.roles.contains(&Id(id)))
    }
    fn user(&self, id: u64) -> Option<String> {
        let nick = self
            .guild
            .and_then(|g| crate::member_index::find(self.members.get(&g)?, g, Id(id)))
            .and_then(|m| m.nick.as_ref().map(ToString::to_string));
        nick.or_else(|| self.users.get(&Id(id)).map(|u| u.display_name.to_string()))
    }
    fn channel(&self, id: u64) -> Option<String> {
        self.channels.get(&Id(id)).map(|c| c.name.to_string())
    }
    fn current_guild(&self) -> Option<u64> {
        self.guild.map(|g| g.0)
    }
    fn link_channel(&self, id: u64) -> Option<crate::markdown::LinkChannel> {
        let ch = self.channels.get(&Id(id)).or_else(|| self.nav.find_thread(Id(id)))?;
        let parent = ch.parent.and_then(|p| self.channels.get(&p)).filter(|p| p.kind != ChannelKind::Category);
        let thread = parent.is_some() || self.nav.thread_parent(ch.id).is_some();
        let icon = if thread {
            "threads"
        } else {
            match ch.kind {
                ChannelKind::Text | ChannelKind::Category => "hash",
                ChannelKind::Voice => "speaker",
                ChannelKind::Announcement => "megaphone",
                ChannelKind::Forum => "forum",
                ChannelKind::Stage => "stage",
            }
        };
        let dm = ch.guild.is_none();
        let name = if dm && ch.name.is_empty() {
            ch.voice_users.first().and_then(|u| self.users.get(u)).map(|u| u.display_name.to_string()).unwrap_or_default()
        } else {
            ch.name.to_string()
        };
        let forum_post = parent.filter(|p| p.kind == ChannelKind::Forum).map(|p| (p.name.to_string(), "forum"));
        Some(crate::markdown::LinkChannel { name, guild: ch.guild.map(|g| g.0), icon, dm, forum_post })
    }
    fn link_guild(&self, id: u64) -> Option<crate::markdown::LinkGuild> {
        let g = self.guilds.iter().find(|g| g.id.0 == id)?;
        let icon = match &g.icon {
            ImageRef::Url(u) => Some(crate::media::with_cdn_size(u, 32).to_string()),
            ImageRef::Default(_) => None,
        };
        Some(crate::markdown::LinkGuild { name: g.name.to_string(), icon, acronym: g.acronym.to_string() })
    }
    fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<crate::markdown::SoundboardMention> {
        let sound = self.soundboard_sounds.iter().find(|s| {
            s.id.0 == sound_id && s.guild.is_none_or(|guild| guild.0 == guild_id)
        })?;
        Some(crate::markdown::SoundboardMention {
            name: sound.name.as_ref().map(ToString::to_string).filter(|n| !n.is_empty()).unwrap_or_else(|| "Unknown".into()),
            emoji_name: sound.emoji_name.as_ref().map(ToString::to_string),
        })
    }
}

pub fn format_time_full(ts: i64) -> SharedString {
    let Some(dt) = Local.timestamp_opt(ts, 0).single() else { return "".into() };
    crate::i18n::calendar(dt, Local::now())
}

pub(crate) fn format_time_short(ts: i64) -> SharedString {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| crate::i18n::clock(d).into())
        .unwrap_or_default()
}

pub(crate) fn format_day(ts: i64) -> SharedString {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| crate::i18n::long_date(d.date_naive()).into())
        .unwrap_or_default()
}

impl PhoenixApp {
    pub fn new(backend: Arc<dyn Backend>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let login_email = cx.new(|cx| InputState::new(window, cx));
        let login_password = cx.new(|cx| InputState::new(window, cx).masked(true));
        let login_token = cx.new(|cx| InputState::new(window, cx).masked(true));
        let login_mfa = cx.new(|cx| InputState::new(window, cx).placeholder(crate::t!("login_mfa.code_placeholder")));
        let max_rows = ((f32::from(window.viewport_size().height) * 0.5 / 22.).floor() as usize).max(2);
        let composer = cx.new(|cx| gpui_component::input::TextareaState::new(window, cx).auto_grow(1, max_rows).submit_on_enter(true));

        let mut subs = Vec::new();
        subs.push(cx.observe_window_activation(window, |this, window, cx| {
            this.profile_fx_clock.set_active(window.is_window_active());
            cx.notify();
        }));
        subs.extend(ui::pickers::install(&composer, window, cx));
        subs.push(cx.subscribe_in(&composer, window, |this, _, ev: &InputEvent, window, cx| {
            match ev {
                InputEvent::PressEnter { shift: false, .. } if ui::chat::announce::enter_inserts_newline(this) => {
                    this.composer.update(cx, |s, cx| s.insert("\n", window, cx));
                }
                InputEvent::PressEnter { shift: false, .. } => this.submit_composer(window, cx),
                InputEvent::Change => this.on_composer_change(cx),
                _ => {}
            }
        }));
        subs.push(cx.observe(&composer, |this, area, cx| {
            if ui::chat::rich_input::enabled(this) {
                ui::chat::rich_input::snap_quote_caret(&area, cx);
            }
        }));
        subs.push(cx.subscribe_in(&login_mfa, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::Change = ev {
                crate::auth::on_code_change(this, window, cx);
            }
        }));
        for input in [&login_email, &login_password, &login_token, &login_mfa] {
            subs.push(cx.subscribe_in(input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.submit_login(window, cx);
                }
            }));
        }

        let (tx, mut rx) = futures::channel::mpsc::unbounded::<AppEvent>();
        let sink: EventSink = Arc::new(move |ev| {
            let _ = tx.unbounded_send(ev);
        });
        backend.set_event_sink(sink);
        cx.spawn(async move |this, cx| {
            while let Some(ev) = rx.next().await {
                let mut batch = vec![ev];
                while let Ok(ev) = rx.try_recv() {
                    batch.push(ev);
                }
                if this.update(cx, |this, cx| batch.into_iter().for_each(|ev| this.handle_event(ev, cx))).is_err() {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |this, cx| { this.expire_typing(cx); crate::ui::burst::tick(cx); if crate::voice::start_time::ticking() || crate::ui::chat::composer::cooling() { cx.notify(); } }).is_err() {
                    break;
                }
            }
        })
        .detach();

        if !crate::paths::is_sandboxed() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    let include_beta = this.prefs.local.beta_channel;
                    crate::updater::spawn_check(include_beta, cx);
                });
            })
            .detach();
        }

        let message_list = ListState::new(0, ListAlignment::Bottom, px(800.));
        let message_list = if ui::chat::rows::measure_everything() { message_list.measure_all() } else { message_list };
        let weak = cx.entity().downgrade();
        message_list.set_scroll_handler(move |ev: &gpui::ListScrollEvent, _w, cx: &mut App| {
            if ev.visible_range.start == 0 {
                let _ = weak.update(cx, |this, cx| this.load_older(cx));
            }
            let _ = weak.update(cx, |this, cx| this.detached_on_scroll(ev, cx));
            let _ = weak.update(cx, |this, cx| this.chat_on_scroll(ev, cx));
        });

        let restore = backend.restore_session();
        let logging_in = restore.is_some();
        if let Some(fut) = restore.is_some().then(|| backend.restore_cached()).flatten() {
            cx.spawn(async move |this, cx| {
                if let Ok(ready) = fut.await {
                    let _ = this.update(cx, |this, cx| {
                        if this.logging_in && this.screen == Screen::Login {
                            let t0 = std::time::Instant::now();
                            this.apply_ready(ready, cx);
                            crate::perf::span("snapshot_applied", t0);
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        if let Some(fut) = restore {
            cx.spawn(async move |this, cx| {
                let res = fut.await;
                let _ = this.update(cx, |this, cx| {
                    let t0 = std::time::Instant::now();
                    let live = res.is_ok();
                    this.logging_in = false;
                    match res {
                        Ok(ready) if this.screen == Screen::Main => {
                            crate::commands::on_resync(this, crate::commands::Resync(ready), cx)
                        }
                        Ok(ready) => this.apply_ready(ready, cx),
                        Err(e) => {
                            if this.screen == Screen::Main {
                                this.clear_session();
                                this.screen = Screen::Login;
                            }
                            if !e.0.is_empty() {
                                this.login_error = Some(e.0.into());
                            }
                        }
                    }
                    crate::perf::span("ready_applied", t0);
                    if live {
                        crate::history_ui::warm_recent(this, cx);
                    }
                    cx.notify();
                });
            })
            .detach();
        }

        let utc_offset = Local::now().offset().local_minus_utc() as i64;
        let (self_mute, self_deaf) = {
            let p = crate::voice::prefs::prefs().lock();
            (p.self_mute, p.self_deaf)
        };
        Self {
            backend,
            profile_fx_clock: ui::profile_fx::PlaybackClock::new(window.is_window_active()),
            screen: Screen::Login,
            focus: cx.focus_handle(),
            login_email,
            login_password,
            login_token,
            login_with_token: false,
            login_mfa,
            login_mfa_step: false,
            login_error: None,
            logging_in,
            boot: Default::default(),
            me: User {
                id: Id(0),
                name: "".into(),
                display_name: "".into(),
                avatar: ImageRef::Default(0),
                bot: false,
                verified_bot: false,
                flags: 0,
                tag: None,
                tag_guild: None,
                nameplate: None,
                name_style: None,
                decoration: None,
            },
            users: HashMap::new(),
            guilds: Vec::new(),
            channels: HashMap::new(),
            guild_channels: HashMap::new(),
            dms: Vec::new(),
            roles: HashMap::new(),
            members: HashMap::new(),
            pending_voice: HashMap::new(),
            pending_own_voice: None,
            presences: HashMap::new(),
            messages: HashMap::new(),
            selected_guild: None,
            selected_channel: None,
            last_channel: HashMap::new(),
            collapsed: HashSet::new(),
            favorites_view: false,
            favorites_last: None,
            favorites_seeded: HashSet::new(),
            nav: Default::default(),
            sidebar_rows: Vec::new(),
            sidebar_rows_guild: None,
            member_rows: Vec::new(),
            guild_list: ListState::new(0, ListAlignment::Top, px(200.)),
            sidebar_list: ListState::new(0, ListAlignment::Top, px(300.)),
            message_list,
            member_list: ListState::new(0, ListAlignment::Top, px(300.)),
            composer,
            replying_to: None,
            editing: None,
            typing: HashMap::new(),
            typing_label: None,
            typing_nicks: HashMap::new(),
            presence: Presence::Online,
            mic_muted: self_mute,
            deafened: self_deaf,
            voice_channel: None,
            pending_entrance_sound: None,
            clips: Default::default(),
            voice: Default::default(),
            show_members: true,
            people: Default::default(),
            switcher: None,
            chat: Default::default(),
            parity: Default::default(),
            revealed: HashSet::new(),
            utc_offset,
            settings: None,
            prefs: ui::settings::Prefs::load(),
            login_ui: Default::default(),
            _subs: subs,
        }
    }

    pub fn submit_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.logging_in {
            return;
        }
        if self.login_mfa_step && !self.login_with_token {
            return crate::auth::submit_mfa(self, cx);
        }
        let creds = if self.login_with_token {
            Credentials::Token(self.login_token.read(cx).value().to_string())
        } else {
            Credentials::Password {
                login: self.login_email.read(cx).value().to_string(),
                password: self.login_password.read(cx).value().to_string(),
            }
        };
        self.logging_in = true;
        self.login_error = None;
        cx.notify();
        let fut = self.backend.login(creds);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.logging_in = false;
                match res {
                    Ok(ready) => {
                        this.login_mfa_step = false;
                        this.apply_ready(ready, cx)
                    }
                    Err(_) if this.backend.needs_mfa() => crate::auth::start_mfa(this, cx),
                    Err(e) => this.login_error = Some(e.0.into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn clear_session(&mut self) {
        self.clips.stop();
        self.users.clear();
        self.guilds.clear();
        self.channels.clear();
        self.guild_channels.clear();
        self.dms.clear();
        self.roles.clear();
        self.members.clear();
        self.people.clear();
        self.presences.clear();
        self.messages.clear();
        self.selected_guild = None;
        self.chat.create_thread = None;
        self.selected_channel = None;
        self.last_channel.clear();
        self.nav.scroll_positions.clear();
        self.nav.prefs_loaded = false;
        self.sidebar_rows.clear();
        self.member_rows.clear();
        self.typing.clear();
        self.typing_label = None;
        self.replying_to = None;
        self.editing = None;
        self.voice_channel = None;
        self.switcher = None;
        self.chat = Default::default();
        self.guild_list.reset(0);
        self.sidebar_list.reset(0);
        self.message_list.reset(0);
        self.member_list.reset(0);
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        crate::ui::game_overlay::close(cx);
        self.persist_chat_position();
        let fut = self.backend.logout();
        cx.spawn(async move |_, _| {
            let _ = fut.await;
        })
        .detach();
        self.clear_session();
        self.nav.relationships.clear();
        self.nav.pending_friend_actions.clear();
        self.nav.relationship_revision = self.nav.relationship_revision.wrapping_add(1);
        self.login_mfa_step = false;
        self.login_ui.mfa = Default::default();
        self.login_error = None;
        self.screen = Screen::Login;
        cx.notify();
    }

    pub fn apply_ready(&mut self, r: ReadyData, cx: &mut Context<Self>) {
        self.persist_chat_position();
        let mine = self.people.activities.remove(&r.me.id);
        let known_presences = std::mem::take(&mut self.presences);
        let known_activities = std::mem::take(&mut self.people.activities);
        let old_voice: HashMap<Id, Vec<Id>> = self
            .channels
            .values()
            .filter(|c| c.guild.is_some() && !c.voice_users.is_empty())
            .map(|c| (c.id, c.voice_users.clone()))
            .collect();
        let in_call = self.voice_channel;
        self.clear_session();
        if let Some(a) = mine {
            self.people.custom_status = a.iter().find(|x| x.kind == crate::presence::ActivityKind::Custom).and_then(|x| x.state.clone());
            self.people.activities.insert(r.me.id, a);
        }
        let mine_status = known_presences.get(&r.me.id).copied();
        self.presences = known_presences;
        self.presences.extend(r.presences);
        for (u, a) in known_activities {
            self.people.activities.entry(u).or_insert(a);
        }
        self.me = r.me;
        if let Some(p) = mine_status {
            self.presence = p;
        }
        self.users = r.users.into_iter().map(|u| (u.id, u)).collect();
        self.guilds = r.guilds;
        for (g, roles) in r.roles {
            self.roles.insert(g, roles);
        }
        for mut ch in r.channels {
            if let Some(g) = ch.guild {
                self.guild_channels.entry(g).or_default().push(ch.id);
            }
            if ch.voice_users.is_empty()
                && ch.guild.is_some()
                && let Some(old) = old_voice.get(&ch.id)
            {
                ch.voice_users = old.clone();
            }
            if ch.guild.is_some() {
                for (u, _) in self.pending_voice.iter().filter(|(_, c)| **c == ch.id) {
                    if !ch.voice_users.contains(u) {
                        ch.voice_users.push(*u);
                    }
                }
            }
            self.channels.insert(ch.id, ch);
        }
        for list in self.guild_channels.values_mut() {
            list.sort_by_key(|id| self.channels[id].position);
        }
        self.voice_channel = self.pending_own_voice.take().or(in_call);
        crate::voice::roster::reapply(self);
        if self.voice_channel.is_some() {
            crate::ui::voice_effects::prewarm();
        }
        for ch in r.dm_channels {
            if ch.guild.is_none() && !self.channels.contains_key(&ch.id) {
                self.dms.push(ch.id);
                self.channels.insert(ch.id, ch);
            }
        }
        self.guild_list.reset(self.guilds.len() + 5);
        self.screen = Screen::Main;
        self.nav_after_ready();
        self.refresh_relationships(cx);
        let first = self.guilds.first().map(|g| g.id);
        self.select_guild(first, cx);
        ui::settings::on_ready(self, cx);
    }

    pub fn select_guild(&mut self, guild: Option<Id>, cx: &mut Context<Self>) {
        crate::ui::discovery::close();
        crate::memory::release_soon();
        self.selected_guild = guild;
        self.favorites_view = false;
        self.rebuild_sidebar();
        self.rebuild_members();
        self.backend.select_guild(guild).detach_ok();
        let target = self.guild_target(guild);
        self.nav_home_target(guild, target.is_some());
        if let Some(t) = target {
            self.open_channel_chat(t, cx);
        } else {
            self.remember_chat_position();
            self.chat.create_thread = None;
            self.selected_channel = None;
            self.message_list.reset(0);
            self.save_nav_prefs();
        }
        cx.notify();
    }

    pub(crate) fn guild_target(&self, guild: Option<Id>) -> Option<Id> {
        if let Some(g) = guild.filter(|g| ui::join_flow::is_lurking(*g)) {
            return self.guild_channels.get(&g)?.iter().copied().filter(|c| !self.is_trap_channel(*c)).find(|id| {
                self.channels.get(id).is_some_and(|c| matches!(c.kind, ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum))
            });
        }
        self.last_channel.get(&guild).copied().filter(|id| !self.is_trap_channel(*id) && self.channels.get(id).is_some_and(|c| c.guild == guild)).or_else(|| match guild {
            Some(g) => self.guild_channels.get(&g).and_then(|l| {
                let open = |id: &Id| !self.is_trap_channel(*id);
                l.iter().copied().filter(open).find(|id| {
                    matches!(self.channels[id].kind, ChannelKind::Text | ChannelKind::Announcement)
                        && matches!(self.channels[id].name.to_lowercase().as_str(), "general" | "général")
                })
                .or_else(|| {
                    l.iter().copied().filter(open).find(|id| self.channels[id].kind == ChannelKind::Text)
                })
                .or_else(|| {
                    l.iter().copied().filter(open).find(|id| matches!(self.channels[id].kind, ChannelKind::Announcement | ChannelKind::Forum))
                })
            }),
            None => None,
        })
    }

    pub fn is_trap_channel(&self, id: Id) -> bool {
        phoenix_api::trap::is_trap(id.0)
            || self.channels.get(&id).is_some_and(|c| phoenix_api::trap::trap_text(&c.name, c.topic.as_deref()))
    }

    pub(crate) fn refuse_trap_in_agent_session(&self, id: Id) {
        if phoenix_api::rest::guard::agent_session() && self.is_trap_channel(id) {
            let msg = format!("agent session tried to open the trap channel {}; stopping before anything is sent", id.0);
            tracing::error!("{msg}");
            eprintln!("{msg}");
            std::process::exit(70);
        }
    }

    fn refresh_guild_mentions(&mut self, guild: Option<Id>) {
        let Some(g) = guild else { return };
        let n = self.guild_channels.get(&g).map_or(0, |l| l.iter().filter_map(|c| self.channels.get(c)).map(|c| c.mentions).sum());
        if let Some(gg) = self.guilds.iter_mut().find(|x| x.id == g) {
            gg.mentions = n;
        }
    }

    pub fn select_channel(&mut self, id: Id, cx: &mut Context<Self>) {
        self.refuse_trap_in_agent_session(id);
        crate::memory::release_soon();
        let Some(ch) = self.channels.get(&id) else { return };
        if matches!(ch.kind, ChannelKind::Voice | ChannelKind::Stage) {
            if ch.guild.is_some_and(ui::join_flow::is_lurking) {
                self.open_channel_chat(id, cx);
                return;
            }
            if self.voice_switch_needs_confirm(id) {
                self.confirm_voice_switch(id, cx);
                return;
            }
            self.enter_voice_channel(id, cx);
            return;
        }
        self.open_channel_chat(id, cx);
    }

    pub(crate) fn enter_voice_channel(&mut self, id: Id, cx: &mut Context<Self>) {
        self.join_voice(Some(id), cx);
        self.open_channel_chat(id, cx);
        crate::ui::call_view::show(id);
    }

    pub fn voice_switch_needs_confirm(&self, target: Id) -> bool {
        let Some(cur) = self.voice_channel else { return false };
        if cur == target || !self.prefs.local.flag("vo-switch-warn", true) {
            return false;
        }
        let guild = self.channels.get(&target).and_then(|c| c.guild);
        let afk = guild.and_then(|g| self.guilds.iter().find(|x| x.id == g)).and_then(|g| g.afk_channel);
        afk != Some(cur)
    }

    fn confirm_voice_switch(&mut self, id: Id, cx: &mut Context<Self>) {
        crate::actions::open_modal(ui::modals::Modal::VoiceSwitch(ui::modals::voice_pick::VoiceSwitch { channel: id, dont_ask: false }), None);
        cx.notify();
    }

    pub fn open_channel_chat(&mut self, id: Id, cx: &mut Context<Self>) {
        self.refuse_trap_in_agent_session(id);
        self.remember_chat_position();
        crate::ui::discovery::close();
        crate::ui::favorites::on_channel_opened(self, id);
        crate::commands::state::remember_read_mark(self, id);
        let Some(ch) = self.channels.get_mut(&id) else { return };
        ch.unread = false;
        ch.mentions = 0;
        let guild = ch.guild;
        self.refresh_guild_mentions(guild);
        if guild != self.selected_guild {
            self.selected_guild = guild;
            self.rebuild_sidebar();
            self.rebuild_members();
        }
        if self.selected_channel != Some(id) {
            self.chat.create_thread = None;
        }
        self.selected_channel = Some(id);
        crate::notifications::clear(id.0);
        crate::ui::chat::voice_message::on_channel_change(self, cx);
        if !guild.is_some_and(ui::join_flow::is_lurking) {
            self.last_channel.insert(guild, id);
            self.nav_channel_selected(guild);
        }
        self.replying_to = None;
        self.editing = None;
        self.chat_on_select(id);
        self.update_typing_label();
        self.backend.select_channel(id).detach_ok();
        let tick = self.messages.values().map(|e| e.last_used).max().unwrap_or(0) + 1;
        let entry = self.messages.entry(id).or_default();
        entry.last_used = tick;
        self.message_list.reset(entry.msgs.len() + 1);
        if self.messages.len() > MAX_CACHED_CHANNELS {
            let evict = self
                .messages
                .iter()
                .filter(|(k, e)| **k != id && !e.loading)
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| *k);
            if let Some(old) = evict {
                self.messages.remove(&old);
            }
        }
        let entry = self.messages.entry(id).or_default();
        crate::perf::channel_opened(id.0);
        let cached = entry.loaded_once;
        let now = chrono::Utc::now().timestamp();
        let exp = |u: &SharedString| crate::media::signed_url_expired(u, now);
        let stale = cached
            && entry.msgs.iter().rev().take(50).any(|m| {
                m.attachments.iter().any(|a| exp(&a.url) || exp(&a.original))
                    || m.embeds.iter().any(|e| e.thumbnail.iter().chain(&e.image).chain(&e.images).chain(&e.video).any(|x| exp(&x.url)))
            });
        if cached {
            crate::perf::channel_messages_ready(id.0, "memory");
        }
        if !entry.loaded_once && !entry.loading {
            entry.loading = true;
            crate::history_ui::load_first_page(self, id, cx);
        }
        if cached {
            crate::commands::state::apply_read_mark(self, id);
            self.restore_chat_position(id, cx);
            if stale {
                self.backend.prefetch_messages(id);
            }
        }
        cx.notify();
    }

    pub fn load_older(&mut self, cx: &mut Context<Self>) {
        crate::history_ui::load_older(self, cx);
    }

    pub(crate) fn resolver(&self) -> Resolver<'_> {
        Resolver { users: &self.users, channels: &self.channels, roles: &self.roles, members: &self.members, soundboard_sounds: &[], me: self.me.id, guild: self.selected_guild, guilds: &self.guilds, nav: &self.nav }
    }

    pub(crate) fn resolver_with_sounds<'a>(&'a self, sounds: &'a [crate::model::SoundboardSound], channel: Id) -> Resolver<'a> {
        let guild = self.channels.get(&channel).or_else(|| self.nav.find_thread(channel)).and_then(|c| c.guild).or(self.selected_guild);
        Resolver { users: &self.users, channels: &self.channels, roles: &self.roles, members: &self.members, soundboard_sounds: sounds, me: self.me.id, guild, guilds: &self.guilds, nav: &self.nav }
    }

    pub(crate) fn render_msg(&self, m: &Message, prev: Option<&Message>) -> RenderedMessage {
        let revealed = |i: usize| self.revealed.contains(&(m.id, i));
        let (parsed, mut blocks, mentions_me, preview, preview_mentions) =
            richtext::build(&m.content, &self.resolver_with_sounds(&m.soundboard_sounds, m.channel), self.me.id, &revealed);
        if m.send_state == crate::model::SendState::Failed {
            blocks = richtext::render_blocks(&parsed, &revealed, ui::chat::tk(crate::theme::generated::idx::TEXT_FEEDBACK_CRITICAL));
        }
        RenderedMessage {
            group_start: grouping::starts_group(prev, m, self.utc_offset),
            day_divider: (prev.is_none() || grouping::day_divider(prev, m, self.utc_offset))
                .then(|| format_day(m.timestamp)),
            time_full: format_time_full(m.timestamp),
            time_short: format_time_short(m.timestamp),
            blocks,
            mentions_me,
            preview,
            preview_mentions,
            parsed,
        }
    }

    pub(crate) fn insert_history(&mut self, ch: Id, page: Vec<Message>, older: bool, cx: &mut Context<Self>) {
        if !older {
            crate::perf::channel_messages_ready(ch.0, "load");
        }
        let mut entry = self.messages.remove(&ch).unwrap_or_default();
        entry.loading = false;
        entry.loaded_once = true;
        if page.len() < 50 {
            entry.reached_start = true;
        }
        let n = page.len();
        let mut all = page;
        if older {
            all.extend(std::mem::take(&mut entry.msgs));
        }
        let rendered: Vec<RenderedMessage> = if older {
            let mut r: Vec<_> = (0..n.min(all.len()))
                .map(|i| self.render_msg(&all[i], i.checked_sub(1).map(|p| &all[p])))
                .collect();
            let mut old = std::mem::take(&mut entry.rendered);
            if let Some(first) = old.first_mut()
                && n < all.len()
            {
                *first = self.render_msg(&all[n], n.checked_sub(1).map(|p| &all[p]));
            }
            r.extend(old);
            r
        } else {
            (0..all.len())
                .map(|i| self.render_msg(&all[i], i.checked_sub(1).map(|p| &all[p])))
                .collect()
        };
        entry.msgs = all;
        entry.rendered = rendered;
        let len = entry.msgs.len();
        self.messages.insert(ch, entry);
        if self.selected_channel == Some(ch) {
            if older {
                ui::chat::rows::prepend(&self.message_list, ch, n, len);
            } else {
                self.message_list.reset(len + 1);
            }
        }
        self.resume_pending_jump(ch, cx);
        cx.notify();
    }

    pub(crate) fn append_message(&mut self, m: Message, cx: &mut Context<Self>) {
        crate::tts::on_message(self, &m);
        if self.take_echo(&m, cx) {
            return;
        }
        ui::chat::welcome::record_message(self, &m);
        let ch = m.channel;
        let selected = self.selected_channel == Some(ch);
        let mention = m.content.contains(&format!("<@{}>", self.me.id.0)) && !self.nav.is_blocked(m.author);
        if let Some(t) = self.typing.get_mut(&ch) {
            t.retain(|(u, _)| *u != m.author);
        }
        if self.messages.get(&ch).is_some_and(|e| e.detached) {
            if selected {
                self.chat_note_incoming(false, m.timestamp);
            }
            cx.notify();
            return;
        }
        let Some(mut entry) = self.messages.remove(&ch) else {
            if let Some(c) = self.channels.get_mut(&ch) {
                c.unread = true;
                if mention {
                    c.mentions += 1;
                }
            }
            return;
        };
        if entry.msgs.last().is_some_and(|l| l.id.0 >= m.id.0) && entry.index_of(m.id).is_some() {
            self.messages.insert(ch, entry);
            return;
        }
        let r = self.render_msg(&m, entry.msgs.last());
        let (own, ts) = (m.author == self.me.id, m.timestamp);
        entry.msgs.push(m);
        entry.rendered.push(r);
        if !selected && entry.msgs.len() > MAX_BG_CHANNEL_MSGS {
            let drop_n = entry.msgs.len() - MAX_BG_CHANNEL_MSGS;
            entry.msgs.drain(..drop_n);
            entry.rendered.drain(..drop_n);
            entry.reached_start = false;
        }
        let len = entry.msgs.len();
        self.messages.insert(ch, entry);
        if selected {
            ui::chat::rows::insert(&self.message_list, len, 1);
            self.chat_note_incoming(own, ts);
        } else if let Some(c) = self.channels.get_mut(&ch) {
            c.unread = true;
        }
        self.update_typing_label();
        cx.notify();
    }

    pub(crate) fn replace_message(&mut self, m: Message, cx: &mut Context<Self>) {
        let ch = m.channel;
        let Some(mut entry) = self.messages.remove(&ch) else { return };
        if let Some(ix) = entry.index_of(m.id) {
            entry.rendered[ix] = self.render_msg(&m, ix.checked_sub(1).map(|p| &entry.msgs[p]));
            entry.msgs[ix] = m;
            if self.selected_channel == Some(ch) {
                ui::chat::rows::refresh(&self.message_list, ix + 1);
            }
        }
        self.messages.insert(ch, entry);
        cx.notify();
    }

    pub(crate) fn rerender_mentions_of(&mut self, users: &[u64]) {
        if users.is_empty() {
            return;
        }
        let pats: Vec<String> = users.iter().flat_map(|u| [format!("<@{u}>"), format!("<@!{u}>")]).collect();
        let chans: Vec<Id> = self.messages.keys().copied().collect();
        for ch in chans {
            let Some(mut entry) = self.messages.remove(&ch) else { continue };
            for ix in 0..entry.msgs.len().min(entry.rendered.len()) {
                if pats.iter().any(|p| entry.msgs[ix].content.contains(p.as_str())) {
                    entry.rendered[ix] = self.render_msg(&entry.msgs[ix], ix.checked_sub(1).map(|p| &entry.msgs[p]));
                    if self.selected_channel == Some(ch) {
                        ui::chat::rows::refresh(&self.message_list, ix + 1);
                    }
                }
            }
            self.messages.insert(ch, entry);
        }
    }

    pub fn reveal_spoiler(&mut self, msg: Id, spoiler: usize, cx: &mut Context<Self>) {
        self.revealed.insert((msg, spoiler));
        let Some(ch) = self.selected_channel else { return };
        if let Some(m) = self
            .messages
            .get(&ch)
            .and_then(|e| e.msgs.iter().find(|x| x.id == msg).cloned())
        {
            self.replace_message(m, cx);
        }
    }

    fn announce_mention(&self, channel: Id) {
        let Some(ch) = self.channels.get(&channel) else { return };
        let dm = ch.guild.is_none();
        let group = self.nav.extras.get(&channel).is_some_and(|e| e.recipients > 1 || e.group_icon.is_some());
        let info = ui::a11y::ChannelInfo { unread: ch.unread, mentions: ch.mentions.max(1), ..Default::default() };
        ui::a11y::announce(ui::a11y::channel(ui::a11y::Kind::of(ch.kind, dm, group), &ch.name, &info), false);
    }

    pub fn handle_event(&mut self, ev: AppEvent, cx: &mut Context<Self>) {
        match ev {
            AppEvent::Ready => {
                if cx.has_global::<crate::ui::pickers::Pickers>() {
                    crate::ui::pickers::load_frecency(self, cx);
                }
            }
            AppEvent::MessageCreate(m) => {
                ui::call_toasts::on_message(self, &m, cx);
                let me = self.me.id.0;
                let pings = m.mention_everyone || m.content.contains(&format!("<@{me}>")) || m.content.contains(&format!("<@!{me}>"));
                let mention = (m.author != self.me.id && self.selected_channel != Some(m.channel) && !self.nav.is_blocked(m.author)).then_some((m.channel, pings));
                self.append_message(m, cx);
                if let Some((channel, true)) = mention.filter(|_| ui::a11y::on()) {
                    self.announce_mention(channel);
                }
            }
            AppEvent::MessageUpdate(m) => self.replace_message(m, cx),
            AppEvent::BurstReaction { channel, msg, emoji, emoji_id, colors } => {
                if let Some(p) = ui::burst::palette(&colors, !crate::theme::is_light()) {
                    ui::burst::play(ui::burst::Play { channel, msg, token: &emoji, emoji_id, accent: p.accent, kind: ui::burst::Kind::External }, cx);
                }
            }
            AppEvent::HistoryRefresh { channel, messages } => crate::history_ui::on_refresh(self, channel, messages, cx),
            AppEvent::MessageDelete { channel, id } => {
                crate::tts::on_message_delete(channel, id);
                if let Some(mut e) = self.messages.remove(&channel) {
                    if let Some(ix) = e.index_of(id) {
                        e.msgs.remove(ix);
                        e.rendered.remove(ix);
                        let selected = self.selected_channel == Some(channel);
                        if selected {
                            self.message_list.splice(ix + 1..ix + 2, 0);
                        }
                        if ix < e.msgs.len() {
                            e.rendered[ix] = self.render_msg(&e.msgs[ix], ix.checked_sub(1).map(|p| &e.msgs[p]));
                            if selected {
                                ui::chat::rows::refresh(&self.message_list, ix + 1);
                            }
                        }
                    }
                    self.messages.insert(channel, e);
                }
            }
            AppEvent::TypingStart { channel, user, nick } => {
                if user != self.me.id {
                    match nick {
                        Some(n) => { self.typing_nicks.insert((channel, user), n); }
                        None => { self.typing_nicks.remove(&(channel, user)); }
                    }
                    let list = self.typing.entry(channel).or_default();
                    list.retain(|(u, _)| *u != user);
                    list.push((user, Instant::now()));
                    self.update_typing_label();
                }
            }
            AppEvent::PresenceUpdate { user, presence } => {
                if user == self.me.id {
                    self.presence = presence;
                }
                if user == self.me.id {
                    self.presence = presence;
                }
                self.people.on_presence(user, presence);
                self.presences.insert(user, presence);
                for ms in self.members.values_mut() {
                    for m in ms.iter_mut().filter(|m| m.user == user) {
                        m.presence = presence;
                    }
                }
                self.rebuild_members();
            }
            AppEvent::GuildMembers { guild, members } => {
                self.members.insert(guild, members);
                crate::member_index::invalidate(guild);
                if self.selected_guild == Some(guild) {
                    self.rebuild_members();
                    self.rebuild_sidebar();
                }
            }
            AppEvent::ChannelUnread { channel, unread, mentions } => {
                if let Some(c) = self.channels.get_mut(&channel) {
                    c.unread = unread;
                    c.mentions = mentions;
                }
                self.rebuild_sidebar();
            }
            AppEvent::VoiceFlags { user, flags } => crate::ui::voice_panel::set_flags(user, flags),
            AppEvent::GuildVoiceReset(guild) => {
                for ch in self.channels.values_mut().filter(|c| c.guild == Some(guild)) {
                    ch.voice_users.retain(|u| crate::voice::roster::is_net(*u));
                }
                if self.selected_guild == Some(guild) {
                    self.rebuild_sidebar();
                }
            }
            AppEvent::VoiceStateUpdate { guild, channel, user, own } => {
                self.apply_voice_state(guild, channel, user, own);
                if own && user == self.me.id {
                    self.maybe_play_entrance_sound(cx);
                }
                self.rebuild_sidebar();
            }
            AppEvent::Voice(e) => {
                if let crate::voice::VoiceUiEvent::Stream(crate::voice::StreamUiEvent::Preview { key, thumbnail }) = &e
                    && self.prefs.synced.flag("vo-stream-preview")
                {
                    let fut = self.backend.upload_stream_preview(key.clone(), thumbnail.clone());
                    let key = key.clone();
                    std::thread::spawn(move || {
                        let ok = futures::executor::block_on(fut).is_ok();
                        crate::voice::call::preview_uploaded(&key, ok);
                    });
                }
                match &e {
                    crate::voice::VoiceUiEvent::SpeakingFlags { user, flags } => {
                        crate::voice::priority::on_flags(self, *user, *flags);
                        if crate::voice::roster::on_clients(self, &[*user], true) {
                            self.rebuild_sidebar();
                        }
                    }
                    crate::voice::VoiceUiEvent::Clients { users, connected } => {
                        if crate::voice::roster::on_clients(self, users, *connected) {
                            self.rebuild_sidebar();
                        }
                    }
                    crate::voice::VoiceUiEvent::CameraFailed(f) => crate::ui::voice_panel::media_failure_modal(f, false),
                    crate::voice::VoiceUiEvent::ScreenFailed(f) => crate::ui::voice_panel::media_failure_modal(f, true),
                    _ => {}
                }
                let connected = matches!(e, crate::voice::VoiceUiEvent::Status(crate::voice::VoiceStatus::Connected));
                self.voice.apply(e);
                if connected {
                    self.maybe_play_entrance_sound(cx);
                }
            }
            AppEvent::Users(users) => {
                let fresh: Vec<u64> = users.iter().filter(|u| !self.users.contains_key(&u.id)).map(|u| u.id.0).collect();
                for u in users {
                    self.users.insert(u.id, u);
                }
                self.rerender_mentions_of(&fresh);
            }
            AppEvent::GuildChannels { guild, channels } => {
                self.set_guild_channels(guild, channels);
                self.refresh_selected_guild_channels(guild, cx);
            }
            AppEvent::GuildRoles { guild, roles } => {
                self.roles.insert(guild, roles);
                if self.selected_guild == Some(guild) {
                    self.rebuild_members();
                }
            }
            AppEvent::GuildUnread { guild, unread, mentions } => {
                if let Some(g) = self.guilds.iter_mut().find(|g| g.id == guild) {
                    g.unread = unread;
                    g.mentions = mentions;
                }
            }
            AppEvent::GuildCreate { guild, channels, roles } => {
                let gid = guild.id;
                match self.guilds.iter_mut().find(|g| g.id == gid) {
                    Some(g) => *g = guild,
                    None => {
                        self.guilds.insert(0, guild);
                        self.guild_list.reset(self.guilds.len() + 5);
                    }
                }
                self.roles.insert(gid, roles);
                self.set_guild_channels(gid, channels);
                self.refresh_selected_guild_channels(gid, cx);
            }
            AppEvent::GuildDelete(gid) => {
                let was_preview = ui::join_flow::is_lurking(gid);
                ui::join_flow::clear_lurking(gid);
                self.guilds.retain(|g| g.id != gid);
                self.guild_list.reset(self.guilds.len() + 5);
                self.set_guild_channels(gid, Vec::new());
                self.guild_channels.remove(&gid);
                self.members.remove(&gid);
                if self.selected_guild == Some(gid) {
                    self.select_guild(None, cx);
                    if was_preview {
                        ui::discovery::open(ui::discovery::Tab::Servers);
                    }
                }
            }
            AppEvent::DmUpsert(ch) => {
                let id = ch.id;
                if ch.guild.is_some() || self.channels.get(&id).is_some_and(|existing| existing.guild.is_some()) {
                    return;
                }
                let keep_unread = self.selected_channel == Some(id);
                let mut ch = ch;
                if keep_unread {
                    ch.unread = false;
                    ch.mentions = 0;
                }
                self.channels.insert(id, ch);
                self.dms.retain(|d| *d != id);
                self.dms.insert(0, id);
                if self.selected_guild.is_none() {
                    self.rebuild_sidebar();
                }
            }
            AppEvent::DmDelete(id) => {
                if self.channels.get(&id).is_some_and(|channel| channel.guild.is_some()) {
                    return;
                }
                self.dms.retain(|d| *d != id);
                self.channels.remove(&id);
                if self.selected_guild.is_none() {
                    self.rebuild_sidebar();
                }
            }
            AppEvent::Nav(e) => self.on_nav_event(e, cx),
            AppEvent::SettingsChanged => ui::settings::on_ready(self, cx),
            AppEvent::SoundboardSounds { guild, sounds } => ui::pickers::soundboard::on_sounds(self, guild, sounds, cx),
            AppEvent::VoiceEffect(e) => crate::voice::soundboard::on_effect(self, e, cx),
            ev @ (AppEvent::MemberListRows { .. } | AppEvent::Activities { .. }) => {
                if let AppEvent::Activities { user, activities } = &ev
                    && *user == self.me.id
                {
                    self.people.custom_status = activities.iter().find(|x| x.kind == crate::presence::ActivityKind::Custom).and_then(|x| x.state.clone());
                }
                ui::members::on_event(self, ev, cx)
            }
            AppEvent::PresenceReset => {
                let me = self.me.id;
                self.presences.retain(|u, _| *u == me);
                self.people.activities.retain(|u, _| *u == me);
            }
            AppEvent::RemoteAuth(u) => crate::auth::on_remote_auth(self, u, cx),
            AppEvent::Resync(r) => crate::commands::on_resync(self, r, cx),
            AppEvent::Parity(e) => crate::commands::on_event(self, e, cx),
            AppEvent::Connection(up) => self.boot.connection(up, std::time::Instant::now()),
            AppEvent::Disconnected(reason) => {
                self.clear_session();
                self.login_error = Some(reason.into());
                self.screen = Screen::Login;
            }
        }
        crate::sounds::watch::watch(self);
        cx.notify();
    }

    pub fn apply_voice_state(&mut self, guild: Option<Id>, channel: Option<Id>, user: Id, own: bool) {
        let guild = guild.or_else(|| channel.and_then(|c| self.channels.get(&c)).and_then(|c| c.guild));
        let call_guild = self.voice_channel.and_then(|c| self.channels.get(&c)).map(|c| c.guild);
        if guild.is_none() {
            let dm = channel.filter(|c| self.channels.get(c).is_some_and(|x| x.guild.is_none()));
            crate::commands::state::call_voice_state(&mut self.parity.call_users, dm, user);
        }
        if guild.is_some() && call_guild == Some(guild) {
            crate::voice::roster::forget(user);
        }
        match (guild, channel) {
            (Some(g), _) => {
                for ch in self.channels.values_mut().filter(|c| c.guild == Some(g)) {
                    ch.voice_users.retain(|u| *u != user);
                }
                self.pending_voice.remove(&user);
                if let Some(c) = channel.and_then(|c| self.channels.get_mut(&c)) {
                    c.voice_users.push(user);
                } else if let Some(c) = channel {
                    self.pending_voice.insert(user, c);
                }
            }
            (None, Some(c)) if !self.channels.contains_key(&c) => {
                self.pending_voice.insert(user, c);
            }
            _ => {}
        }
        if user == self.me.id && !own && channel.is_some() && guild.is_some() && call_guild == Some(guild) {
            crate::voice::roster::reset(self);
            self.voice_channel = None;
            self.pending_own_voice = None;
        }
        if user == self.me.id && own {
            let stale_leave = channel.is_none() && guild.is_some() && call_guild.is_some_and(|g| g != guild);
            if !stale_leave {
                let prev = self.voice_channel;
                if channel != prev {
                    crate::voice::roster::reset(self);
                }
                if channel.is_some() {
                    crate::ui::voice_effects::prewarm();
                }
                self.voice_channel = channel;
                self.pending_own_voice = channel.filter(|c| !self.channels.contains_key(c));
            }
        }
    }

    fn refresh_selected_guild_channels(&mut self, guild: Id, cx: &mut Context<Self>) {
        if self.selected_guild != Some(guild) {
            return;
        }
        if ui::join_flow::is_lurking(guild) && self.selected_channel.is_none() && ui::discovery::current().is_none() {
            if let Some(channel) = self.guild_target(Some(guild)) {
                self.open_channel_chat(channel, cx);
            } else {
                self.message_list.reset(0);
            }
        }
        self.rebuild_sidebar();
    }

    fn set_guild_channels(&mut self, guild: Id, channels: Vec<crate::model::Channel>) {
        if let Some(old) = self.guild_channels.remove(&guild) {
            for id in old {
                self.channels.remove(&id);
            }
        }
        let mut ids = Vec::with_capacity(channels.len());
        for mut ch in channels {
            if self.selected_channel == Some(ch.id) {
                ch.unread = false;
                ch.mentions = 0;
            }
            ids.push(ch.id);
            self.channels.insert(ch.id, ch);
        }
        ids.sort_by_key(|id| self.channels[id].position);
        self.guild_channels.insert(guild, ids);
        crate::voice::roster::reapply(self);
        if let Some(sel) = self.selected_channel
            && !self.channels.contains_key(&sel)
        {
            self.chat.create_thread = None;
            self.selected_channel = None;
        }
    }

    pub fn delete_message(&mut self, id: Id, cx: &mut Context<Self>) {
        let Some(ch) = self.selected_channel else { return };
        if self.messages.get(&ch).and_then(|e| e.index_of(id).map(|i| &e.msgs[i])).is_some_and(ui::chat::pending::is_local) {
            self.discard_local(ch, id, cx);
            return;
        }
        self.backend.delete_message(ch, id).detach_ok();
        cx.notify();
    }

    fn expire_typing(&mut self, cx: &mut Context<Self>) {
        let before = self.typing.values().map(Vec::len).sum::<usize>();
        for v in self.typing.values_mut() {
            v.retain(|(_, t)| t.elapsed() < Duration::from_secs(10));
        }
        if before != self.typing.values().map(Vec::len).sum::<usize>() {
            self.update_typing_label();
            cx.notify();
        }
    }

    pub fn typing_name(&self, channel: Id, user: Id) -> Option<SharedString> {
        if self.nav.is_blocked(user) || self.nav.is_ignored(user) {
            return None;
        }
        self.typing_nicks
            .get(&(channel, user))
            .cloned()
            .or_else(|| self.member_nick(user))
            .or_else(|| self.users.get(&user).map(|u| u.display_name.clone()))
    }

    fn update_typing_label(&mut self) {
        let names: Vec<SharedString> = self
            .selected_channel
            .and_then(|c| self.typing.get(&c).map(|v| (c, v)))
            .map(|(c, v)| v.iter().filter_map(|(u, _)| self.typing_name(c, *u)).collect())
            .unwrap_or_default();
        self.typing_label = match names.as_slice() {
            [] => None,
            [a] => Some(crate::t!("chat.typing_one", a = a)),
            [a, b] => Some(crate::t!("chat.typing_two", a = a, b = b)),
            [a, b, c] => Some(crate::t!("chat.typing_three", a = a, b = b, c = c)),
            _ => Some(crate::t!("chat.typing_many")),
        };
    }

    fn on_composer_change(&mut self, cx: &mut Context<Self>) {
        ui::chat::rich_input::sync(self, cx);
        if let Some(ch) = self.selected_channel
            && !self.composer.read(cx).value().is_empty()
        {
            self.backend.trigger_typing(ch).detach_ok();
        }
    }

    pub fn submit_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_none() && ui::chat::composer::channel_read_only(self) {
            return;
        }
        if self.editing.is_none() {
            let announce = ui::chat::announce::is_active(self);
            if announce && !ui::chat::announce::prepare_submit(self, window, cx) {
                return;
            }
            let handled = self.chat_submit(window, cx);
            if announce {
                let sent = self.composer.read(cx).value().is_empty();
                ui::chat::announce::finish_submit(self, sent, window, cx);
            }
            if handled {
                return;
            }
        }
        let Some(ch) = self.selected_channel else { return };
        let text = ui::chat::rich_input::take_plain(self.composer.read(cx).value().trim());
        if text.is_empty() {
            return;
        }
        if self.editing.is_none() && ui::chat::composer::send_held(self, ch) {
            ui::chat::composer::emphasize_cooldown();
            cx.notify();
            return;
        }
        self.composer.update(cx, |s, cx| s.set_value("", window, cx));
        let text = if self.prefs.synced.flag("emoticons") { ui::settings::emoticons::convert(&text) } else { text };
        if let Some(id) = self.editing.take() {
            let fut = self.backend.edit_message(ch, id, text);
            cx.spawn(async move |this, cx| {
                if let Ok(m) = fut.await {
                    let _ = this.update(cx, |this, cx| this.replace_message(m, cx));
                }
            })
            .detach();
        } else {
            let reply = self.replying_to.take();
            ui::chat::composer::note_sent(ch);
            let fut = self.backend.send_message(ch, text, reply);
            cx.spawn(async move |this, cx| {
                if let Ok(m) = fut.await {
                    let _ = this.update(cx, |this, cx| this.append_message(m, cx));
                }
            })
            .detach();
        }
        cx.notify();
    }

    pub fn edit_last(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.composer.read(cx).value().is_empty() {
            return false;
        }
        let Some(ch) = self.selected_channel else { return false };
        let Some(m) = self
            .messages
            .get(&ch)
            .and_then(|e| e.msgs.iter().rev().find(|m| self.can_edit(m)))
        else {
            return false;
        };
        let id = m.id;
        self.start_edit(id, window, cx);
        true
    }

    pub fn start_reply(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        self.replying_to = Some(id);
        self.editing = None;
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub fn join_voice(&mut self, ch: Option<Id>, cx: &mut Context<Self>) {
        self.join_voice_with_sound(ch, true, cx);
    }

    pub fn join_voice_without_sound(&mut self, ch: Id, cx: &mut Context<Self>) {
        self.join_voice_with_sound(Some(ch), false, cx);
    }

    fn join_voice_with_sound(&mut self, ch: Option<Id>, play_sound: bool, cx: &mut Context<Self>) {
        self.pending_entrance_sound = ch.filter(|ch| play_sound && Some(*ch) != self.voice_channel);
        crate::voice::soundboard::stop_all();
        if ch.is_some() {
            crate::ui::voice_effects::prewarm();
        }
        self.backend.join_voice(ch).detach_ok();
        crate::voice::on_join_clicked(self, ch);
        cx.notify();
    }

    pub(crate) fn entrance_sound_for(&self, channel: Id) -> Option<phoenix_api::rest::settings::JoinSound> {
        if !self.backend.premium_tier2() {
            return None;
        }
        let channel_data = self.channels.get(&channel)?;
        let guild = channel_data.guild?;
        if channel_data.kind != crate::model::ChannelKind::Voice {
            return None;
        }
        let sounds = &self.prefs.synced.join_sounds;
        let sound = sounds.get(&guild.0).or_else(|| sounds.get(&0)).copied()?;
        let external = sound.source_guild != 0 && sound.source_guild != guild.0;
        self.backend.channel_permissions(channel)
            .is_none_or(|permissions| permissions & (1 << 42) != 0 && (!external || permissions & (1 << 45) != 0))
            .then_some(sound)
    }

    fn maybe_play_entrance_sound(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.pending_entrance_sound else { return };
        if self.voice.status != crate::voice::VoiceStatus::Connected || self.voice_channel != Some(channel) {
            return;
        }
        self.pending_entrance_sound = None;
        let Some(sound) = self.entrance_sound_for(channel) else { return };
        if crate::ui::pickers::soundboard::disabled(self) {
            return;
        }
        let catalog = &cx.global::<crate::ui::pickers::Pickers>().soundboard;
        let list = if sound.source_guild == 0 { catalog.defaults.as_ref() } else { catalog.guild_sounds.get(&Id(sound.source_guild)) };
        let volume = list.and_then(|sounds| sounds.iter().find(|s| s.id.0 == sound.sound)).map_or(1., |s| s.volume);
        let play = self.backend.play_entrance_sound(channel);
        cx.spawn(async move |this, cx| {
            if let Err(error) = play.await {
                tracing::debug!(%error, "could not play the voice entrance sound");
                return;
            }
            let _ = this.update(cx, |app, cx| {
                if app.voice_channel == Some(channel) {
                    crate::voice::soundboard::play(app, Id(sound.sound), volume, app.me.id, cx);
                }
            });
        }).detach();
    }

    pub fn toggle_category(&mut self, id: Id, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.rebuild_sidebar();
        cx.notify();
    }

    pub fn rebuild_sidebar(&mut self) {
        ui::sidebar::build_rows(self);
    }

    pub fn rebuild_members(&mut self) {
        if ui::members::sync_server_list(self) {
            return;
        }
        self.member_rows.clear();
        let Some(g) = self.selected_guild else {
            self.member_list.reset(0);
            return;
        };
        let roles = self.roles.get(&g).cloned().unwrap_or_default();
        let Some(members) = self.members.get(&g) else {
            self.member_list.reset(0);
            return;
        };
        let mut hoisted: Vec<&Role> = roles.iter().filter(|r| r.hoist).collect();
        hoisted.sort_by_key(|r| -r.position);
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); hoisted.len() + 2];
        for (i, m) in members.iter().enumerate() {
            let b = if m.presence == Presence::Offline {
                hoisted.len() + 1
            } else {
                hoisted
                    .iter()
                    .position(|r| m.roles.contains(&r.id))
                    .unwrap_or(hoisted.len())
            };
            buckets[b].push(i);
        }
        let users = &self.users;
        for (bi, mut b) in buckets.into_iter().enumerate() {
            if b.is_empty() {
                continue;
            }
            b.sort_by(|x, y| {
                let a = users.get(&members[*x].user).map(|u| u.display_name.to_lowercase());
                let bb = users.get(&members[*y].user).map(|u| u.display_name.to_lowercase());
                a.cmp(&bb)
            });
            let title = if bi < hoisted.len() {
                hoisted[bi].name.to_uppercase()
            } else if bi == hoisted.len() {
                crate::t!("friends.h_online").to_string()
            } else {
                crate::t!("members.h_offline").to_string()
            };
            self.member_rows
                .push(MemberRow::Header(format!("{title} — {}", b.len()).into()));
            self.member_rows.extend(b.into_iter().map(MemberRow::Member));
        }
        self.member_list.reset(self.member_rows.len() + 1);
    }

    pub fn member_role_style(&self, guild: Id, m: &Member) -> (Option<u32>, Option<crate::ui::role_gradient::RoleGradient>) {
        if self.prefs.local.pick("roles", 0) != 0 {
            return (None, None);
        }
        let Some(roles) = self.roles.get(&guild) else {
            return (None, None);
        };
        crate::ui::role_gradient::style_for_roles(roles, &m.roles)
    }

    pub fn channel_guild(&self, channel: Id) -> Option<Id> {
        self.channels.get(&channel).or_else(|| self.nav.find_thread(channel)).and_then(|c| c.guild)
    }

    pub fn member_nick_in(&self, guild: Option<Id>, user: Id) -> Option<SharedString> {
        let g = guild?;
        crate::member_index::find(self.members.get(&g)?, g, user)?.nick.clone()
    }

    pub fn member_nick(&self, user: Id) -> Option<SharedString> {
        self.member_nick_in(self.selected_guild, user)
    }

    pub fn member_user(&self, g: Option<Id>, user: Id) -> Option<std::borrow::Cow<'_, crate::model::User>> {
        let u = self.users.get(&user)?;
        let m = g.and_then(|g| crate::member_index::find(self.members.get(&g)?, g, user));
        match m {
            Some(m) if m.avatar.is_some() || m.decoration.is_some() || m.nameplate.is_some() => {
                let mut u = u.clone();
                if let Some(a) = &m.avatar {
                    u.avatar = crate::model::ImageRef::Url(a.clone());
                }
                if m.decoration.is_some() {
                    u.decoration = m.decoration.clone();
                }
                if m.nameplate.is_some() {
                    u.nameplate = m.nameplate.clone();
                }
                Some(std::borrow::Cow::Owned(u))
            }
            _ => Some(std::borrow::Cow::Borrowed(u)),
        }
    }

    pub fn author_role_dot_in(&self, guild: Option<Id>, user: Id) -> Option<(u32, Option<crate::ui::role_gradient::RoleGradient>)> {
        if self.prefs.local.pick("roles", 0) != 1 {
            return None;
        }
        let g = guild?;
        let m = crate::member_index::find(self.members.get(&g)?, g, user)?;
        let (color, gradient) = crate::ui::role_gradient::style_for_roles(self.roles.get(&g)?, &m.roles);
        Some((color?, gradient))
    }

    pub fn author_role_dot(&self, user: Id) -> Option<(u32, Option<crate::ui::role_gradient::RoleGradient>)> {
        self.author_role_dot_in(self.selected_guild, user)
    }

    pub fn author_role_style_in(&self, guild: Option<Id>, user: Id) -> (Option<u32>, Option<crate::ui::role_gradient::RoleGradient>) {
        guild
            .and_then(|g| Some((g, crate::member_index::find(self.members.get(&g)?, g, user)?)))
            .map_or((None, None), |(g, m)| self.member_role_style(g, m))
    }

    pub fn author_role_style(&self, user: Id) -> (Option<u32>, Option<crate::ui::role_gradient::RoleGradient>) {
        self.author_role_style_in(self.selected_guild, user)
    }

    pub fn author_role_icon_in(&self, guild: Option<Id>, user: Id) -> Option<(SharedString, SharedString)> {
        let g = guild?;
        let m = crate::member_index::find(self.members.get(&g)?, g, user)?;
        let r = self.roles.get(&g)?.iter().filter(|r| m.roles.contains(&r.id) && r.icon.is_some()).max_by_key(|r| r.position)?;
        Some((r.icon.clone()?, r.name.clone()))
    }

    pub fn author_role_icon(&self, user: Id) -> Option<(SharedString, SharedString)> {
        self.author_role_icon_in(self.selected_guild, user)
    }

    pub fn is_new_member(&self, user: Id) -> bool {
        const WEEK_MS: i64 = 7 * 86_400_000;
        let Some(g) = self.selected_guild else { return false };
        let Some(m) = self.members.get(&g).and_then(|ms| crate::member_index::find(ms, g, user)) else { return false };
        let now = chrono::Utc::now().timestamp_millis();
        let guild_ms = (g.0 >> 22) as i64 + 1_420_070_400_000;
        let bot = self.users.get(&user).is_some_and(|u| u.bot);
        !m.rejoined && !bot && now - guild_ms >= WEEK_MS && m.joined_at.is_some_and(|j| (0..=WEEK_MS).contains(&(now - j * 1000)))
    }

    pub fn presence_of(&self, user: Id) -> Presence {
        if user == self.me.id {
            return self.presence;
        }
        let known = self.presences.get(&user).copied();
        self.selected_guild
            .and_then(|g| self.members.get(&g))
            .and_then(|ms| ms.iter().find(|m| m.user == user))
            .map(|m| m.presence)
            .or(known)
            .unwrap_or(Presence::Offline)
    }

    pub fn open_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        ui::switcher::open(self, window, cx);
    }

    pub fn close_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.switcher = None;
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        self.escape_stack(window, cx);
        cx.notify();
    }
}

trait DetachOk {
    fn detach_ok(self);
}
impl<T: Send + 'static> DetachOk for crate::backend::BoxFuture<T> {
    fn detach_ok(self) {
        std::thread::spawn(move || {
            let _ = futures::executor::block_on(self);
        });
    }
}

impl Focusable for PhoenixApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PhoenixApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::media::lru::next_frame(self.selected_channel.map_or(0, |c| c.0));
        ui::profile_fx::sweep(cx);
        crate::perf::on_render(self.selected_channel.map(|c| c.0), self.selected_channel.and_then(|c| self.messages.get(&c)).is_some_and(|e| e.loaded_once));
        ui::context_menu::begin_frame();
        ui::chat::selection::begin_frame(self.selected_channel.map(|c| c.0));
        ui::a11y::sync(window);
        crate::auth::ensure_qr(self, cx);
        crate::auth::sync(self, window, cx);
        crate::theme::sync(&self.prefs, window, cx);
        crate::sounds::set_focused(window.is_window_active());
        phoenix_api::props::set_app_skeleton(self.screen == Screen::Main);
        self.backend.set_activity(window.is_window_active(), self.voice.status == crate::voice::VoiceStatus::Connected);
        crate::notifications::sync_focus(window.is_window_active(), self.selected_channel.map(|c| c.0));
        crate::media::emoji::sync_motion(self.prefs.synced.flag("anim-emoji"), window.is_window_active());
        crate::ui::settings::effects::sync(self, cx);
        crate::tts::sync(self);
        if ui::chat::rich_input::needs_sync(self, cx) {
            let e = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = e.update(cx, ui::chat::rich_input::sync);
            });
        }
        self.chat_sync_style();
        if ui::focus_ring::sync(window, cx) {
            cx.notify();
        }
        let body = match self.screen {
            Screen::Login => ui::login::render(self, window, cx).into_any_element(),
            Screen::Main => {
                let title = ui::title_bar::render(self, window, cx).into_any_element();
                let rail = ui::server_bar::render(self, cx).into_any_element();
                let notice = ui::join_flow::lurker_banner(self, cx)
                    .or_else(|| crate::streamer::notice(self, cx))
                    .or_else(|| ui::view_as::notice(self, cx));
                let shift = notice.is_some();
                let below_notice = |el: gpui::AnyElement, grow: bool| {
                    if !shift {
                        return el;
                    }
                    div()
                        .when(grow, |d| d.flex_1().min_w_0())
                        .when(!grow, |d| d.flex_none())
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(div().flex_none().h(crate::theme::z(crate::streamer::NOTICE_H)))
                        .child(div().flex_1().min_h_0().flex().child(el))
                        .into_any_element()
                };
                let sidebar = below_notice(ui::sidebar::render(self, cx).into_any_element(), false);
                let panels = ui::sidebar::panels(self, cx).into_any_element();
                let pane = below_notice(ui::home::main_pane(self, window, cx), true);
                let left_col = div()
                    .flex_none()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(div().flex_1().min_h_0().flex().flex_row().child(rail).child(sidebar))
                    .child(panels);
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(title)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_row()
                            .relative()
                            .child(left_col)
                            .child(pane)
                            .children(notice),
                    )
                    .into_any_element()
            }
        };
        let (sx, sy) = ui::shake::offset(window, crate::ui::settings::effects::reduced_motion()).unwrap_or((0., 0.));
        let body = div().size_full().relative().left(px(sx)).top(px(sy)).child(body).into_any_element();
        let boot = ui::boot::render(self, window, cx);
        let root = div()
            .id("phoenix-root")
            .when(ui::a11y::on(), |d| d.role(gpui::Role::Application).aria_label("Discord"))
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &QuickSwitcher, w, cx| this.open_switcher(w, cx)))
            .on_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(ui::settings::open_action))
            .on_modifiers_changed(cx.listener(|this, ev: &gpui::ModifiersChangedEvent, _, cx| {
                if this.chat.shift_held != ev.modifiers.shift {
                    this.chat.shift_held = ev.modifiers.shift;
                    cx.notify();
                }
            }))
            .capture_key_down(cx.listener(|this, ev, _, cx| {
                if !crate::keybinds::on_key_down(this, ev, cx) {
                    crate::voice::ptt::on_key_down(this, ev, cx)
                }
            }))
            .capture_key_up(cx.listener(|this, ev, _, cx| {
                crate::keybinds::on_key_up(this, ev, cx);
                crate::voice::ptt::on_key_up(this, ev, cx)
            }))
            .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, w, cx| {
                if ui::focus_ring::on_key_down(ev, w, cx) {
                    cx.stop_propagation();
                    return;
                }
                let k = &ev.keystroke;
                if k.key == "enter" && k.modifiers.number_of_modifiers() == 0 && !ev.is_held && ui::chat::voice_message::on_enter(this, cx) {
                    cx.stop_propagation();
                }
            }))
            .map(|d| crate::shortcuts::handlers(self, d, window, cx))
            .on_action(cx.listener(|this, _: &ToggleMembers, _, cx| {
                this.show_members = !this.show_members;
                cx.notify();
            }))
            .size_full()
            .relative()
            .map(crate::fonts::ui_root)
            .text_color(c(Palette::TEXT_NORMAL))
            .bg(c(Palette::BG_TERTIARY))
            .children(crate::theme::client_theme::backdrop(window))
            .child({
                let (dx, dy) = ui::easter::shake_offset(window);
                ui::popout_motion::base(div().size_full().relative().left(gpui::px(dx)).top(gpui::px(dy)).child(body))
            })
            .children(ui::settings::render(self, window, cx))
            .children(ui::boost_layer::render(self, window, cx).map(|e| if crate::actions::with_modal(|m| m.is_some()) { gpui::deferred(e).with_priority(27).into_any_element() } else { e }))
            .children(ui::chat::panels::overlays(self, window, cx))
            .children(ui::home::overlay(self, window, cx))
            .when_some(self.switcher.as_ref(), |d, sw| d.child(ui::switcher::render(self, sw, cx)))
            .children(ui::image_viewer::render(window, cx))
            .children(ui::autocomplete::render_for_edit(self, window, cx))
            .children(ui::pickers::render_overlays(self, window, cx))
            .children(ui::chat::app_launcher::render(self, window, cx))
            .children(ui::profile::overlays(self, window, cx))
            .children(ui::tag_popout::render(self, window, cx))
            .children(ui::components::overlays(self, window, cx))
            .children(ui::voice_panel::overlays(self, window, cx))
            .children(ui::pip::render(self, window, cx))
            .children(crate::actions::overlays(self, window, cx))
            .children(ui::pickers::render_status_picker(self, window, cx))
            .children(crate::shortcuts::render_help(self, window, cx))
            .children(boot)
            .when(self.screen == Screen::Login && !self.boot.showing(), |d| d.child(ui::title_bar::login_bar(window, cx)))
            .children(ui::window_chrome::resize_handles(window))
            .children(ui::incoming_call::render(self, cx))
            .when(ui::a11y::on(), |d| d.child(ui::a11y::announcer()));
        crate::media::lru::scope(root)
    }
}

#[cfg(test)]
mod trap_tests {
    use std::sync::Arc;

    use super::PhoenixApp;
    use crate::{backend::Backend, mock::MockBackend, model::ChannelKind};

    #[gpui::test]
    fn opening_a_server_never_lands_on_a_honeypot(cx: &mut gpui::TestAppContext) {
        let backend = Arc::new(MockBackend::synchronous());
        let ready = futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| PhoenixApp::new(backend.clone(), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(ready, cx);
                let g = app.guilds[0].id;
                let text: Vec<_> = app.guild_channels[&g].iter().copied().filter(|c| app.channels[c].kind == ChannelKind::Text).collect();
                assert!(text.len() >= 2);
                for c in app.guild_channels[&g].clone() {
                    if let Some(ch) = app.channels.get_mut(&c) {
                        ch.name = "salon".into();
                    }
                }
                app.last_channel.clear();
                app.channels.get_mut(&text[0]).unwrap().name = "〢🍯・honeypot".into();
                assert!(app.is_trap_channel(text[0]));
                assert!(!app.is_trap_channel(text[1]));
                assert_eq!(app.guild_target(Some(g)), Some(text[1]));
                app.channels.get_mut(&text[1]).unwrap().topic = Some("Honeypot, do not post here".into());
                assert!(app.is_trap_channel(text[1]));
                assert_ne!(app.guild_target(Some(g)), Some(text[1]));
                app.last_channel.insert(Some(g), text[0]);
                assert_ne!(app.guild_target(Some(g)), Some(text[0]));
                app.select_guild(Some(g), cx);
                assert!(app.selected_channel.is_none_or(|c| !app.is_trap_channel(c)));
            })
            .unwrap();
    }
}

