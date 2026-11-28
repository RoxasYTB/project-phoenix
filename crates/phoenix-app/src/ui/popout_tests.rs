use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use gpui::{Bounds, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, WindowHandle, point, px};

use crate::{
    app::PhoenixApp,
    backend::*,
    mock::MockBackend,
    model::{ChannelKind, Id, Message},
    ui::{
        home::data::{NavReply, NavRequest},
        inbox_unreads::{Entry, Kind},
    },
};

struct Probe {
    mock: MockBackend,
    acks: AtomicUsize,
    leaves: AtomicUsize,
    unreads: Mutex<Vec<Entry>>,
}

impl Probe {
    fn ack(&self) {
        self.acks.fetch_add(1, Ordering::SeqCst);
    }
}

impl Backend for Probe {
    fn set_event_sink(&self, sink: EventSink) {
        self.mock.set_event_sink(sink)
    }
    fn login(&self, creds: Credentials) -> BoxFuture<ReadyData> {
        self.mock.login(creds)
    }
    fn select_guild(&self, guild: Option<Id>) -> BoxFuture<()> {
        self.mock.select_guild(guild)
    }
    fn select_channel(&self, channel: Id) -> BoxFuture<()> {
        self.ack();
        self.mock.select_channel(channel)
    }
    fn load_messages(&self, channel: Id, before: Option<Id>, limit: u32) -> BoxFuture<Vec<Message>> {
        self.mock.load_messages(channel, before, limit)
    }
    fn load_messages_at(&self, channel: Id, _at: HistoryAt, limit: u32) -> BoxFuture<Vec<Message>> {
        self.mock.load_messages(channel, None, limit)
    }
    fn send_message(&self, channel: Id, content: String, reply_to: Option<Id>) -> BoxFuture<Message> {
        self.mock.send_message(channel, content, reply_to)
    }
    fn edit_message(&self, channel: Id, id: Id, content: String) -> BoxFuture<Message> {
        self.mock.edit_message(channel, id, content)
    }
    fn trigger_typing(&self, channel: Id) -> BoxFuture<()> {
        self.mock.trigger_typing(channel)
    }
    fn join_voice(&self, channel: Option<Id>) -> BoxFuture<()> {
        if channel.is_none() {
            self.leaves.fetch_add(1, Ordering::SeqCst);
        }
        self.mock.join_voice(channel)
    }
    fn delete_message(&self, channel: Id, id: Id) -> BoxFuture<()> {
        self.mock.delete_message(channel, id)
    }
    fn mark_unread(&self, channel: Id, last_read: Id) -> BoxFuture<()> {
        self.ack();
        self.mock.mark_unread(channel, last_read)
    }
    fn nav(&self, req: NavRequest) -> BoxFuture<NavReply> {
        if matches!(req, NavRequest::AckChannel(_) | NavRequest::AckGuild(_)) {
            self.ack();
        }
        self.mock.nav(req)
    }
    fn parity(&self, req: crate::commands::ParityRequest) -> BoxFuture<crate::commands::ParityReply> {
        self.mock.parity(req)
    }
    fn load_pins(&self, channel: Id) -> BoxFuture<Vec<Message>> {
        self.mock.load_pins(channel)
    }
    fn inbox_unreads(&self, _guild_order: &[Id]) -> Vec<Entry> {
        self.unreads.lock().unwrap().clone()
    }
}

struct Harness {
    window: WindowHandle<PhoenixApp>,
    vcx: VisualTestContext,
    probe: Arc<Probe>,
    channels: Vec<Id>,
}

fn setup(cx: &mut TestAppContext) -> Harness {
    let probe = Arc::new(Probe { mock: MockBackend::synchronous(), acks: AtomicUsize::new(0), leaves: AtomicUsize::new(0), unreads: Mutex::new(Vec::new()) });
    let ready = futures::executor::block_on(probe.mock.login(Credentials::Token("x".into()))).unwrap();
    cx.update(gpui_component::init);
    let backend: Arc<dyn Backend> = probe.clone();
    let window = cx.add_window(|w, cx| PhoenixApp::new(backend, w, cx));
    let mut channels = Vec::new();
    window
        .update(cx, |app, _, cx| {
            app.apply_ready(ready, cx);
            let g = app.guilds[0].id;
            app.select_guild(Some(g), cx);
            channels = app.guild_channels[&g].iter().copied().filter(|c| app.channels[c].kind == ChannelKind::Text).collect();
            app.select_channel(channels[0], cx);
        })
        .unwrap();
    let vcx = VisualTestContext::from_window(window.into(), cx);
    vcx.run_until_parked();
    probe.acks.store(0, Ordering::SeqCst);
    Harness { window, vcx, probe, channels }
}

impl Harness {
    fn act(&mut self, f: impl FnOnce(&mut PhoenixApp, &mut gpui::Window, &mut gpui::Context<PhoenixApp>)) {
        self.window.update(&mut self.vcx, f).unwrap();
        self.vcx.run_until_parked();
    }

    fn read<T>(&mut self, f: impl FnOnce(&PhoenixApp) -> T) -> T {
        self.window.update(&mut self.vcx, |a, _, _| f(a)).unwrap()
    }

    fn bounds(&mut self, selector: &'static str) -> Bounds<Pixels> {
        self.vcx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} not drawn"))
    }

    fn device(&mut self, at: Point<Pixels>) -> Point<Pixels> {
        let k = self.vcx.update(|w, _| w.page_zoom());
        point(at.x * k, at.y * k)
    }

    fn click(&mut self, at: Point<Pixels>) {
        let at = self.device(at);
        self.vcx.simulate_mouse_move(at, None::<gpui::MouseButton>, Modifiers::none());
        self.vcx.run_until_parked();
        self.vcx.simulate_click(at, Modifiers::none());
        self.vcx.run_until_parked();
    }

    fn click_outside(&mut self) {
        let vp = self.vcx.update(|w, _| w.viewport_size());
        self.click(point(px(30.), vp.height - px(30.)));
    }

    fn press_inside(&mut self, selector: &'static str, is_open: fn(&PhoenixApp) -> bool) {
        let pop = self.bounds(selector);
        let at = self.device(header_point(pop));
        let vp = self.vcx.update(|w, _| w.viewport_size());
        self.vcx.simulate_mouse_move(at, None::<gpui::MouseButton>, Modifiers::none());
        self.vcx.run_until_parked();
        self.vcx.simulate_mouse_down(at, gpui::MouseButton::Left, Modifiers::none());
        self.vcx.run_until_parked();
        let after_down = self.read(is_open);
        self.vcx.simulate_mouse_up(at, gpui::MouseButton::Left, Modifiers::none());
        self.vcx.run_until_parked();
        let after_up = self.read(is_open);
        assert!(
            after_down && after_up,
            "press inside {selector} closed it: popout {pop:?}, press {at:?}, viewport {vp:?}, open after down {after_down}, after up {after_up}"
        );
    }

    fn escape(&mut self) {
        self.act(|a, w, cx| a.on_escape(&crate::app::Escape, w, cx));
    }

    fn acks(&self) -> usize {
        self.probe.acks.load(Ordering::SeqCst)
    }
}

fn header_point(b: Bounds<Pixels>) -> Point<Pixels> {
    b.origin + point(px(30.), px(24.))
}

#[gpui::test]
fn inbox_closes_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let open = |h: &mut Harness| {
        h.act(|a, _, cx| {
            a.parity.inbox.tab = crate::commands::state::InboxTab::Mentions;
            a.parity.inbox.open = false;
            a.toggle_inbox(cx);
        });
        assert!(h.read(|a| a.parity.inbox.open));
    };

    open(&mut h);
    let saved = h.bounds("inbox-tab-saved");
    h.click(saved.center());
    let (open_after_tab, tab) = h.read(|a| (a.parity.inbox.open, a.parity.inbox.tab));
    assert!(open_after_tab && tab == crate::commands::state::InboxTab::Saved, "the Saved tab did not get its press (open {open_after_tab}, tab {tab:?})");
    h.press_inside("inbox-popout", |a| a.parity.inbox.open);
    h.click_outside();
    assert!(!h.read(|a| a.parity.inbox.open), "press outside did not close the inbox");
    assert!(!h.read(|a| a.people.status_picker), "the closing press went through");

    open(&mut h);
    h.escape();
    assert!(!h.read(|a| a.parity.inbox.open), "Esc did not close the inbox");

    open(&mut h);
    let btn = h.bounds("title-inbox");
    h.click(btn.center());
    assert!(!h.read(|a| a.parity.inbox.open), "the inbox button did not close the inbox");
    h.click(btn.center());
    assert!(h.read(|a| a.parity.inbox.open), "the inbox button did not reopen the inbox");

    h.vcx.deactivate_window();
    h.vcx.run_until_parked();
    assert!(h.read(|a| a.parity.inbox.open), "window blur closed the inbox");

    assert_eq!(h.acks(), 0, "closing the inbox sent a read marker");
}

#[gpui::test]
fn inbox_filter_menu_closes_first_on_escape(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    h.act(|a, _, cx| {
        a.parity.inbox.tab = crate::commands::state::InboxTab::Mentions;
        a.toggle_inbox(cx);
        a.parity.inbox.filter_menu = true;
    });
    h.escape();
    assert!(h.read(|a| a.parity.inbox.open && !a.parity.inbox.filter_menu));
    h.escape();
    assert!(!h.read(|a| a.parity.inbox.open));
    assert_eq!(h.acks(), 0);
}

#[gpui::test]
fn threads_popout_closes_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let open = |h: &mut Harness| {
        h.act(|a, _, cx| {
            a.parity.threads_open = false;
            a.toggle_threads_popout(cx);
        });
        assert!(h.read(|a| a.parity.threads_open));
    };
    open(&mut h);
    h.press_inside("threads-popout", |a| a.parity.threads_open);
    h.click_outside();
    assert!(!h.read(|a| a.parity.threads_open), "press outside did not close the threads browser");

    open(&mut h);
    h.escape();
    assert!(!h.read(|a| a.parity.threads_open), "Esc did not close the threads browser");

    open(&mut h);
    let btn = h.bounds("hdr-threads");
    h.click(btn.center());
    assert!(!h.read(|a| a.parity.threads_open), "the threads button did not close it");

    open(&mut h);
    let other = h.channels.get(1).copied().unwrap_or(h.channels[0]);
    h.act(|a, _, cx| a.select_channel(other, cx));
    assert!(!h.read(|a| a.parity.threads_open), "switching channel kept the threads browser");
}

#[gpui::test]
fn pins_popout_closes_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let open = |h: &mut Harness| {
        h.act(|a, _, cx| {
            a.chat.pins = None;
            a.toggle_pins(cx);
        });
        assert!(h.read(|a| a.chat.pins.is_some()));
    };
    open(&mut h);
    h.press_inside("pins-popout", |a| a.chat.pins.is_some());
    h.click_outside();
    assert!(h.read(|a| a.chat.pins.is_none()), "press outside did not close the pins");

    open(&mut h);
    h.escape();
    assert!(h.read(|a| a.chat.pins.is_none()), "Esc did not close the pins");

    open(&mut h);
    let btn = h.bounds("hdr-pin");
    h.click(btn.center());
    assert!(h.read(|a| a.chat.pins.is_none()), "the pin button did not close the pins");

    if let Some(other) = h.channels.get(1).copied() {
        open(&mut h);
        h.act(|a, _, cx| a.select_channel(other, cx));
        assert!(h.read(|a| a.chat.pins.is_none()), "switching channel kept the pins");
    }
}

#[gpui::test]
fn account_popout_closes_like_the_client(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let open = |h: &mut Harness| {
        h.act(|a, w, cx| {
            a.people.close_overlays();
            a.toggle_status_picker(w, cx);
        });
        assert!(h.read(|a| a.people.status_picker));
    };

    open(&mut h);
    let vp = h.vcx.update(|w, _| w.viewport_size());
    h.click(point(vp.width / 2., vp.height / 2.));
    assert!(!h.read(|a| a.people.status_picker), "press outside did not close the account popout");

    open(&mut h);
    h.escape();
    assert!(!h.read(|a| a.people.status_picker), "Esc did not close the account popout");

    open(&mut h);
    h.act(|a, w, cx| a.toggle_status_picker(w, cx));
    assert!(!h.read(|a| a.people.status_picker), "the trigger did not close the account popout");
}

#[gpui::test]
fn one_header_popout_at_a_time(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    h.act(|a, _, cx| a.toggle_inbox(cx));
    h.act(|a, _, cx| a.toggle_threads_popout(cx));
    assert!(h.read(|a| a.parity.threads_open && !a.parity.inbox.open));
    h.click_outside();
    assert!(h.read(|a| !a.header_popout_open()));
}

#[gpui::test]
fn unreads_tab_shows_messages_without_acking(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let entries: Vec<Entry> = h
        .channels
        .iter()
        .take(2)
        .enumerate()
        .map(|(n, c)| Entry { channel: *c, guild: None, kind: Kind::Messages, oldest_read: Id(0), newest: Id(u64::MAX), mentions: 0, sort: 5 + n as u8 })
        .collect();
    *h.probe.unreads.lock().unwrap() = entries.clone();
    h.act(|a, _, cx| {
        a.parity.inbox.tab = crate::commands::state::InboxTab::Unreads;
        a.parity.inbox.open = false;
        a.toggle_inbox(cx);
    });
    for _ in 0..10 {
        h.act(|_, _, cx| cx.notify());
    }
    let (order, shown): (Vec<Id>, Vec<usize>) =
        h.read(|a| a.parity.inbox.unreads.iter().filter(|c| c.loaded).map(|c| (c.entry.channel, c.msgs.len())).unzip());
    assert_eq!(order, entries.iter().map(|e| e.channel).collect::<Vec<_>>(), "cards out of the backend order");
    assert!(shown.iter().all(|n| (1..=crate::ui::inbox_unreads::SHOWN).contains(n)), "unexpected message counts {shown:?}");
    assert_eq!(h.acks(), 0, "showing Inbox > Unreads sent a read marker");

    h.act(|a, _, cx| a.mark_inbox_read(cx));
    assert_eq!(h.acks(), 1);
    assert_eq!(h.read(|a| a.parity.inbox.unreads.first().map(|c| c.entry.channel)), Some(entries[1].channel));
}

#[gpui::test]
fn unreads_mark_all_asks_first_and_cards_collapse(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let entries: Vec<Entry> = h
        .channels
        .iter()
        .take(2)
        .map(|c| Entry { channel: *c, guild: None, kind: Kind::Messages, oldest_read: Id(0), newest: Id(u64::MAX), mentions: 0, sort: 5 })
        .collect();
    *h.probe.unreads.lock().unwrap() = entries.clone();
    h.act(|a, _, cx| {
        a.parity.inbox.tab = crate::commands::state::InboxTab::Unreads;
        a.parity.inbox.open = false;
        a.toggle_inbox(cx);
    });
    for _ in 0..10 {
        h.act(|_, _, cx| cx.notify());
    }
    h.act(|a, _, cx| {
        if let Some(c) = a.parity.inbox.unreads.first_mut() {
            c.collapsed = true;
        }
        cx.notify();
    });
    assert_eq!(h.read(|a| a.parity.inbox.unreads.len()), 2);
    assert_eq!(h.acks(), 0, "collapsing a card sent a read marker");

    h.act(|a, _, cx| a.confirm_mark_all_unreads(cx));
    let asked = crate::actions::with_modal(|m| matches!(m, Some(crate::ui::modals::Modal::Form(_))));
    assert!(asked, "Mark All as Read did not ask for confirmation");
    assert_eq!(h.acks(), 0, "Mark All as Read acked before the confirmation");
    crate::actions::close_modal();

    h.act(|a, _, cx| a.mark_all_unread_cards_read(cx));
    assert_eq!(h.acks(), entries.len());
    assert!(h.read(|a| a.parity.inbox.unreads.is_empty()));
}

fn keyed(cx: &mut TestAppContext) -> Harness {
    let mut h = setup(cx);
    h.vcx.update(|_, cx| crate::app::bind_keys(cx));
    h.act(|a, w, cx| a.composer.update(cx, |s, cx| s.focus(w, cx)));
    h
}

impl Harness {
    fn ask<T>(&mut self, f: impl FnOnce(&mut PhoenixApp, &mut gpui::Window, &mut gpui::Context<PhoenixApp>) -> T) -> T {
        let out = self.window.update(&mut self.vcx, f).unwrap();
        self.vcx.run_until_parked();
        out
    }

    fn keys(&mut self, keys: &str) {
        self.vcx.simulate_keystrokes(keys);
        self.vcx.run_until_parked();
    }

    fn make_unread(&mut self) {
        self.act(|a, _, _| {
            let ch = a.selected_channel.unwrap();
            let c = a.channels.get_mut(&ch).unwrap();
            c.unread = true;
            c.muted = false;
        });
    }
}

#[gpui::test]
fn up_edits_the_last_message_the_client_lets_you_edit(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    let here = h.channels[0];
    let (text, voice, pin) = (Id(9_300_101), Id(9_300_102), Id(9_300_103));
    h.act(|a, _, cx| {
        let me = a.me.id;
        a.append_message(Message { id: text, channel: here, author: me, content: "mine".into(), ..Default::default() }, cx);
        let flags = phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE;
        a.append_message(Message { id: voice, channel: here, author: me, flags, ..Default::default() }, cx);
        a.append_message(Message { id: pin, channel: here, author: me, type_id: 6, ..Default::default() }, cx);
    });
    h.act(|a, w, cx| a.composer.update(cx, |s, cx| s.focus(w, cx)));
    h.keys("up");
    assert_eq!(h.read(|a| a.chat.edit.as_ref().map(|e| e.id)), Some(text), "Up did not skip the voice message and the pin notice");
    let mut caret = None;
    h.act(|a, _, cx| caret = a.chat.edit.as_ref().map(|e| e.input.read(cx).cursor()));
    assert_eq!(caret, Some("mine".len()), "the edit caret is not at the end");
}

#[gpui::test]
fn escape_from_the_composer_walks_the_client_stack(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    let here = h.channels[0];
    let id = Id(9_300_001);
    h.act(|a, w, cx| {
        let me = a.me.id;
        a.append_message(Message { id, channel: here, author: me, content: "mine".into(), ..Default::default() }, cx);
        a.start_edit(id, w, cx);
    });
    h.make_unread();
    h.act(|a, w, cx| a.composer.update(cx, |s, cx| s.focus(w, cx)));
    h.act(|a, _, cx| {
        a.replying_to = Some(id);
        a.parity.inbox.tab = crate::commands::state::InboxTab::Mentions;
        a.parity.inbox.open = false;
        a.toggle_inbox(cx);
    });
    assert!(h.read(|a| a.parity.inbox.open));
    let base = h.acks();

    h.keys("escape");
    assert!(!h.read(|a| a.parity.inbox.open), "Esc did not close the inbox first");
    assert!(h.read(|a| a.chat.edit.is_some() && a.replying_to.is_some()));

    h.keys("escape");
    assert!(h.read(|a| a.chat.edit.is_none()), "Esc did not cancel the edit");
    assert!(h.read(|a| a.replying_to.is_some()), "Esc cancelled the reply together with the edit");

    h.act(|a, w, cx| a.composer.update(cx, |s, cx| s.focus(w, cx)));
    h.keys("escape");
    assert!(h.read(|a| a.replying_to.is_none()), "Esc did not cancel the reply");
    assert_eq!(h.acks(), base, "Esc acked before the layers above the chat were closed");

    h.keys("escape");
    assert_eq!(h.acks(), base + 1, "Esc in the chat did not mark the channel read");
    assert!(h.read(|a| !a.channels[&here].unread));

    h.keys("escape");
    assert_eq!(h.acks(), base + 1, "Esc acked a channel that was already read");
}

#[gpui::test]
fn escape_closes_search_and_modals_without_acking(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    let here = h.channels[0];
    h.make_unread();
    let base = h.acks();
    h.act(crate::ui::search::open);
    assert!(h.ask(|_, _, cx| crate::ui::search::is_open(cx)));
    h.act(|a, w, cx| a.composer.update(cx, |s, cx| s.focus(w, cx)));
    h.keys("escape");
    assert!(!h.ask(|_, _, cx| crate::ui::search::is_open(cx)), "Esc did not close the search");

    h.act(|_, _, _| crate::actions::open_modal(crate::actions::Modal::Call { channel: here, user: Id(2) }, None));
    h.keys("escape");
    assert!(crate::actions::with_modal(|m| m.is_none()), "Esc did not close the modal");

    h.act(|_, _, _| crate::shortcuts::open_help());
    h.keys("escape");
    assert!(!crate::shortcuts::help_open(), "Esc did not close the shortcuts modal");
    assert_eq!(h.acks(), base);
}

#[gpui::test]
fn escape_works_after_the_focused_field_goes_away(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    let base = h.acks();
    h.act(|_, w, cx| {
        let gone = cx.focus_handle();
        w.focus(&gone, cx);
    });
    h.act(|a, _, cx| {
        a.parity.inbox.tab = crate::commands::state::InboxTab::Mentions;
        a.parity.inbox.open = false;
        a.toggle_inbox(cx);
    });
    h.keys("escape");
    assert!(!h.read(|a| a.parity.inbox.open), "Esc was lost once the focused element was dropped");
    h.keys("ctrl-/");
    assert!(crate::shortcuts::help_open(), "Ctrl+/ was lost once the focused element was dropped");
    h.keys("escape");
    assert!(!crate::shortcuts::help_open());
    assert_eq!(h.acks(), base);
}

#[gpui::test]
fn shift_escape_only_acks_an_unread_server(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    h.act(|a, _, _| {
        let g = a.selected_guild.unwrap();
        for id in a.guild_channels[&g].clone() {
            if let Some(c) = a.channels.get_mut(&id) {
                c.unread = false;
                c.mentions = 0;
            }
        }
    });
    let base = h.acks();
    h.keys("shift-escape");
    assert_eq!(h.acks(), base, "Shift+Esc acked a server with nothing unread");
    h.make_unread();
    h.keys("shift-escape");
    assert_eq!(h.acks(), base + 1, "Shift+Esc did not mark the server read");
}

#[gpui::test]
fn quit_shortcut_leaves_voice_and_quits(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    let base = h.acks();
    crate::shortcuts::escape::reset_quitting();
    h.act(crate::ui::search::open);
    h.act(|a, _, _| a.voice_channel = Some(Id(424_242)));
    h.keys(crate::shortcuts::QUIT_KEY);
    assert!(crate::shortcuts::escape::quitting(), "{} was eaten by the search field", crate::shortcuts::QUIT_KEY);
    assert_eq!(h.probe.leaves.load(Ordering::SeqCst), 1, "quitting did not leave the voice channel");
    assert!(h.read(|a| a.voice_channel.is_none()));
    assert_eq!(h.acks(), base);
    crate::shortcuts::escape::reset_quitting();
}

#[gpui::test]
fn ctrl_shift_a_collapses_then_expands_every_category(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    let cats = h.read(|a| {
        let g = a.selected_guild.unwrap();
        a.guild_channels[&g].iter().copied().filter(|id| a.channels[id].kind == ChannelKind::Category).collect::<Vec<_>>()
    });
    assert!(!cats.is_empty());
    h.keys("ctrl-shift-a");
    assert!(h.read(|a| cats.iter().all(|c| a.collapsed.contains(c))), "Ctrl+Shift+A did not collapse every category");
    h.keys("ctrl-shift-a");
    assert!(h.read(|a| cats.iter().all(|c| !a.collapsed.contains(c))), "Ctrl+Shift+A did not expand them back");
}

#[gpui::test]
fn ctrl_t_opens_the_quick_switcher_from_the_composer(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    h.keys("ctrl-t");
    assert!(h.read(|a| a.switcher.is_some()), "Ctrl+T did not open the quick switcher");
    h.keys("escape");
    assert!(h.read(|a| a.switcher.is_none()));
}

#[gpui::test]
fn own_status_sent_before_ready_reaches_the_user_panel(cx: &mut TestAppContext) {
    let probe = Arc::new(Probe { mock: MockBackend::synchronous(), acks: AtomicUsize::new(0), leaves: AtomicUsize::new(0), unreads: Mutex::new(Vec::new()) });
    let ready = futures::executor::block_on(probe.mock.login(Credentials::Token("x".into()))).unwrap();
    cx.update(gpui_component::init);
    let backend: Arc<dyn Backend> = probe.clone();
    let window = cx.add_window(|w, cx| PhoenixApp::new(backend, w, cx));
    let me = ready.me.id;
    let status = window
        .update(cx, |app, _, cx| {
            app.handle_event(crate::backend::AppEvent::PresenceUpdate { user: me, presence: crate::model::Presence::Dnd }, cx);
            app.apply_ready(ready, cx);
            app.presence_of(me)
        })
        .unwrap();
    assert_eq!(status, crate::model::Presence::Dnd);
}

type OpenFull = fn(&mut PhoenixApp, Id, Option<Id>, Point<Pixels>, &mut gpui::Window, &mut gpui::Context<PhoenixApp>);

#[gpui::test]
fn own_full_profile_is_the_editor_from_every_entry_point(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let (me, other, guild) = h.read(|a| {
        let me = a.me.id;
        let other = a.users.values().find(|u| u.id != me && !u.bot).map(|u| u.id).unwrap();
        (me, other, a.selected_guild)
    });
    let at = point(px(900.), px(200.));
    let entries: [(&str, OpenFull); 5] = [
        ("popout", |a, u, _, at, w, cx| {
            a.open_profile(u, None, at, w, cx);
            a.open_profile_modal(u, cx);
        }),
        ("member list", |a, u, g, at, w, cx| {
            a.open_profile(u, g, at, w, cx);
            a.open_profile_modal(u, cx);
        }),
        ("dm panel", |a, u, _, _, _, cx| a.open_profile_modal(u, cx)),
        ("mention", |a, u, g, at, w, cx| {
            a.open_profile(u, g, at, w, cx);
            a.open_profile_modal_at(u, crate::ui::profile::TAB_BOARD, cx);
        }),
        ("mod view", |a, u, g, at, w, cx| {
            a.open_profile(u, g, at, w, cx);
            a.open_profile_modal_at(u, 0, cx);
        }),
    ];
    for (name, open) in entries {
        h.act(|a, w, cx| {
            a.settings = None;
            a.people.close_overlays();
            open(a, me, guild, at, w, cx);
        });
        let (modal, editor) = h.read(|a| (a.people.modal, a.settings.as_ref().and_then(|s| s.profile.as_ref()).is_some_and(|e| e.modal)));
        assert_eq!(modal, None, "{name}: own full profile opened the read-only modal");
        assert!(editor, "{name}: own full profile did not open in edit mode");
        assert!(h.read(|a| a.people.popout.is_none()), "{name}: popout left open under the editor");

        h.act(|a, w, cx| {
            a.settings = None;
            a.people.close_overlays();
            open(a, other, guild, at, w, cx);
        });
        let (modal, editor) = h.read(|a| (a.people.modal, a.settings.is_some()));
        assert_eq!(modal, Some(other), "{name}: someone else's full profile did not open");
        assert!(!editor, "{name}: someone else's full profile opened the editor");
    }
}

#[gpui::test]
fn wheel_and_touchpad_scroll_the_chat_from_its_bottom(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    let from_end = |h: &mut Harness| {
        h.read(|a| f32::from(a.message_list.max_offset_for_scrollbar().y) + f32::from(a.message_list.scroll_px_offset_for_scrollbar().y))
    };
    let (max, open_top, count, anchor) = h.read(|a| {
        let l = &a.message_list;
        (f32::from(l.max_offset_for_scrollbar().y), -f32::from(l.scroll_px_offset_for_scrollbar().y), l.item_count(), l.logical_scroll_top().item_ix)
    });
    assert!(max > 200., "the mock chat must have room to scroll, max offset {max}");
    assert_eq!((open_top, anchor), (max, count), "the chat must open at its bottom");
    let chat = h.read(|a| a.message_list.viewport_bounds());
    let at = h.device(chat.center());
    h.vcx.simulate_mouse_move(at, None::<gpui::MouseButton>, Modifiers::none());
    h.vcx.run_until_parked();

    h.vcx.simulate_event(gpui::ScrollEvent { position: at, delta: gpui::ScrollDelta::Pixels(point(px(0.), px(120.))), ..Default::default() });
    h.vcx.run_until_parked();
    let after = from_end(&mut h);
    assert!(after > 100. && after < 140., "a 120 px touchpad scroll left the chat {after} px from its end");
    assert!(h.read(|a| a.message_list.logical_scroll_top().item_ix) < count, "the chat went back to its end");

    h.act(|a, _, _| a.message_list.scroll_to_end());
    assert!(from_end(&mut h) < 0.5);
    h.vcx.simulate_event(gpui::ScrollEvent { position: at, delta: gpui::ScrollDelta::Lines(point(0., 3.)), ..Default::default() });
    h.vcx.run_until_parked();
    for _ in 0..25 {
        std::thread::sleep(std::time::Duration::from_millis(12));
        h.vcx.executor().advance_clock(std::time::Duration::from_millis(16));
        h.vcx.update(|w, cx| {
            w.simulate_next_frame(cx);
        });
        h.vcx.run_until_parked();
    }
    let after = from_end(&mut h);
    assert!(after > 40., "a wheel notch left the chat {after} px from its end");
    assert!(!h.read(|a| a.chat.at_bottom), "the jump to present bar must follow the wheel");
}

#[gpui::test]
fn pin_and_delete_confirm_modals_are_drawn_inside_the_window(cx: &mut TestAppContext) {
    let mut h = setup(cx);
    h.vcx.update(|_, cx| crate::media::lru::init(cx));
    let ch = h.channels[0];
    let vp = h.vcx.update(|w, _| w.viewport_size());
    let inside = |b: Bounds<Pixels>| b.size.width > px(0.) && b.size.height > px(0.) && b.origin.y >= px(0.) && b.origin.y + b.size.height <= vp.height;
    for unpin in [false, true] {
        h.act(|a, _, cx| {
            let mut m = a.messages[&ch].msgs.iter().rev().find(|m| !m.content.is_empty()).cloned().unwrap();
            m.pinned = unpin;
            a.request_pin_message(ch, m, false, cx);
        });
        let b = h.bounds("modal-card");
        assert!(inside(b), "pin modal at {b:?} outside {vp:?}");
        h.act(|a, _, cx| {
            a.chat.confirm_pin = None;
            cx.notify();
        });
    }
    h.act(|a, _, cx| {
        let id = a.messages[&ch].msgs.last().unwrap().id;
        a.chat.confirm_delete = Some((ch, id));
        cx.notify();
    });
    let b = h.bounds("modal-card");
    assert!(inside(b), "delete modal at {b:?} outside {vp:?}");
}

#[gpui::test]
fn escape_closes_the_search_and_brings_the_hidden_thread_back(cx: &mut TestAppContext) {
    let mut h = keyed(cx);
    let t = h.channels[1];
    h.act(|a, w, cx| a.open_thread(crate::model::ThreadRef { id: t, name: "t".into(), message_count: 0 }, w, cx));
    h.act(crate::ui::search::open);
    h.act(|_, _, cx| {
        if let Some(s) = cx.global_mut::<crate::ui::search::Search>().0.as_mut() {
            s.request = Some(crate::ui::search::SearchRequest { content: Some("mire".into()), ..Default::default() });
            s.page = Some(crate::ui::search::SearchPage::default());
        }
    });
    assert!(h.ask(|_, _, cx| crate::ui::search::results_shown(cx)));
    h.act(crate::ui::search::open);
    assert!(h.ask(|_, w, cx| crate::ui::search::is_focused(w, cx)));
    h.keys("escape");
    assert!(!h.ask(|_, _, cx| crate::ui::search::is_open(cx)), "Esc did not close the search");
    assert!(h.read(|a| a.chat.thread.as_ref().is_some_and(|p| p.thread.id == t)), "Esc closed the thread hidden behind the search");
}

#[test]
fn header_tooltip_hides_on_click_until_the_pointer_leaves() {
    let k = crate::ui::states::key(("hdr-btn", "pin-test"));
    crate::ui::states::set_hover(k, true);
    assert!(crate::ui::states::tip_shown(k));
    crate::ui::states::hide_tip_until_leave(k);
    assert!(!crate::ui::states::tip_shown(k), "the tooltip stays over the opened popout");
    crate::ui::states::set_hover(k, false);
    crate::ui::states::set_hover(k, true);
    assert!(crate::ui::states::tip_shown(k), "hovering again shows the tooltip");
}
