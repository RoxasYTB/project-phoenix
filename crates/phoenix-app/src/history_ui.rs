use std::{cell::RefCell, collections::VecDeque};

use gpui::{AsyncApp, Context, ListOffset, WeakEntity};

use crate::{
    app::PhoenixApp,
    model::{ChannelKind, Id, Message},
};

const PRELOAD_CAP: usize = 24;

thread_local! {
    static PRELOADED: RefCell<VecDeque<Id>> = const { RefCell::new(VecDeque::new()) };
    static ASKED_CHANNELS: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
}

pub(crate) fn unknown_channel_mentions(app: &PhoenixApp, msgs: &[Message]) -> Vec<u64> {
    let (mut u, mut r, mut c) = (HashSet::new(), HashSet::new(), HashSet::new());
    for m in msgs {
        mention_ids(&m.content, &mut u, &mut r, &mut c);
    }
    let mut out: Vec<u64> = c.into_iter().filter(|id| !app.channels.contains_key(&Id(*id)) && app.nav.find_thread(Id(*id)).is_none()).collect();
    out.sort_unstable();
    out
}

pub(crate) fn fetch_mentioned_channels(app: &mut PhoenixApp, msgs: &[Message], cx: &mut Context<PhoenixApp>) {
    let ids: Vec<u64> = unknown_channel_mentions(app, msgs).into_iter().filter(|id| ASKED_CHANNELS.with(|a| a.borrow_mut().insert(*id))).take(5).collect();
    for id in ids {
        app.parity_request(crate::commands::ParityRequest::FetchThread { id: Id(id) }, cx, move |a, r, cx| {
            if let Ok(crate::commands::ParityReply::Thread(ch)) = r {
                a.channels.entry(Id(id)).or_insert(ch);
                a.rerender_channel_mentions_of(&[id]);
                cx.notify();
            }
        });
    }
}

pub fn on_refresh(app: &mut PhoenixApp, channel: Id, messages: Vec<Message>, cx: &mut Context<PhoenixApp>) {
    refresh_when_idle(app, channel, messages, 0, cx);
}

fn refresh_when_idle(app: &mut PhoenixApp, channel: Id, messages: Vec<Message>, tries: u8, cx: &mut Context<PhoenixApp>) {
    let Some(e) = app.messages.get_mut(&channel).filter(|e| e.msgs.len() <= 50) else { return };
    if !e.loaded_once || e.loading {
        if tries < 40 {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                let _ = this.update(cx, |a, cx| refresh_when_idle(a, channel, messages, tries + 1, cx));
            })
            .detach();
        }
        return;
    }
    e.loading = true;
    cx.spawn(async move |this, cx| {
        let laid = layout(&this, cx, messages).await;
        let _ = this.update(cx, |a, cx| {
            let Some(e) = a.messages.get_mut(&channel) else { return };
            if !e.loading {
                return;
            }
            match laid {
                Some((msgs, rendered)) if e.msgs.len() <= 50 => replace_page(a, channel, msgs, rendered, cx),
                _ => e.loading = false,
            }
        });
    })
    .detach();
}

fn replace_page(app: &mut PhoenixApp, channel: Id, msgs: Vec<Message>, rendered: Vec<RenderedMessage>, cx: &mut Context<PhoenixApp>) {
    let anchor = (app.selected_channel == Some(channel) && !app.chat.at_bottom).then(|| {
        let top = app.message_list.logical_scroll_top();
        let id = top.item_ix.checked_sub(1).and_then(|i| app.messages.get(&channel)?.msgs.get(i).map(|m| m.id));
        (id, top.offset_in_item)
    });
    insert_first_page(app, channel, msgs, rendered, cx);
    if let Some((Some(id), offset_in_item)) = anchor
        && let Some(ix) = app.messages.get(&channel).and_then(|e| e.index_of(id))
    {
        app.message_list.scroll_to(ListOffset { item_ix: ix + 1, offset_in_item });
    }
}

fn preload_off() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OFF.get_or_init(|| std::env::var_os("PHX_NO_PRELOAD").is_some())
}

fn preloadable(app: &PhoenixApp, id: Id) -> bool {
    !preload_off()
        && app.selected_channel != Some(id)
        && app.channels.get(&id).is_some_and(|c| matches!(c.kind, ChannelKind::Text | ChannelKind::Announcement))
        && !app.messages.get(&id).is_some_and(|e| e.loaded_once || e.loading)
}

pub fn preload(app: &mut PhoenixApp, id: Id, cx: &mut Context<PhoenixApp>) {
    if !preloadable(app, id) {
        return;
    }
    app.messages.entry(id).or_default().loading = true;
    load_first_page(app, id, cx);
    let evicted = PRELOADED.with_borrow_mut(|q| {
        q.retain(|c| *c != id);
        q.push_back(id);
        if q.len() > PRELOAD_CAP { q.pop_front() } else { None }
    });
    if let Some(old) = evicted
        && app.selected_channel != Some(old)
        && !app.last_channel.values().any(|c| *c == old)
        && app.messages.get(&old).is_some_and(|e| !e.loading && !e.detached && e.msgs.len() <= 50)
    {
        app.messages.remove(&old);
    }
}

pub fn hover_channel(app: &mut PhoenixApp, id: Id, cx: &mut Context<PhoenixApp>) {
    if preload_off() {
        app.backend.prefetch_messages(id);
        return;
    }
    if !preloadable(app, id) {
        return;
    }
    if app.backend.has_cached_messages(id) {
        preload(app, id, cx);
    } else {
        app.backend.prefetch_messages(id);
    }
}

pub fn hover_guild(app: &mut PhoenixApp, guild: Id, cx: &mut Context<PhoenixApp>) {
    if preload_off() || app.selected_guild == Some(guild) {
        return;
    }
    if let Some(id) = app.guild_target(Some(guild)) {
        hover_channel(app, id, cx);
    }
}

pub fn warm_recent(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) {
    if preload_off() {
        return;
    }
    let backend = app.backend.clone();
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
        let ids = cx.background_executor().spawn(async move { backend.recent_cached_channels(8) }).await;
        for id in ids {
            if this.update(cx, |a, cx| preload(a, id, cx)).is_err() {
                return;
            }
            cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
        }
    })
    .detach();
}

async fn layout(this: &WeakEntity<PhoenixApp>, cx: &mut AsyncApp, msgs: Vec<Message>) -> Option<(Vec<Message>, Vec<RenderedMessage>)> {
    let (resolver, me, off, revealed) = this
        .update(cx, |a, _| {
            let rev: HashSet<(Id, usize)> = a.revealed.iter().filter(|(m, _)| msgs.iter().any(|x| x.id == *m)).copied().collect();
            (page_resolver(a, &msgs), a.me.id, a.utc_offset, rev)
        })
        .ok()?;
    if cfg!(test) {
        let r = render_page(&msgs, &resolver, me, off, &revealed);
        return Some((msgs, r));
    }
    let t0 = std::time::Instant::now();
    let (tx, rx) = futures::channel::oneshot::channel();
    let job = move || {
        let r = render_page(&msgs, &resolver, me, off, &revealed);
        let _ = tx.send((msgs, r));
    };
    std::thread::Builder::new().name("phoenix-layout".into()).spawn(job).ok()?;
    let out = rx.await.ok()?;
    crate::perf::span("page_layout_bg", t0);
    Some(out)
}

use std::collections::{HashMap, HashSet};

use crate::{grouping, markdown::{MentionResolver, SoundboardMention}, richtext::{self, RenderedMessage}};

#[derive(Default)]
pub struct PageResolver {
    users: HashMap<u64, String>,
    channels: HashMap<u64, String>,
    mention_channels: HashMap<u64, crate::markdown::MentionChannel>,
    roles: HashMap<u64, (String, Option<u32>, bool)>,
    sounds: HashMap<(u64, u64), SoundboardMention>,
    current_guild: Option<u64>,
    link_channels: HashMap<u64, crate::markdown::LinkChannel>,
    link_guilds: HashMap<u64, crate::markdown::LinkGuild>,
}

impl MentionResolver for PageResolver {
    fn user(&self, id: u64) -> Option<String> {
        self.users.get(&id).cloned()
    }
    fn channel(&self, id: u64) -> Option<String> {
        self.channels.get(&id).cloned()
    }
    fn mention_channel(&self, id: u64) -> Option<crate::markdown::MentionChannel> {
        self.mention_channels.get(&id).copied()
    }
    fn role(&self, id: u64) -> Option<String> {
        self.roles.get(&id).map(|r| r.0.clone())
    }
    fn role_color(&self, id: u64) -> Option<u32> {
        self.roles.get(&id).and_then(|r| r.1)
    }
    fn has_role(&self, id: u64) -> bool {
        self.roles.get(&id).is_some_and(|r| r.2)
    }
    fn current_guild(&self) -> Option<u64> {
        self.current_guild
    }
    fn link_channel(&self, id: u64) -> Option<crate::markdown::LinkChannel> {
        self.link_channels.get(&id).cloned()
    }
    fn link_guild(&self, id: u64) -> Option<crate::markdown::LinkGuild> {
        self.link_guilds.get(&id).cloned()
    }
    fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<SoundboardMention> {
        self.sounds.get(&(guild_id, sound_id)).or_else(|| self.sounds.get(&(0, sound_id))).cloned()
    }
}

fn link_ids(text: &str, guilds: &mut HashSet<u64>, channels: &mut HashSet<u64>) {
    for (i, _) in text.match_indices("https://") {
        if let Some(m) = crate::markdown::channel_link::match_url(&text[i..]) {
            if let Some(g) = m.guild {
                guilds.insert(g);
            }
            if let Some(c) = m.channel {
                channels.insert(c);
            }
        }
    }
}

fn mention_ids(text: &str, users: &mut HashSet<u64>, roles: &mut HashSet<u64>, channels: &mut HashSet<u64>) {
    for (i, _) in text.match_indices('<') {
        let rest = &text[i + 1..];
        let (set, body) = if let Some(b) = rest.strip_prefix("@&") {
            (&mut *roles, b)
        } else if let Some(b) = rest.strip_prefix("@!") {
            (&mut *users, b)
        } else if let Some(b) = rest.strip_prefix('@') {
            (&mut *users, b)
        } else if let Some(b) = rest.strip_prefix('#') {
            (&mut *channels, b)
        } else {
            continue;
        };
        let digits: &str = &body[..body.find(|c: char| !c.is_ascii_digit()).unwrap_or(body.len())];
        if body[digits.len()..].starts_with('>')
            && let Ok(id) = digits.parse()
        {
            set.insert(id);
        }
    }
}

pub fn page_resolver(app: &PhoenixApp, msgs: &[Message]) -> PageResolver {
    let (mut u, mut r, mut c) = (HashSet::new(), HashSet::new(), HashSet::new());
    let (mut link_g, mut link_c) = (HashSet::new(), HashSet::new());
    for m in msgs {
        mention_ids(&m.content, &mut u, &mut r, &mut c);
        link_ids(&m.content, &mut link_g, &mut link_c);
    }
    let guild = msgs.first().and_then(|m| app.channels.get(&m.channel).or_else(|| app.nav.find_thread(m.channel))).and_then(|c| c.guild);
    let members = guild.and_then(|g| Some((g, app.members.get(&g)?)));
    let resolver = app.resolver();
    let mut out = PageResolver {
        current_guild: guild.map(|g| g.0).or_else(|| resolver.current_guild()),
        ..PageResolver::default()
    };
    for id in u {
        let nick = members.and_then(|(g, ms)| crate::member_index::find(ms, g, Id(id))).and_then(|m| m.nick.as_ref().map(ToString::to_string));
        if let Some(name) = nick.or_else(|| app.users.get(&Id(id)).map(|u| u.display_name.to_string())) {
            out.users.insert(id, name);
        }
    }
    for id in c {
        if let Some(ch) = app.channels.get(&Id(id)).or_else(|| app.nav.find_thread(Id(id))) {
            out.channels.insert(id, ch.name.to_string());
        }
        if let Some(mc) = resolver.mention_channel(id) {
            out.mention_channels.insert(id, mc);
        }
    }
    for id in r {
        let Some((g, role)) = app.roles.iter().find_map(|(g, rs)| rs.iter().find(|x| x.id.0 == id).map(|x| (*g, x))) else {
            continue;
        };
        let mine = app
            .members
            .get(&g)
            .and_then(|ms| ms.iter().find(|m| m.user == app.me.id))
            .is_some_and(|m| m.roles.contains(&Id(id)));
        out.roles.insert(id, (role.name.to_string(), role.color, mine));
    }
    for id in link_c {
        if let Some(ch) = resolver.link_channel(id) {
            if let Some(g) = ch.guild {
                link_g.insert(g);
            }
            out.link_channels.insert(id, ch);
        }
    }
    for id in link_g {
        if let Some(g) = resolver.link_guild(id) {
            out.link_guilds.insert(id, g);
        }
    }
    out
}

struct MessagePageResolver<'a> {
    base: &'a PageResolver,
    sounds: &'a [crate::model::SoundboardSound],
}

impl MentionResolver for MessagePageResolver<'_> {
    fn user(&self, id: u64) -> Option<String> {
        self.base.user(id)
    }
    fn channel(&self, id: u64) -> Option<String> {
        self.base.channel(id)
    }
    fn mention_channel(&self, id: u64) -> Option<crate::markdown::MentionChannel> {
        self.base.mention_channel(id)
    }
    fn role(&self, id: u64) -> Option<String> {
        self.base.role(id)
    }
    fn role_color(&self, id: u64) -> Option<u32> {
        self.base.role_color(id)
    }
    fn has_role(&self, id: u64) -> bool {
        self.base.has_role(id)
    }
    fn current_guild(&self) -> Option<u64> {
        self.base.current_guild()
    }
    fn link_channel(&self, id: u64) -> Option<crate::markdown::LinkChannel> {
        self.base.link_channel(id)
    }
    fn link_guild(&self, id: u64) -> Option<crate::markdown::LinkGuild> {
        self.base.link_guild(id)
    }
    fn soundboard(&self, guild_id: u64, sound_id: u64) -> Option<SoundboardMention> {
        self.sounds
            .iter()
            .find(|s| s.id.0 == sound_id && (guild_id == 0 || s.guild.is_none_or(|g| g.0 == guild_id)))
            .map(|sound| SoundboardMention {
                name: sound.name.as_ref().map(ToString::to_string).filter(|n| !n.is_empty()).unwrap_or_else(|| "Unknown".into()),
                emoji_name: sound.emoji_name.as_ref().map(ToString::to_string),
            })
            .or_else(|| self.base.soundboard(guild_id, sound_id))
    }
}

pub fn render_page(
    msgs: &[Message],
    resolver: &PageResolver,
    me: Id,
    utc_offset: i64,
    revealed: &HashSet<(Id, usize)>,
) -> Vec<RenderedMessage> {
    (0..msgs.len())
        .map(|i| {
            let (m, prev) = (&msgs[i], i.checked_sub(1).map(|p| &msgs[p]));
            let message_resolver = MessagePageResolver {
                base: resolver,
                sounds: &m.soundboard_sounds,
            };
            let rev = |s: usize| revealed.contains(&(m.id, s));
            let (parsed, blocks, mentions_me, preview, preview_mentions) = richtext::build(&m.content, &message_resolver, me, &rev);
            RenderedMessage {
                group_start: grouping::starts_group(prev, m, utc_offset),
                day_divider: (prev.is_none() || grouping::day_divider(prev, m, utc_offset))
                    .then(|| crate::app::format_day(m.timestamp)),
                time_full: crate::app::format_time_full(m.timestamp),
                time_short: crate::app::format_time_short(m.timestamp),
                blocks,
                mentions_me,
                preview,
                preview_mentions,
                parsed,
            }
        })
        .collect()
}

pub fn load_first_page(app: &PhoenixApp, id: Id, cx: &mut Context<PhoenixApp>) {
    let fut = app.backend.load_messages(id, None, 50);
    cx.spawn(async move |this, cx| {
        let res = fut.await;
        let loaded = res.is_ok();
        let on_screen = this.update(cx, |a, _| a.selected_channel == Some(id)).unwrap_or(false);
        let msgs = match res {
            Ok(m) => m,
            Err(_) if !on_screen => {
                let _ = this.update(cx, |a, _| {
                    if let Some(e) = a.messages.get_mut(&id).filter(|e| !e.loaded_once) {
                        e.loading = false;
                    }
                });
                return;
            }
            Err(_) => Vec::new(),
        };
        if on_screen {
            let Ok((resolver, me, off, revealed)) = this.update(cx, |a, _| {
                let rev: HashSet<(Id, usize)> = a.revealed.iter().filter(|(m, _)| msgs.iter().any(|x| x.id == *m)).copied().collect();
                (page_resolver(a, &msgs), a.me.id, a.utc_offset, rev)
            }) else {
                return;
            };
            let rendered = render_page(&msgs, &resolver, me, off, &revealed);
            let _ = this.update(cx, |a, cx| {
                if loaded {
                    crate::ui::chat::welcome::observe_history(a, id, &msgs);
                }
                insert_first_page(a, id, msgs, rendered, cx);
                crate::commands::state::apply_read_mark(a, id);
                if loaded {
                    a.restore_chat_position(id, cx);
                }
            });
            return;
        }
        let Some((msgs, rendered)) = layout(&this, cx, msgs).await else { return };
        let _ = this.update(cx, |a, cx| {
            if loaded {
                crate::ui::chat::welcome::observe_history(a, id, &msgs);
            }
            insert_first_page(a, id, msgs, rendered, cx);
            crate::commands::state::apply_read_mark(a, id);
            if loaded {
                a.restore_chat_position(id, cx);
            }
        });
    })
    .detach();
}

pub fn load_older(app: &mut PhoenixApp, cx: &mut Context<PhoenixApp>) {
    let Some(id) = app.selected_channel else { return };
    let Some(entry) = app.messages.get_mut(&id) else { return };
    if entry.loading || entry.reached_start || !entry.loaded_once {
        return;
    }
    let before = entry.msgs.first().map(|m| m.id);
    entry.loading = true;
    let fut = app.backend.load_messages(id, before, 50);
    cx.spawn(async move |this, cx| {
        let msgs = fut.await.unwrap_or_default();
        let laid = if msgs.is_empty() { Some((msgs, Vec::new())) } else { layout(&this, cx, msgs).await };
        let _ = this.update(cx, |a, cx| match laid {
            Some((msgs, rendered)) => insert_older(a, id, msgs, rendered, cx),
            None => {
                if let Some(e) = a.messages.get_mut(&id) {
                    e.loading = false;
                }
            }
        });
    })
    .detach();
}

fn insert_older(app: &mut PhoenixApp, ch: Id, page: Vec<Message>, mut rendered: Vec<RenderedMessage>, cx: &mut Context<PhoenixApp>) {
    let Some(mut entry) = app.messages.remove(&ch) else { return };
    entry.loading = false;
    if page.len() < 50 {
        entry.reached_start = true;
    }
    let n = page.len();
    let mut all = page;
    all.extend(std::mem::take(&mut entry.msgs));
    rendered.append(&mut entry.rendered);
    if n > 0
        && let (Some(prev), Some(m), Some(r)) = (all.get(n - 1), all.get(n), rendered.get_mut(n))
    {
        r.group_start = grouping::starts_group(Some(prev), m, app.utc_offset);
        r.day_divider = grouping::day_divider(Some(prev), m, app.utc_offset).then(|| crate::app::format_day(m.timestamp));
    }
    entry.msgs = all;
    entry.rendered = rendered;
    let len = entry.msgs.len();
    app.messages.insert(ch, entry);
    if app.selected_channel == Some(ch) {
        crate::ui::chat::rows::prepend(&app.message_list, ch, n, len);
    }
    app.resume_pending_jump(ch, cx);
    cx.notify();
}

fn insert_first_page(app: &mut PhoenixApp, ch: Id, page: Vec<Message>, rendered: Vec<RenderedMessage>, cx: &mut Context<PhoenixApp>) {
    crate::perf::channel_messages_ready(ch.0, "load");
    let mut entry = app.messages.remove(&ch).unwrap_or_default();
    entry.loading = false;
    entry.loaded_once = true;
    entry.reached_start = page.len() < 50;
    entry.msgs = page;
    entry.rendered = rendered;
    let len = entry.msgs.len();
    let unknown = !unknown_channel_mentions(app, &entry.msgs).is_empty();
    let msgs = if unknown { entry.msgs.clone() } else { Vec::new() };
    app.messages.insert(ch, entry);
    if app.selected_channel == Some(ch) {
        app.message_list.reset(len + 1);
    }
    if unknown {
        fetch_mentioned_channels(app, &msgs, cx);
    }
    app.resume_pending_jump(ch, cx);
    cx.notify();
}

#[cfg(test)]
mod preload_tests {
    use std::sync::Arc;

    use crate::{app::PhoenixApp, backend::Backend, mock::MockBackend, model::ChannelKind};

    #[gpui::test]
    fn mention_of_a_thread_known_only_from_the_sidebar_shows_its_name(cx: &mut gpui::TestAppContext) {
        let backend = Arc::new(MockBackend::synchronous());
        let ready = futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| PhoenixApp::new(backend.clone(), w, cx));
        window
            .update(cx, |app, _, cx| {
                app.apply_ready(ready, cx);
                let g = app.guilds[0].id;
                let text: Vec<_> = app.guild_channels[&g].iter().copied().filter(|c| app.channels[c].kind == ChannelKind::Text).collect();
                let mut thread = app.channels[&text[0]].clone();
                thread.id = crate::model::Id(987_654_321_012_345);
                thread.name = "fil-de-test".into();
                thread.parent = Some(text[0]);
                let mut msg = app.messages.values().flat_map(|e| e.msgs.iter()).next().cloned().unwrap_or_default();
                msg.content = format!("voir <#{}>", thread.id.0);
                assert_eq!(super::unknown_channel_mentions(app, std::slice::from_ref(&msg)), vec![thread.id.0]);
                app.nav.threads.entry(text[0]).or_default().push(thread.clone());
                assert!(super::unknown_channel_mentions(app, std::slice::from_ref(&msg)).is_empty());
                let r = super::page_resolver(app, std::slice::from_ref(&msg));
                use crate::markdown::MentionResolver;
                assert_eq!(r.channel(thread.id.0).as_deref(), Some("fil-de-test"));
                assert_eq!(app.resolver().channel(thread.id.0).as_deref(), Some("fil-de-test"));
            })
            .unwrap();
    }

    #[gpui::test]
    fn hovered_channel_is_in_memory_before_the_click(cx: &mut gpui::TestAppContext) {
        let backend = Arc::new(MockBackend::synchronous());
        let ready = futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| PhoenixApp::new(backend.clone(), w, cx));
        let (first, next) = window
            .update(cx, |app, _, cx| {
                app.apply_ready(ready, cx);
                let g = app.guilds[0].id;
                app.select_guild(Some(g), cx);
                let text: Vec<_> = app.guild_channels[&g].iter().copied().filter(|c| app.channels[c].kind == ChannelKind::Text).collect();
                app.select_channel(text[0], cx);
                super::preload(app, text[1], cx);
                assert!(app.messages[&text[1]].loading);
                (text[0], text[1])
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |app, _, cx| {
                assert_eq!(app.selected_channel, Some(first));
                let e = &app.messages[&next];
                assert!(e.loaded_once && !e.loading);
                assert_eq!(e.msgs.len(), e.rendered.len());
                let n = e.msgs.len();
                app.select_channel(next, cx);
                assert_eq!(app.messages[&next].msgs.len(), n);
                assert!(!app.messages[&next].loading);
            })
            .unwrap();
    }
}
