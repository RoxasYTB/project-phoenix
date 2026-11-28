use std::cell::RefCell;

use gpui::{
    App, ClipboardItem, Context, FontWeight, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Pixels, SharedString, Stateful, Styled, actions, div, prelude::*, px,
};

pub mod escape;
pub mod message;

use crate::{
    app::{CONTEXT, PhoenixApp},
    model::{ChannelKind, Id},
    theme::{c, generated::idx, tok, z},
    ui,
};

actions!(
    phoenix_shortcuts,
    [
        ToggleHelp,
        PrevChannel,
        NextChannel,
        PrevUnread,
        NextUnread,
        PrevMention,
        NextMention,
        PrevGuild,
        NextGuild,
        ToggleLastGuild,
        HistoryBack,
        HistoryForward,
        ToggleInbox,
        TogglePins,
        UploadFile,
        FocusSearch,
        ToggleMute,
        ToggleDeafen,
        ToggleEmoji,
        ToggleGif,
        ToggleSticker,
        PageUp,
        PageDown,
        ChatTop,
        ChatBottom,
        JumpOldestUnread,
        MarkServerRead,
        CreateGuild,
        CreateGroupDm,
        CopyChannelLink,
        GetHelp,
        MarkInboxRead,
        CallDm,
        JumpToCall,
        CreateThread,
        RecordVoiceMessage,
        ToggleSoundboard,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        ToggleFullscreen,
        QuickSwitcherAlt,
        JumpToPresent,
        ToggleAllCategories,
        JumpToVoiceGuild,
        Quit,
    ]
);

pub const ZOOM_SCALES: [u32; 11] = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

pub fn zoom_step(cur: u32, dir: i32) -> u32 {
    if dir > 0 {
        ZOOM_SCALES.iter().copied().find(|&z| z > cur).unwrap_or(ZOOM_SCALES[ZOOM_SCALES.len() - 1])
    } else {
        ZOOM_SCALES.iter().rev().copied().find(|&z| z < cur).unwrap_or(ZOOM_SCALES[0])
    }
}

pub struct Shortcut {
    pub keys: &'static [&'static str],
    pub group: Group,
    pub label: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Navigation,
    Voice,
    Chat,
    Misc,
}

impl Group {
    fn title(self) -> SharedString {
        match self {
            Group::Navigation => crate::t!("shortcuts.group.navigation"),
            Group::Voice => crate::t!("shortcuts.group.voice"),
            Group::Chat => crate::t!("shortcuts.group.chat"),
            Group::Misc => crate::t!("shortcuts.group.misc"),
        }
    }
}

macro_rules! sc {
    ($group:ident, $label:literal, [$($k:literal),+]) => {
        Shortcut { keys: &[$($k),+], group: Group::$group, label: $label }
    };
}

pub const SHORTCUTS: &[Shortcut] = &[
    sc!(Navigation, "shortcuts.guilds", ["ctrl-alt-up", "ctrl-alt-down"]),
    sc!(Navigation, "shortcuts.channels", ["alt-up", "alt-down"]),
    sc!(Navigation, "shortcuts.history", ["alt-left", "alt-right"]),
    sc!(Navigation, "shortcuts.unread", ["alt-shift-up", "alt-shift-down"]),
    sc!(Navigation, "shortcuts.mentions", ["ctrl-shift-alt-up", "ctrl-shift-alt-down"]),
    sc!(Navigation, "shortcuts.jump_call", ["ctrl-shift-alt-v"]),
    sc!(Navigation, "shortcuts.last_guild", ["ctrl-alt-right"]),
    sc!(Navigation, "shortcuts.switcher", ["ctrl-k"]),
    sc!(Navigation, "shortcuts.create_guild", ["ctrl-shift-n"]),
    sc!(Voice, "shortcuts.mute", ["ctrl-shift-m"]),
    sc!(Voice, "shortcuts.deafen", ["ctrl-shift-d"]),
    sc!(Voice, "shortcuts.call", ["ctrl-'"]),
    sc!(Voice, "shortcuts.soundboard", ["ctrl-shift-b"]),
    sc!(Chat, "shortcuts.mark_server_read", ["shift-escape"]),
    sc!(Chat, "shortcuts.mark_channel_read", ["escape"]),
    sc!(Chat, "shortcuts.group_dm", ["ctrl-shift-t"]),
    sc!(Chat, "shortcuts.thread", ["ctrl-shift-alt-n"]),
    sc!(Chat, "shortcuts.pins", ["ctrl-p"]),
    sc!(Chat, "shortcuts.inbox", ["ctrl-i"]),
    sc!(Chat, "shortcuts.inbox_read", ["ctrl-shift-e"]),
    sc!(Chat, "shortcuts.members", ["ctrl-u"]),
    sc!(Chat, "shortcuts.emoji", ["ctrl-e"]),
    sc!(Chat, "shortcuts.gif", ["ctrl-g"]),
    sc!(Chat, "shortcuts.sticker", ["ctrl-s"]),
    sc!(Chat, "shortcuts.scroll", ["pageup", "pagedown"]),
    sc!(Chat, "shortcuts.oldest_unread", ["shift-pageup"]),
    sc!(Chat, "shortcuts.voice_message", ["alt-r"]),
    sc!(Chat, "shortcuts.upload", ["ctrl-shift-u"]),
    sc!(Chat, "shortcuts.copy_link", ["ctrl-shift-l"]),
    sc!(Misc, "shortcuts.help", ["ctrl-shift-h"]),
    sc!(Misc, "shortcuts.search", ["ctrl-f"]),
    sc!(Misc, "shortcuts.fullscreen", ["f11"]),
    sc!(Misc, "shortcuts.modal", ["ctrl-/"]),
];

pub const QUIT_KEY: &str = if cfg!(target_os = "macos") {
    "cmd-q"
} else if cfg!(target_os = "windows") {
    "alt-f4"
} else {
    "ctrl-q"
};

const COMPOSER: Option<&str> = Some("Composer > Input");

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(CONTEXT);
    cx.on_action(|_: &Quit, cx| escape::quit_app(cx));
    cx.bind_keys([
        KeyBinding::new(QUIT_KEY, Quit, None),
        KeyBinding::new("ctrl-/", ToggleHelp, ctx),
        KeyBinding::new("ctrl-shift-/", ToggleHelp, ctx),
        KeyBinding::new("alt-up", PrevChannel, ctx),
        KeyBinding::new("alt-down", NextChannel, ctx),
        KeyBinding::new("alt-shift-up", PrevUnread, ctx),
        KeyBinding::new("alt-shift-down", NextUnread, ctx),
        KeyBinding::new("ctrl-shift-alt-up", PrevMention, ctx),
        KeyBinding::new("ctrl-shift-alt-down", NextMention, ctx),
        KeyBinding::new("ctrl-alt-up", PrevGuild, ctx),
        KeyBinding::new("ctrl-shift-[", PrevGuild, ctx),
        KeyBinding::new("ctrl-shift-tab", PrevGuild, ctx),
        KeyBinding::new("ctrl-alt-down", NextGuild, ctx),
        KeyBinding::new("ctrl-shift-]", NextGuild, ctx),
        KeyBinding::new("ctrl-tab", NextGuild, ctx),
        KeyBinding::new("ctrl-alt-right", ToggleLastGuild, ctx),
        KeyBinding::new("alt-left", HistoryBack, ctx),
        KeyBinding::new("alt-right", HistoryForward, ctx),
        KeyBinding::new("ctrl-i", ToggleInbox, ctx),
        KeyBinding::new("ctrl-p", TogglePins, ctx),
        KeyBinding::new("ctrl-shift-u", UploadFile, ctx),
        KeyBinding::new("ctrl-f", FocusSearch, ctx),
        KeyBinding::new("ctrl-shift-f", FocusSearch, ctx),
        KeyBinding::new("ctrl-shift-m", ToggleMute, ctx),
        KeyBinding::new("ctrl-shift-d", ToggleDeafen, ctx),
        KeyBinding::new("ctrl-e", ToggleEmoji, ctx),
        KeyBinding::new("ctrl-g", ToggleGif, ctx),
        KeyBinding::new("ctrl-s", ToggleSticker, ctx),
        KeyBinding::new("pageup", PageUp, COMPOSER),
        KeyBinding::new("pagedown", PageDown, COMPOSER),
        KeyBinding::new("shift-pageup", JumpOldestUnread, COMPOSER),
        KeyBinding::new("shift-pagedown", JumpToPresent, COMPOSER),
        KeyBinding::new("pageup", PageUp, ctx),
        KeyBinding::new("pagedown", PageDown, ctx),
        KeyBinding::new("ctrl-home", ChatTop, ctx),
        KeyBinding::new("ctrl-end", ChatBottom, ctx),
        KeyBinding::new("shift-pageup", JumpOldestUnread, ctx),
        KeyBinding::new("shift-escape", MarkServerRead, ctx),
        KeyBinding::new("ctrl-shift-n", CreateGuild, ctx),
        KeyBinding::new("ctrl-shift-t", CreateGroupDm, ctx),
        KeyBinding::new("ctrl-shift-l", CopyChannelLink, ctx),
        KeyBinding::new("ctrl-shift-h", GetHelp, ctx),
        KeyBinding::new("f1", GetHelp, ctx),
        KeyBinding::new("ctrl-shift-e", MarkInboxRead, ctx),
        KeyBinding::new("ctrl-'", CallDm, ctx),
        KeyBinding::new("ctrl-shift-'", CallDm, ctx),
        KeyBinding::new("ctrl-shift-alt-v", JumpToCall, ctx),
        KeyBinding::new("ctrl-shift-alt-n", CreateThread, ctx),
        KeyBinding::new("alt-r", RecordVoiceMessage, ctx),
        KeyBinding::new("ctrl-shift-b", ToggleSoundboard, ctx),
        KeyBinding::new("ctrl-=", ZoomIn, ctx),
        KeyBinding::new("ctrl-+", ZoomIn, ctx),
        KeyBinding::new("ctrl-shift-=", ZoomIn, ctx),
        KeyBinding::new("ctrl-shift-+", ZoomIn, ctx),
        KeyBinding::new("ctrl--", ZoomOut, ctx),
        KeyBinding::new("ctrl-shift--", ZoomOut, ctx),
        KeyBinding::new("ctrl-0", ZoomReset, ctx),
        KeyBinding::new("ctrl-shift-0", ZoomReset, ctx),
        KeyBinding::new("ctrl-à", ZoomReset, ctx),
        KeyBinding::new("f11", ToggleFullscreen, ctx),
        KeyBinding::new("ctrl-t", QuickSwitcherAlt, ctx),
        KeyBinding::new("shift-pagedown", JumpToPresent, ctx),
        KeyBinding::new("ctrl-shift-a", ToggleAllCategories, ctx),
        KeyBinding::new("ctrl-alt-left", JumpToVoiceGuild, ctx),
    ]);
}

pub fn cycle<T: Copy + PartialEq>(list: &[T], cur: Option<T>, step: i32) -> Option<T> {
    if list.is_empty() {
        return None;
    }
    let n = list.len() as i32;
    let i = match cur.and_then(|c| list.iter().position(|x| *x == c)) {
        Some(i) => (i as i32 + step).rem_euclid(n),
        None if step < 0 => n - 1,
        None => 0,
    };
    Some(list[i as usize])
}

pub fn seek<T: Copy + PartialEq>(list: &[T], cur: Option<T>, step: i32, pred: impl Fn(T) -> bool) -> Option<T> {
    let n = list.len() as i32;
    let start = cur.and_then(|c| list.iter().position(|x| *x == c)).map(|i| i as i32);
    (1..=n)
        .map(|k| match start {
            Some(s) => (s + step.signum() * k).rem_euclid(n),
            None if step < 0 => n - k,
            None => k - 1,
        })
        .map(|i| list[i as usize])
        .find(|x| Some(*x) != cur && pred(*x))
}

pub fn collapse_all_target(categories: &[Id], collapsed: &std::collections::HashSet<Id>) -> bool {
    categories.iter().any(|c| !collapsed.contains(c))
}

pub fn rail_order(guilds: &[Id], folders: &[Vec<Id>]) -> Vec<Option<Id>> {
    let in_folder = |g: &Id| folders.iter().any(|f| f.contains(g));
    let mut out = vec![None];
    out.extend(guilds.iter().filter(|g| !in_folder(g)).map(|g| Some(*g)));
    for f in folders {
        out.extend(f.iter().filter(|g| guilds.contains(g)).map(|g| Some(*g)));
    }
    out
}

#[derive(Default, Debug)]
pub struct History {
    pub past: Vec<Id>,
    pub future: Vec<Id>,
    pub current: Option<Id>,
}

impl History {
    pub fn visit(&mut self, ch: Id) {
        if self.current == Some(ch) {
            return;
        }
        if let Some(c) = self.current.replace(ch) {
            self.past.push(c);
            if self.past.len() > 50 {
                self.past.remove(0);
            }
        }
        self.future.clear();
    }

    pub fn back(&mut self) -> Option<Id> {
        let to = self.past.pop()?;
        if let Some(c) = self.current.replace(to) {
            self.future.push(c);
        }
        Some(to)
    }

    pub fn forward(&mut self) -> Option<Id> {
        let to = self.future.pop()?;
        if let Some(c) = self.current.replace(to) {
            self.past.push(c);
        }
        Some(to)
    }
}

#[derive(Default)]
struct State {
    help_open: bool,
    keys: Option<gpui::Subscription>,
    app: Option<gpui::WeakEntity<PhoenixApp>>,
    window: Option<gpui::AnyWindowHandle>,
    history: History,
    last_guild: Option<Id>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn main_app() -> Option<gpui::Entity<PhoenixApp>> {
    with(|s| s.app.clone())?.upgrade()
}

pub fn is_main_window(window: &gpui::Window) -> bool {
    with(|s| s.window) == Some(window.window_handle())
}

pub const SUPPORT_GUILD: Id = Id(1_574_895_284_901_777_448);
pub const SUPPORT_CHANNEL: Id = Id(1_574_895_979_386_380_461);
pub const SUPPORT_INVITE: &str = "https://discord.gg/uF3wtckHkN";

pub fn support_is_joined(mut guilds: impl Iterator<Item = Id>) -> bool {
    guilds.any(|g| g == SUPPORT_GUILD)
}

impl PhoenixApp {
    pub fn open_support(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if support_is_joined(self.guilds.iter().map(|g| g.id)) {
            self.go_to_invite(SUPPORT_GUILD, Some(SUPPORT_CHANNEL), cx);
            return;
        }
        self.open_add_server(window, cx);
        ui::add_server::set_step(ui::add_server::Step::Join);
        if let Some(input) = self.nav.invite_input.clone() {
            input.update(cx, |s, cx| {
                s.set_value(SUPPORT_INVITE, window, cx);
                s.focus(window, cx);
            });
            self.preview_invite(cx);
        } else {
            cx.open_url(SUPPORT_INVITE);
        }
        cx.notify();
    }
}

pub fn open_help() {
    with(|s| s.help_open = true);
}

pub fn help_open() -> bool {
    with(|s| s.help_open)
}

pub fn on_escape() -> bool {
    with(|s| std::mem::take(&mut s.help_open))
}

fn texty(kind: ChannelKind) -> bool {
    !matches!(kind, ChannelKind::Voice | ChannelKind::Stage | ChannelKind::Category)
}

impl PhoenixApp {
    fn shortcut_channels(&self) -> Vec<Id> {
        let Some(g) = self.selected_guild else { return self.dms.clone() };
        let ok = |id: &Id| {
            self.channels.get(id).is_some_and(|c| c.guild == Some(g) && (texty(c.kind) || self.voice_channel == Some(*id)))
        };
        let rows: Vec<Id> = self
            .sidebar_rows
            .iter()
            .filter_map(|r| match r {
                crate::app::SidebarRow::Channel(id) | crate::app::SidebarRow::Thread(id) => Some(*id),
                _ => None,
            })
            .filter(ok)
            .collect();
        if rows.is_empty() { self.guild_channels.get(&g).into_iter().flatten().copied().filter(ok).collect() } else { rows }
    }

    fn shortcut_rail(&self) -> Vec<Option<Id>> {
        let guilds: Vec<Id> = self.guilds.iter().map(|g| g.id).collect();
        let folders: Vec<Vec<Id>> = self.nav.folders.iter().map(|f| f.guilds.clone()).collect();
        rail_order(&guilds, &folders)
    }

    fn shortcut_all_channels(&self) -> Vec<Id> {
        let mut out = Vec::new();
        for g in self.shortcut_rail() {
            match g {
                _ if g == self.selected_guild => out.extend(self.shortcut_channels()),
                None => out.extend(self.dms.iter().copied()),
                Some(g) => out.extend(
                    self.guild_channels.get(&g).into_iter().flatten().copied().filter(|id| self.channels.get(id).is_some_and(|c| texty(c.kind))),
                ),
            }
        }
        out
    }

    fn goto_channel(&mut self, id: Id, cx: &mut Context<Self>) {
        self.open_channel_chat(id, cx);
        cx.notify();
    }

    fn step_channel(&mut self, step: i32, cx: &mut Context<Self>) {
        if let Some(id) = cycle(&self.shortcut_channels(), self.selected_channel, step) {
            self.goto_channel(id, cx);
        }
    }

    fn step_unread(&mut self, step: i32, mentions_only: bool, cx: &mut Context<Self>) {
        let list = self.shortcut_all_channels();
        let pred = |id: Id| {
            self.channels.get(&id).is_some_and(|c| if mentions_only { c.mentions > 0 } else { (c.unread && !c.muted) || c.mentions > 0 })
        };
        if let Some(id) = seek(&list, self.selected_channel, step, pred) {
            self.goto_channel(id, cx);
        }
    }

    fn step_guild(&mut self, step: i32, cx: &mut Context<Self>) {
        if let Some(g) = cycle(&self.shortcut_rail(), Some(self.selected_guild), step) {
            self.select_guild(g, cx);
        }
    }

    fn toggle_last_guild(&mut self, cx: &mut Context<Self>) {
        let target = match self.selected_guild {
            Some(g) => {
                with(|s| s.last_guild = Some(g));
                None
            }
            None => match with(|s| s.last_guild) {
                Some(g) => Some(g),
                None => return,
            },
        };
        self.select_guild(target, cx);
    }

    pub(crate) fn history_step(&mut self, back: bool, cx: &mut Context<Self>) {
        let to = with(|s| if back { s.history.back() } else { s.history.forward() });
        if let Some(id) = to.filter(|id| self.channels.contains_key(id)) {
            self.goto_channel(id, cx);
        }
    }

    fn page(&mut self, up: bool, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let reduced = ui::settings::effects::reduced_motion();
        ui::chat::page_scroll::page(&self.message_list, up, cx.entity_id(), window, reduced);
    }

    fn toggle_all_categories(&mut self, cx: &mut Context<Self>) {
        let Some(g) = self.selected_guild else { return };
        let cats: Vec<Id> = self
            .guild_channels
            .get(&g)
            .into_iter()
            .flatten()
            .copied()
            .filter(|c| self.channels.get(c).is_some_and(|c| c.kind == ChannelKind::Category))
            .collect();
        if collapse_all_target(&cats, &self.collapsed) {
            self.collapsed.extend(cats);
        } else {
            for c in &cats {
                self.collapsed.remove(c);
            }
        }
        self.rebuild_sidebar();
        self.save_nav_prefs();
        cx.notify();
    }

    fn jump_to_voice_guild(&mut self, cx: &mut Context<Self>) {
        let Some(v) = self.voice_channel else { return };
        let guild = self.channels.get(&v).and_then(|c| c.guild);
        if self.selected_guild != guild || self.favorites_view {
            self.select_guild(guild, cx);
        }
        if let Some(ix) = self.sidebar_rows.iter().position(|r| matches!(r, crate::app::SidebarRow::Channel(id) if *id == v)) {
            self.sidebar_list.scroll_to_reveal_item(ix + 1);
        }
        cx.notify();
    }

    fn copy_channel_link(&self, cx: &mut Context<Self>) {
        let Some(ch) = self.selected_channel else { return };
        let guild = self.channels.get(&ch).and_then(|c| c.guild).map_or("@me".to_string(), |g| g.0.to_string());
        cx.write_to_clipboard(ClipboardItem::new_string(format!("https://discord.com/channels/{guild}/{}", ch.0)));
    }
}

pub fn handlers(app: &PhoenixApp, root: Stateful<gpui::Div>, window: &gpui::Window, cx: &mut Context<PhoenixApp>) -> Stateful<gpui::Div> {
    let handle = window.window_handle();
    let me = cx.entity().downgrade();
    with(|s| {
        s.window = Some(handle);
        s.app = Some(me);
    });
    crate::keybinds::listen(cx);
    crate::keybinds::sync_global(app);
    if let Some(ch) = app.selected_channel {
        with(|s| s.history.visit(ch));
    }
    if let Some(g) = app.selected_guild {
        with(|s| s.last_guild = Some(g));
    }
    if with(|s| s.keys.is_none()) {
        let weak = cx.entity().downgrade();
        let sub = cx.intercept_keystrokes(move |ev, window, cx| {
            if ui::pickers::take_routed() {
                return;
            }
            if is_main_window(window) && ui::context_menu::on_key(&ev.keystroke, window, cx) {
                cx.stop_propagation();
                return;
            }
            let ks = &ev.keystroke;
            if is_main_window(window)
                && ks.key == "c"
                && (ks.modifiers.control || ks.modifiers.platform)
                && !ks.modifiers.shift
                && !ks.modifiers.alt
                && ui::chat::selection::copy(cx)
            {
                cx.stop_propagation();
                return;
            }
            let Some(app) = weak.upgrade() else { return };
            app.update(cx, |app, cx| {
                if app.shortcut_keystroke(&ev.keystroke, window, cx) {
                    cx.stop_propagation();
                }
            });
        });
        with(|s| s.keys = Some(sub));
    }
    use ui::pickers::{EmojiTarget, Tab};
    root.on_action(cx.listener(|_, _: &ToggleHelp, _, cx| {
        with(|s| s.help_open = !s.help_open);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &PrevChannel, _, cx| a.step_channel(-1, cx)))
    .on_action(cx.listener(|a, _: &NextChannel, _, cx| a.step_channel(1, cx)))
    .on_action(cx.listener(|a, _: &PrevUnread, _, cx| a.step_unread(-1, false, cx)))
    .on_action(cx.listener(|a, _: &NextUnread, _, cx| a.step_unread(1, false, cx)))
    .on_action(cx.listener(|a, _: &PrevMention, _, cx| a.step_unread(-1, true, cx)))
    .on_action(cx.listener(|a, _: &NextMention, _, cx| a.step_unread(1, true, cx)))
    .on_action(cx.listener(|a, _: &PrevGuild, _, cx| a.step_guild(-1, cx)))
    .on_action(cx.listener(|a, _: &NextGuild, _, cx| a.step_guild(1, cx)))
    .on_action(cx.listener(|a, _: &ToggleLastGuild, _, cx| a.toggle_last_guild(cx)))
    .on_action(cx.listener(|a, _: &HistoryBack, _, cx| a.history_step(true, cx)))
    .on_action(cx.listener(|a, _: &HistoryForward, _, cx| a.history_step(false, cx)))
    .on_action(cx.listener(|a, _: &ToggleInbox, _, cx| a.toggle_inbox(cx)))
    .on_action(cx.listener(|a, _: &TogglePins, _, cx| a.toggle_pins(cx)))
    .on_action(cx.listener(|a, _: &UploadFile, _, cx| {
        if a.selected_channel.is_some() {
            a.prompt_attach(cx)
        }
    }))
    .on_action(cx.listener(|a, _: &FocusSearch, w, cx| ui::search::open(a, w, cx)))
    .on_action(cx.listener(|a, _: &ToggleMute, _, cx| {
        a.mic_muted = !a.mic_muted;
        crate::voice::sync_self_state(a);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &ToggleDeafen, _, cx| {
        a.deafened = !a.deafened;
        crate::voice::sync_self_state(a);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &ToggleEmoji, w, cx| ui::pickers::toggle(a, Tab::Emoji, EmojiTarget::Composer, w, cx)))
    .on_action(cx.listener(|a, _: &ToggleGif, w, cx| ui::pickers::toggle(a, Tab::Gif, EmojiTarget::Composer, w, cx)))
    .on_action(cx.listener(|a, _: &ToggleSticker, w, cx| ui::pickers::toggle(a, Tab::Sticker, EmojiTarget::Composer, w, cx)))
    .on_action(cx.listener(|a, _: &PageUp, w, cx| a.page(true, w, cx)))
    .on_action(cx.listener(|a, _: &PageDown, w, cx| a.page(false, w, cx)))
    .on_action(cx.listener(|a, _: &ChatTop, w, cx| {
        let reduced = ui::settings::effects::reduced_motion();
        ui::chat::page_scroll::to_top(&a.message_list, cx.entity_id(), w, reduced);
    }))
    .on_action(cx.listener(|a, _: &ChatBottom, _, cx| {
        ui::chat::page_scroll::stop();
        a.jump_to_present(cx);
    }))
    .on_action(cx.listener(|a, _: &JumpOldestUnread, _, cx| {
        if let Some((ch, msg)) = a.chat.unread_marker {
            a.jump_to_message(ch, msg, cx);
        }
    }))
    .on_action(cx.listener(|a, _: &MarkServerRead, _, cx| a.mark_server_read_shortcut(cx)))
    .on_action(cx.listener(|a, _: &CreateGuild, w, cx| a.open_join_server(w, cx)))
    .on_action(cx.listener(|a, _: &CreateGroupDm, _, cx| {
        if a.screen == crate::app::Screen::Login {
            crate::ui::login::toggle_token(a, cx);
        } else {
            a.open_group_dm_picker(cx);
        }
    }))
    .on_action(cx.listener(|a, _: &CopyChannelLink, _, cx| a.copy_channel_link(cx)))
    .on_action(cx.listener(|a, _: &MarkInboxRead, _, cx| a.mark_inbox_read(cx)))
    .on_action(cx.listener(|a, _: &CallDm, _, cx| a.call_current_dm(cx)))
    .on_action(cx.listener(|a, _: &JumpToCall, _, cx| a.jump_to_call(cx)))
    .on_action(cx.listener(|a, _: &CreateThread, w, cx| a.create_thread_shortcut(w, cx)))
    .on_action(cx.listener(|a, _: &ZoomIn, _, cx| ui::settings::update_local(a, |p| p.zoom = zoom_step(p.zoom, 1), cx)))
    .on_action(cx.listener(|a, _: &ZoomOut, _, cx| ui::settings::update_local(a, |p| p.zoom = zoom_step(p.zoom, -1), cx)))
    .on_action(cx.listener(|a, _: &ZoomReset, _, cx| ui::settings::update_local(a, |p| p.zoom = 100, cx)))
    .on_action(cx.listener(|_, _: &ToggleFullscreen, w, _| w.toggle_fullscreen()))
    .on_action(cx.listener(|a, _: &RecordVoiceMessage, _, cx| crate::ui::chat::voice_message::start(a, cx)))
    .on_action(cx.listener(|a, _: &ToggleSoundboard, w, cx| {
        if a.voice_channel.is_some() {
            crate::ui::pickers::soundboard::toggle(a, w, cx);
        }
    }))
    .on_action(cx.listener(|a, _: &GetHelp, w, cx| a.open_support(w, cx)))
    .on_action(cx.listener(|a, _: &QuickSwitcherAlt, w, cx| a.open_switcher(w, cx)))
    .on_action(cx.listener(|a, _: &JumpToPresent, w, cx| {
        a.escape_stack(w, cx);
        cx.notify();
    }))
    .on_action(cx.listener(|a, _: &ToggleAllCategories, _, cx| a.toggle_all_categories(cx)))
    .on_action(cx.listener(|a, _: &JumpToVoiceGuild, _, cx| a.jump_to_voice_guild(cx)))
}

pub const MESSAGE_SHORTCUTS: &[(&str, &str)] = &[
    ("e", "shortcuts.msg.edit"),
    ("backspace", "shortcuts.msg.delete"),
    ("p", "shortcuts.msg.pin"),
    ("+", "shortcuts.msg.react"),
    ("r", "shortcuts.msg.reply"),
    ("f", "shortcuts.msg.forward"),
    ("s", "shortcuts.msg.speak"),
    ("ctrl-c", "shortcuts.msg.copy"),
    ("alt-enter", "shortcuts.msg.unread"),
    ("escape", "shortcuts.msg.composer"),
];

pub fn key_caps(keys: &str) -> Vec<SharedString> {
    if keys == "+" {
        return vec!["+".into()];
    }
    keys.split('-')
        .map(|k| match k {
            "ctrl" => crate::t!("shortcuts.key.ctrl"),
            "shift" => crate::t!("shortcuts.key.shift"),
            "alt" => "ALT".into(),
            "escape" => crate::t!("shortcuts.key.escape"),
            "pageup" => crate::t!("shortcuts.key.pageup"),
            "pagedown" => crate::t!("shortcuts.key.pagedown"),
            "backspace" => crate::t!("shortcuts.key.backspace"),
            "enter" => crate::t!("shortcuts.key.enter"),
            "up" => "↑".into(),
            "down" => "↓".into(),
            "left" => "←".into(),
            "right" => "→".into(),
            k => k.to_uppercase().into(),
        })
        .collect()
}

pub fn combo(keys: &str) -> gpui::Div {
    let caps = key_caps(keys);
    let n = caps.len();
    div()
        .flex()
        .h(z(23.))
        .children(caps.into_iter().enumerate().map(move |(i, k)| {
            div()
                .h(z(23.))
                .min_w(z(14.))
                .pt(z(3.))
                .px(z(6.))
                .pb(z(4.))
                .when(i + 1 < n, |d| d.mr(z(3.)))
                .rounded(z(4.))
                .border_1()
                .border_color(c(tok(idx::BORDER_SUBTLE)))
                .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
                .text_size(z(12.))
                .line_height(z(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::INTERACTIVE_TEXT_ACTIVE)))
                .flex()
                .justify_center()
                .child(k)
        }))
}

fn keybind_card(label: SharedString, keys: &[&str], width: Pixels) -> impl IntoElement {
    let mut combos = div().flex().flex_wrap().mb(z(8.));
    for (i, k) in keys.iter().enumerate() {
        combos = combos.child(combo(k).when(i > 0, |d| d.ml(z(8.))));
    }
    div()
        .w(width)
        .p(z(12.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
        .flex()
        .flex_col()
        .gap(z(8.))
        .child(div().text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(label))
        .child(combos)
}

fn section(title: SharedString, desc: Option<SharedString>, rows: Vec<gpui::AnyElement>) -> impl IntoElement {
    div()
        .ml(z(16.))
        .px(z(8.))
        .py(z(24.))
        .border_b_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .child(
            div()
                .text_size(z(20.))
                .line_height(z(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .child(title),
        )
        .when_some(desc, |d, t| {
            d.child(div().mt(z(8.)).text_size(z(14.)).line_height(z(18.)).text_color(c(tok(idx::TEXT_DEFAULT))).child(t))
        })
        .child(div().mt(z(8.)).py(z(8.)).flex().flex_col().gap(z(24.)).children(rows))
}

pub fn grid_columns(content: &[f32], avail: f32, min: f32, gap: f32) -> Vec<f32> {
    let n = (((avail + gap) / (min + gap)).floor() as usize).max(1);
    let mut cols = vec![min; n.min(content.len().max(1))];
    let mut limits = cols.clone();
    for (i, w) in content.iter().enumerate() {
        let c = &mut limits[i % n];
        *c = c.max(*w);
    }
    let mut free = avail - gap * (cols.len() as f32 - 1.) - cols.iter().sum::<f32>();
    while free > 0.5 {
        let growing: Vec<usize> = (0..cols.len()).filter(|&i| limits[i] - cols[i] > 0.01).collect();
        if growing.is_empty() {
            break;
        }
        let share = free / growing.len() as f32;
        for i in growing {
            let d = share.min(limits[i] - cols[i]);
            cols[i] += d;
            free -= d;
        }
    }
    cols
}

fn text_width(window: &gpui::Window, text: &str, size: Pixels, weight: FontWeight) -> f32 {
    let mut font = crate::richtext::ui_font();
    font.weight = weight;
    let run = gpui::TextRun { len: text.len(), font, color: gpui::black(), background_color: None, underline: None, strikethrough: None };
    f32::from(window.text_system().layout_line(text, size, &[run], None).width)
}

fn card_content_width(window: &gpui::Window, label: &str, keys: &[&str]) -> f32 {
    let label_w = text_width(window, label, z(14.), FontWeight::NORMAL);
    let combos: f32 = keys
        .iter()
        .map(|k| {
            let caps = key_caps(k);
            caps.iter().map(|c| text_width(window, c, z(12.), FontWeight::SEMIBOLD).max(2.) + 14.).sum::<f32>() + 3. * (caps.len() as f32 - 1.)
        })
        .sum::<f32>()
        + 8. * (keys.len() as f32 - 1.);
    label_w.max(combos).ceil() + 24.
}

fn grid_rows(window: &gpui::Window, items: Vec<(SharedString, &[&str])>, avail: f32) -> Vec<gpui::AnyElement> {
    let widths: Vec<f32> = items.iter().map(|(l, k)| card_content_width(window, l, k)).collect();
    let cols = grid_columns(&widths, avail, 180., 24.);
    let n = cols.len();
    let mut rows = Vec::new();
    let mut it = items.into_iter().enumerate().peekable();
    while it.peek().is_some() {
        let mut row = div().flex().gap(z(24.));
        for _ in 0..n {
            let Some((i, (label, keys))) = it.next() else { break };
            row = row.child(keybind_card(label, keys, px(cols[i % n])));
        }
        rows.push(row.into_any_element());
    }
    rows
}

pub fn help_card(window: &gpui::Window) -> gpui::AnyElement {
    let vp = window.viewport_size();
    let w = (vp.width * 0.8).clamp(px(800.), px(950.));
    let avail = f32::from(w) - 48.;
    let mut list = div().id("shortcuts-list").flex_1().min_h_0().overflow_y_scroll().pr(z(16.));
    let msg: Vec<(SharedString, &[&str])> = MESSAGE_SHORTCUTS.iter().map(|(k, l)| (crate::t!(l), std::slice::from_ref(k))).collect();
    list = list.child(section(crate::t!("shortcuts.group.messages"), Some(crate::t!("shortcuts.group.messages_desc")), grid_rows(window, msg, avail)));
    for g in [Group::Navigation, Group::Voice, Group::Chat, Group::Misc] {
        let items = SHORTCUTS.iter().filter(|s| s.group == g).map(|s| (crate::t!(s.label), s.keys)).collect();
        list = list.child(section(g.title(), None, grid_rows(window, items, avail)));
    }
    div()
        .rounded(z(12.))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.16),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(8.),
            spread_radius: px(0.),
            inset: false,
        }])
        .child(
            div()
                .w(w)
                .max_h(vp.height * 0.8)
                .relative()
                .rounded(z(4.))
                .overflow_hidden()
                .bg(c(tok(idx::BACKGROUND_BASE_LOWER)))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .pt(z(20.))
                        .px(z(40.))
                        .mb(z(5.))
                        .text_size(z(26.))
                        .line_height(z(34.))
                        .text_color(c(tok(idx::TEXT_STRONG)))
                        .child(crate::t!("shortcuts.title"))
                        .child(combo("ctrl-/").ml(z(20.))),
                )
                .child(
                    div()
                        .px(z(40.))
                        .pb(z(20.))
                        .text_size(z(16.))
                        .line_height(z(20.))
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(crate::t!("shortcuts.subtitle")),
                )
                .child(list)
                .child(crate::ui::easter::arrows()),
        )
        .into_any_element()
}

pub fn render_help(_app: &PhoenixApp, window: &mut gpui::Window, cx: &mut Context<PhoenixApp>) -> Option<gpui::AnyElement> {
    if !help_open() {
        return None;
    }
    crate::ui::easter::preload();
    if crate::ui::easter::demon_active() {
        return crate::ui::easter::render_demon(window, cx);
    }
    if crate::ui::easter::arrows_animating() {
        window.request_animation_frame();
    }
    let entity = cx.entity().downgrade();
    let close: ui::modal::Action = std::rc::Rc::new(move |_, cx| {
        with(|s| s.help_open = false);
        let _ = entity.update(cx, |_, cx| cx.notify());
    });
    Some(ui::modal::frame("shortcuts-modal", close, help_card(window)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_goes_to_the_channel_only_for_members() {
        assert!(support_is_joined([Id(1), SUPPORT_GUILD].into_iter()));
        assert!(!support_is_joined([Id(1), Id(2)].into_iter()));
        assert!(!support_is_joined(std::iter::empty()));
        assert_eq!(crate::ui::home::data::invite_code(SUPPORT_INVITE).as_deref(), Some("uF3wtckHkN"));
    }

    #[test]
    fn zoom_steps_follow_the_client() {
        assert_eq!(zoom_step(100, 1), 110);
        assert_eq!(zoom_step(100, -1), 90);
        assert_eq!(zoom_step(200, 1), 200);
        assert_eq!(zoom_step(50, -1), 50);
        assert_eq!(zoom_step(105, 1), 110);
        assert_eq!(zoom_step(105, -1), 100);
    }

    #[test]
    fn cycle_wraps_both_ways() {
        let l = [1, 2, 3];
        assert_eq!(cycle(&l, Some(3), 1), Some(1));
        assert_eq!(cycle(&l, Some(1), -1), Some(3));
        assert_eq!(cycle(&l, Some(2), 1), Some(3));
        assert_eq!(cycle(&l, None, 1), Some(1));
        assert_eq!(cycle(&l, None, -1), Some(3));
        assert_eq!(cycle::<i32>(&[], None, 1), None);
    }

    #[test]
    fn seek_skips_current_and_wraps() {
        let l = [1, 2, 3, 4];
        let odd = |x: i32| x % 2 == 1;
        assert_eq!(seek(&l, Some(1), 1, odd), Some(3));
        assert_eq!(seek(&l, Some(3), 1, odd), Some(1));
        assert_eq!(seek(&l, Some(1), -1, odd), Some(3));
        assert_eq!(seek(&l, Some(1), 1, |x| x == 1), None);
        assert_eq!(seek(&l, None, 1, |x| x > 1), Some(2));
    }

    #[test]
    fn rail_puts_home_first_and_folders_last() {
        let (a, b, c, d) = (Id(1), Id(2), Id(3), Id(4));
        let order = rail_order(&[a, b, c, d], &[vec![c, Id(99)], vec![a]]);
        assert_eq!(order, vec![None, Some(b), Some(d), Some(c), Some(a)]);
    }

    #[test]
    fn history_back_and_forward() {
        let mut h = History::default();
        h.visit(Id(1));
        h.visit(Id(2));
        h.visit(Id(2));
        h.visit(Id(3));
        assert_eq!(h.back(), Some(Id(2)));
        assert_eq!(h.back(), Some(Id(1)));
        assert_eq!(h.back(), None);
        assert_eq!(h.forward(), Some(Id(2)));
        h.visit(Id(5));
        assert_eq!(h.forward(), None);
        assert_eq!(h.back(), Some(Id(2)));
    }

    #[test]
    fn every_listed_key_is_bound() {
        let cx = gpui::TestAppContext::single();
        cx.update(|cx| {
            bind_keys(cx);
            crate::app::bind_keys(cx);
            for s in SHORTCUTS {
                for k in s.keys {
                    let ks = gpui::Keystroke::parse(k).unwrap();
                    let keymap = cx.key_bindings();
                    let keymap = keymap.borrow();
                    assert!(
                        keymap.bindings().any(|b| b.keystrokes().len() == 1 && b.keystrokes()[0].inner().modifiers == ks.modifiers && b.keystrokes()[0].inner().key == ks.key),
                        "{k} not bound"
                    );
                }
            }
        });
    }

    #[test]
    fn client_only_binds_are_wired() {
        let cx = gpui::TestAppContext::single();
        cx.update(|cx| {
            bind_keys(cx);
            for (k, name) in [
                ("ctrl-t", "QuickSwitcherAlt"),
                ("shift-pagedown", "JumpToPresent"),
                ("ctrl-shift-a", "ToggleAllCategories"),
                ("ctrl-alt-left", "JumpToVoiceGuild"),
                ("ctrl-shift-tab", "PrevGuild"),
                ("ctrl-tab", "NextGuild"),
                ("f1", "GetHelp"),
            ] {
                let ks = gpui::Keystroke::parse(k).unwrap();
                let keymap = cx.key_bindings();
                let keymap = keymap.borrow();
                let hit = keymap.bindings().find(|b| b.keystrokes().len() == 1 && b.keystrokes()[0].inner().modifiers == ks.modifiers && b.keystrokes()[0].inner().key == ks.key);
                let hit = hit.unwrap_or_else(|| panic!("{k} not bound"));
                assert!(hit.action().name().ends_with(name), "{k} -> {}", hit.action().name());
            }
        });
    }

    #[test]
    fn collapse_all_collapses_until_everything_is_folded() {
        let cats = [Id(1), Id(2)];
        let mut c = std::collections::HashSet::new();
        assert!(collapse_all_target(&cats, &c));
        c.insert(Id(1));
        assert!(collapse_all_target(&cats, &c));
        c.insert(Id(2));
        assert!(!collapse_all_target(&cats, &c));
        assert!(!collapse_all_target(&[], &c));
    }

    #[test]
    fn grid_columns_follow_auto_fill_max_content() {
        let cols = grid_columns(&[150., 245., 120., 180., 90., 100., 170., 160.], 902., 180., 24.);
        assert_eq!(cols, vec![180., 245., 180., 180.]);
        assert_eq!(grid_columns(&[300.], 902., 180., 24.), vec![300.]);
        let cols = grid_columns(&[220., 220., 400., 400.], 902., 180., 24.);
        assert!((cols.iter().sum::<f32>() + 72. - 902.).abs() < 1., "{cols:?}");
        assert!(cols.iter().all(|c| *c >= 180. && *c <= 400.), "{cols:?}");
    }

    #[test]
    fn caps_are_localized_upper() {
        let caps = key_caps("ctrl-shift-u");
        assert_eq!(caps.len(), 3);
        assert_eq!(caps[2].as_ref(), "U");
    }
}
