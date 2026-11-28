use std::{path::PathBuf, sync::Arc, time::Instant};

use gpui::{
    AppContext, ClipboardEntry, ClipboardItem, Context, Entity, Image, ListScrollEvent, PathPromptOptions, Pixels,
    Point, SharedString, Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    backend::Upload,
    model::{Id, Message, Reaction, ThreadRef},
};

pub const MAX_FILES: usize = phoenix_api::rest::uploads::MAX_ATTACHMENTS;
pub const QUICK_REACTIONS: [&str; 4] = ["👍", "❤️", "😂", "🔥"];

pub struct EditState {
    pub id: Id,
    pub channel: Id,
    pub input: Entity<gpui_component::input::TextareaState>,
    _sub: Subscription,
}

#[derive(Clone, Copy)]
pub struct MenuState {
    pub channel: Id,
    pub msg: Id,
    pub pos: Point<Pixels>,
}

pub struct PinsState {
    pub channel: Id,
    pub loading: bool,
    pub msgs: Vec<Message>,
}

pub struct ThreadPanel {
    pub thread: ThreadRef,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

#[derive(Clone)]
pub struct PendingFile {
    pub name: SharedString,
    pub bytes: Arc<Vec<u8>>,
    pub image: Option<Arc<Image>>,
    pub spoiler: bool,
    pub blurred: Option<Arc<Image>>,
    pub description: String,
    pub pre: Option<u64>,
}

pub struct Sending {
    pub key: u64,
    pub channel: Id,
    pub label: SharedString,
    pub icon_class: &'static str,
    pub size: u64,
    pub started: Instant,
    pub sent: Arc<std::sync::atomic::AtomicU64>,
    pub cancel: Arc<std::sync::atomic::AtomicBool>,
    pub nonce: Option<u64>,
}

impl Sending {
    pub fn fraction(&self) -> f32 {
        if self.size == 0 {
            return 1.;
        }
        (self.sent.load(std::sync::atomic::Ordering::Relaxed) as f64 / self.size as f64).clamp(0., 1.) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmojiTarget {
    Reaction { channel: Id, msg: Id },
    Composer,
    Edit,
}

const FLASH_HOLD: std::time::Duration = std::time::Duration::from_millis(2600);
const JUMP_MAX_PAGES: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewMessages {
    pub count: u32,
    pub since: i64,
    pub estimated: bool,
}

pub struct ChatState {
    pub style_gen: u32,
    pub edit: Option<EditState>,
    pub reply_mention: bool,
    pub menu: Option<MenuState>,
    pub confirm_delete: Option<(Id, Id)>,
    pub confirm_pin: Option<(Id, Message)>,
    pub search_menu_msg: Option<Message>,
    pub pins: Option<PinsState>,
    pub thread: Option<ThreadPanel>,
    pub(crate) create_thread: Option<crate::ui::thread_create::CreateThread>,
    pub uploads: Vec<PendingFile>,
    pub sending: Vec<Sending>,
    pub attach_menu: bool,
    pub new_messages: Option<NewMessages>,
    pub at_bottom: bool,
    pub unread_marker: Option<(Id, Id)>,
    pub emoji_target: Option<EmojiTarget>,
    pub error: Option<SharedString>,
    pub shift_held: bool,
    pub flash: Option<Id>,
    pub pending_jump: Option<(Id, Id)>,
    pub hidden_open: std::collections::HashSet<Id>,
    pub quick_reactions: Option<Vec<QuickReaction>>,
    next_key: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuickReaction {
    pub emoji: SharedString,
    pub id: Option<Id>,
    pub guild: Option<Id>,
}

impl ChatState {
    pub fn quick_reactions_for(&self, guild: Option<Id>, premium: bool, n: usize) -> Vec<QuickReaction> {
        match &self.quick_reactions {
            Some(q) => q.iter().filter(|r| premium || r.guild.is_none() || r.guild == guild).take(n).cloned().collect(),
            None => QUICK_REACTIONS.iter().take(n).map(|e| QuickReaction { emoji: (*e).into(), id: None, guild: None }).collect(),
        }
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            style_gen: 0,
            edit: None,
            reply_mention: true,
            menu: None,
            confirm_delete: None,
            confirm_pin: None,
            search_menu_msg: None,
            pins: None,
            thread: None,
            create_thread: None,
            uploads: Vec::new(),
            sending: Vec::new(),
            attach_menu: false,
            new_messages: None,
            at_bottom: true,
            unread_marker: None,
            emoji_target: None,
            error: None,
            shift_held: false,
            flash: None,
            pending_jump: None,
            hidden_open: Default::default(),
            quick_reactions: None,
            next_key: 0,
        }
    }
}

pub fn reaction_name(emoji: &str) -> String {
    let t = emoji.trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = t.split(':').collect();
    match parts.as_slice() {
        [_, name, _id] if emoji.starts_with('<') => (*name).to_string(),
        [name, _id] if emoji.starts_with('<') => (*name).to_string(),
        _ if emoji.starts_with(':') && emoji.ends_with(':') && emoji.len() > 2 => emoji[1..emoji.len() - 1].to_string(),
        _ => emoji.to_string(),
    }
}

pub fn apply_reaction(m: &mut Message, emoji: &str, emoji_id: Option<Id>, add: bool) -> bool {
    let same = |r: &Reaction| match (r.emoji_id, emoji_id) {
        (Some(a), Some(b)) => a == b,
        _ => r.emoji.as_ref() == emoji,
    };
    match m.reactions.iter().position(same) {
        Some(ix) => {
            let r = &mut m.reactions[ix];
            if r.me == add {
                return false;
            }
            r.me = add;
            if add {
                r.count += 1;
            } else {
                r.count = r.count.saturating_sub(1);
                if r.count == 0 && r.burst_count == 0 {
                    m.reactions.remove(ix);
                }
            }
            true
        }
        None if add => {
            m.reactions.push(Reaction { emoji: emoji.to_string().into(), count: 1, me: true, emoji_id, ..Default::default() });
            true
        }
        None => false,
    }
}

pub fn is_image_name(name: &str) -> Option<gpui::ImageFormat> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => gpui::ImageFormat::Png,
        "jpg" | "jpeg" => gpui::ImageFormat::Jpeg,
        "gif" => gpui::ImageFormat::Gif,
        "webp" => gpui::ImageFormat::Webp,
        "bmp" => gpui::ImageFormat::Bmp,
        _ => return None,
    })
}

pub fn pending_file(name: String, bytes: Vec<u8>) -> PendingFile {
    let image = is_image_name(&name).map(|f| Arc::new(Image::from_bytes(f, bytes.clone())));
    PendingFile { name: name.into(), bytes: Arc::new(bytes), image, spoiler: false, blurred: None, description: String::new(), pre: None }
}

impl PhoenixApp {
    pub fn chat_sync_style(&mut self) {
        let g = crate::theme::style_generation();
        if self.chat.style_gen == g {
            return;
        }
        self.chat.style_gen = g;
        let ids: Vec<Id> = self.messages.keys().copied().collect();
        for ch in ids {
            let Some(mut entry) = self.messages.remove(&ch) else { continue };
            entry.rendered = (0..entry.msgs.len())
                .map(|i| self.render_msg(&entry.msgs[i], i.checked_sub(1).map(|p| &entry.msgs[p])))
                .collect();
            self.messages.insert(ch, entry);
        }
        self.message_list.remeasure();
    }

    pub fn find_msg(&self, channel: Id, id: Id) -> Option<&Message> {
        self.messages.get(&channel)?.msgs.iter().find(|m| m.id == id)
    }

    pub fn chat_on_select(&mut self, channel: Id) {
        super::page_scroll::stop();
        self.messages.retain(|_, e| !e.detached);
        self.chat.edit = None;
        self.chat.menu = None;
        self.chat.confirm_delete = None;
        self.chat.confirm_pin = None;
        for f in self.chat.uploads.drain(..) {
            if let Some(k) = f.pre {
                self.backend.drop_preupload(k);
            }
        }
        self.chat.attach_menu = false;
        self.chat.new_messages = None;
        self.chat.at_bottom = true;
        self.chat.emoji_target = None;
        self.chat.error = None;
        if self.chat.pins.as_ref().is_some_and(|p| p.channel != channel) {
            self.chat.pins = None;
        }
        self.parity.threads_open = false;
        if self.chat.unread_marker.is_some_and(|(c, _)| c != channel) {
            self.chat.unread_marker = None;
        }
        self.chat.thread = None;
    }

    pub fn chat_on_scroll(&mut self, ev: &ListScrollEvent, cx: &mut Context<Self>) {
        let bottom = ev.visible_range.end >= ev.count || ev.is_following_tail;
        if bottom != self.chat.at_bottom {
            self.chat.at_bottom = bottom;
            if bottom {
                self.chat.new_messages = None;
            }
            cx.notify();
        }
    }

    pub fn chat_note_incoming(&mut self, own: bool, ts: i64) {
        if own {
            self.message_list.scroll_to_end();
            self.chat.new_messages = None;
        } else if !self.chat.at_bottom || self.chat.new_messages.is_some() {
            let e = self.chat.new_messages.get_or_insert(NewMessages { count: 0, since: ts, estimated: false });
            e.count += 1;
        }
    }

    pub fn jump_to_present(&mut self, cx: &mut Context<Self>) {
        if self.leave_detached(cx) {
            return;
        }
        self.message_list.scroll_to_end();
        self.chat.at_bottom = true;
        self.chat.new_messages = None;
        cx.notify();
    }

    pub fn jump_to_first_unread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((channel, id)) = self.chat.unread_marker.filter(|(c, _)| Some(*c) == self.selected_channel) else {
            self.jump_to_present(cx);
            return;
        };
        match self.messages.get(&channel).and_then(|e| e.msgs.iter().position(|m| m.id == id)) {
            Some(ix) => {
                let view = self.message_list.viewport_bounds();
                match self.message_list.bounds_for_item(ix + 1).filter(|_| view.size.height > gpui::px(0.)) {
                    Some(node) => {
                        let rel = f32::from(node.top() - view.top());
                        let by = super::page_scroll::middle_offset(rel, f32::from(node.size.height), f32::from(view.size.height), f32::from(crate::theme::z(1.)));
                        let reduced = crate::ui::settings::effects::reduced_motion();
                        super::page_scroll::animate_by(&self.message_list, f64::from(by), cx.entity_id(), window, reduced);
                    }
                    None => self.message_list.scroll_to_reveal_item(ix + 1),
                }
                cx.notify();
            }
            None => self.jump_to_message(channel, id, cx),
        }
    }

    pub fn mark_channel_read(&mut self, cx: &mut Context<Self>) {
        self.chat.new_messages = None;
        self.chat.unread_marker = None;
        if let Some(ch) = self.selected_channel {
            self.backend.select_channel(ch).detach_ok_chat();
        }
        cx.notify();
    }

    pub fn can_edit(&self, m: &Message) -> bool {
        m.author == self.me.id
            && !super::pending::is_local(m)
            && matches!(m.type_id, 0 | 19 | 20 | 23 | 26 | 41 | 45 | 47 | 68)
            && m.flags & phoenix_voice::audio::voice_message::FLAG_IS_VOICE_MESSAGE == 0
            && m.forward.is_none()
            && crate::commands::store::get(m.id).is_none_or(|p| p.poll.is_none())
    }

    pub fn start_edit(&mut self, id: Id, window: &mut Window, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        let Some(m) = self.find_msg(channel, id) else { return };
        if !self.can_edit(m) {
            return;
        }
        let content = m.content.clone();
        let max_rows = ((f32::from(window.viewport_size().height) * 0.5 / 22.).floor() as usize).max(2);
        let input = cx.new(|cx| gpui_component::input::TextareaState::new(window, cx).auto_grow(1, max_rows).submit_on_enter(true));
        input.update(cx, |s, cx| {
            super::rich_input::set_value_at_end(s, content.to_string(), window, cx);
            s.focus(window, cx);
        });
        let sub = cx.subscribe_in(&input, window, |this, input, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { shift: false, .. } => this.save_edit(window, cx),
            InputEvent::Change => {
                super::rich_input::sync_area(this, input, cx);
                crate::ui::autocomplete::refresh(this, window, cx);
            }
            _ => {}
        });
        crate::ui::autocomplete::dismiss(cx);
        self.chat.menu = None;
        self.chat.edit = Some(EditState { id, channel, input, _sub: sub });
        if let Some(ix) = self.messages.get(&channel).and_then(|e| e.msgs.iter().position(|m| m.id == id)) {
            super::rows::refresh(&self.message_list, ix + 1);
        }
        cx.notify();
    }

    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(e) = self.chat.edit.take() {
            self.refresh_row(e.channel, e.id);
        }
        crate::ui::autocomplete::dismiss(cx);
        if crate::ui::pickers::open_tab(cx).is_some_and(|(_, t)| t == crate::ui::pickers::EmojiTarget::Edit) {
            crate::ui::pickers::close(self, window, cx);
        }
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub fn save_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.chat.edit.as_ref() else { return };
        let (id, channel) = (e.id, e.channel);
        let text = super::rich_input::peek_plain(e.input.read(cx).value().trim());
        let bare = self.find_msg(channel, id).is_none_or(|m| m.attachments.is_empty());
        if text.is_empty() && bare {
            self.chat.edit = None;
            self.chat.confirm_delete = Some((channel, id));
            cx.notify();
            return;
        }
        let unchanged = self.find_msg(channel, id).is_some_and(|m| m.content.as_ref() == text);
        if !unchanged {
            let original = self.find_msg(channel, id).cloned();
            if let Some(mut m) = original.clone() {
                m.content = text.clone().into();
                m.edited = true;
                m.edited_at = chrono::Utc::now().timestamp();
                self.replace_message(m, cx);
            }
            let fut = self.backend.edit_message(channel, id, text);
            cx.spawn(async move |this, cx| {
                let res = fut.await;
                let _ = this.update(cx, |this, cx| match res {
                    Ok(m) => this.replace_message(m, cx),
                    Err(e) => {
                        if let Some(m) = original {
                            this.replace_message(m, cx);
                        }
                        this.chat.error = Some(e.0.into());
                    }
                });
            })
            .detach();
        }
        self.cancel_edit(window, cx);
    }

    pub(crate) fn refresh_row(&mut self, channel: Id, id: Id) {
        if self.selected_channel == Some(channel)
            && let Some(ix) = self.messages.get(&channel).and_then(|e| e.msgs.iter().position(|m| m.id == id))
        {
            super::rows::refresh(&self.message_list, ix + 1);
        }
    }

    pub fn toggle_reaction(&mut self, channel: Id, msg: Id, r: &Reaction, cx: &mut Context<Self>) {
        self.set_reaction_local(channel, msg, r.emoji.clone(), r.emoji_id, !r.me, cx);
    }

    pub fn toggle_burst_reaction(&mut self, channel: Id, msg: Id, r: &Reaction, cx: &mut Context<Self>) {
        if !self.backend.has_premium() {
            crate::ui::modals::burst_upsell::open(cx);
            return;
        }
        let e = crate::actions::exec::emoji(r);
        self.blocked_write(crate::actions::guard::GuardedWrite::burst_reaction(channel.0, msg.0, &e, !r.me_burst), cx);
    }

    pub fn react_with(&mut self, channel: Id, msg: Id, emoji: SharedString, emoji_id: Option<Id>, cx: &mut Context<Self>) {
        self.set_reaction_local(channel, msg, emoji, emoji_id, true, cx);
    }

    fn set_reaction_local(
        &mut self,
        channel: Id,
        msg: Id,
        emoji: SharedString,
        emoji_id: Option<Id>,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(mut m) = self.find_msg(channel, msg).cloned() else { return };
        if super::pending::is_local(&m) {
            return;
        }
        if !apply_reaction(&mut m, &emoji, emoji_id, add) {
            return;
        }
        self.replace_message(m, cx);
        let spoken = reaction_name(&emoji);
        let r = Reaction { emoji, count: 0, me: add, emoji_id, ..Default::default() };
        let fut = self.backend.set_reaction(channel, msg, r, add);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Err(e) => this.chat.error = Some(e.0.into()),
                    Ok(_) if add => crate::ui::a11y::announce(crate::t!("a11y.reaction_added", name = spoken), false),
                    Ok(_) => crate::ui::a11y::announce(crate::t!("a11y.reaction_removed", name = spoken), false),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_emoji_picker(&mut self, target: EmojiTarget, cx: &mut Context<Self>) {
        self.chat.menu = None;
        self.chat.emoji_target = Some(target);
        cx.notify();
    }

    pub fn emoji_picked(&mut self, emoji: SharedString, emoji_id: Option<Id>, window: &mut Window, cx: &mut Context<Self>) {
        match self.chat.emoji_target.take() {
            Some(EmojiTarget::Reaction { channel, msg }) => self.react_with(channel, msg, emoji, emoji_id, cx),
            Some(EmojiTarget::Edit) if self.chat.edit.is_some() => {
                if let Some(e) = self.chat.edit.as_ref().map(|e| e.input.clone()) {
                    e.update(cx, |s, cx| {
                        s.insert(emoji.to_string(), window, cx);
                        s.focus(window, cx);
                    });
                }
            }
            Some(EmojiTarget::Composer | EmojiTarget::Edit) | None => {
                self.composer.update(cx, |s, cx| {
                    s.insert(emoji.to_string(), window, cx);
                    s.focus(window, cx);
                });
            }
        }
        cx.notify();
    }

    pub fn request_delete(&mut self, channel: Id, id: Id, skip_confirm: bool, cx: &mut Context<Self>) {
        self.chat.menu = None;
        if self.find_msg(channel, id).is_some_and(super::pending::is_local) {
            self.discard_local(channel, id, cx);
            return;
        }
        if skip_confirm {
            self.backend.delete_message(channel, id).detach_ok_chat();
            crate::ui::a11y::announce(crate::t!("a11y.message_deleted"), false);
        } else {
            self.chat.confirm_delete = Some((channel, id));
        }
        cx.notify();
    }

    pub fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if let Some((channel, id)) = self.chat.confirm_delete.take() {
            self.backend.delete_message(channel, id).detach_ok_chat();
            crate::ui::a11y::announce(crate::t!("a11y.message_deleted"), false);
        }
        cx.notify();
    }

    pub fn open_message_menu(&mut self, channel: Id, msg: Id, pos: Point<Pixels>, cx: &mut Context<Self>) {
        self.chat.menu = Some(MenuState { channel, msg, pos });
        super::panels::arm_menu_media();
        cx.notify();
    }

    pub fn copy_message_text(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        if let Some(m) = self.find_msg(channel, id) {
            cx.write_to_clipboard(ClipboardItem::new_string(m.content.to_string()));
        }
    }

    pub fn copy_message_link(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        let guild = self.channels.get(&channel).and_then(|c| c.guild).map(|g| g.0.to_string());
        let url = format!(
            "https://discord.com/channels/{}/{}/{}",
            guild.unwrap_or_else(|| "@me".into()),
            channel.0,
            id.0
        );
        cx.write_to_clipboard(ClipboardItem::new_string(url));
    }

    pub fn request_pin(&mut self, channel: Id, id: Id, skip_confirm: bool, cx: &mut Context<Self>) {
        self.chat.menu = None;
        if skip_confirm {
            self.toggle_pin(channel, id, cx);
        } else if let Some(m) = self.find_msg(channel, id).cloned() {
            self.chat.confirm_pin = Some((channel, m));
        }
        cx.notify();
    }

    pub fn request_pin_message(&mut self, channel: Id, m: Message, skip_confirm: bool, cx: &mut Context<Self>) {
        self.chat.menu = None;
        if !skip_confirm {
            self.chat.confirm_pin = Some((channel, m));
        } else if m.pinned {
            self.unpin_from_popout(channel, m.id, cx);
        } else {
            self.set_pinned_by_id(channel, m.id, true, cx);
        }
        cx.notify();
    }

    pub fn confirm_pin(&mut self, cx: &mut Context<Self>) {
        if let Some((channel, m)) = self.chat.confirm_pin.take() {
            if m.pinned {
                self.unpin_from_popout(channel, m.id, cx);
            } else {
                self.set_pinned_by_id(channel, m.id, true, cx);
            }
        }
        cx.notify();
    }

    pub fn toggle_pin(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        let Some(mut m) = self.find_msg(channel, id).cloned() else { return };
        m.pinned = !m.pinned;
        let pinned = m.pinned;
        if let Some(p) = self.chat.pins.as_mut().filter(|p| p.channel == channel) {
            p.msgs.retain(|x| x.id != id);
            if pinned {
                p.msgs.insert(0, m.clone());
            }
        }
        self.replace_message(m, cx);
        self.backend.set_pinned(channel, id, pinned).detach_ok_chat();
    }

    pub fn set_pinned_by_id(&mut self, channel: Id, id: Id, pinned: bool, cx: &mut Context<Self>) {
        if self.find_msg(channel, id).is_some_and(|m| m.pinned != pinned) {
            self.toggle_pin(channel, id, cx);
            return;
        }
        self.backend.set_pinned(channel, id, pinned).detach_ok_chat();
        cx.notify();
    }

    pub fn mark_unread_by_id(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        if self.find_msg(channel, id).is_some() {
            self.mark_unread_from(channel, id, cx);
            return;
        }
        if let Some(c) = self.channels.get_mut(&channel) {
            c.unread = true;
        }
        self.backend.mark_unread(channel, Id(id.0.saturating_sub(1))).detach_ok_chat();
        self.rebuild_sidebar();
        cx.notify();
    }

    pub fn unpin_from_popout(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        if let Some(p) = self.chat.pins.as_mut() {
            p.msgs.retain(|x| x.id != id);
        }
        if let Some(mut m) = self.find_msg(channel, id).cloned() {
            m.pinned = false;
            self.replace_message(m, cx);
        }
        self.backend.set_pinned(channel, id, false).detach_ok_chat();
        cx.notify();
    }

    pub fn mark_unread_from(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        let Some(entry) = self.messages.get(&channel) else { return };
        let Some(ix) = entry.msgs.iter().position(|m| m.id == id) else { return };
        let last_read = ix.checked_sub(1).map(|p| entry.msgs[p].id).unwrap_or(Id(id.0.saturating_sub(1)));
        let old = self.chat.unread_marker.replace((channel, id));
        if let Some((c, m)) = old {
            self.refresh_row(c, m);
        }
        self.refresh_row(channel, id);
        if let Some(c) = self.channels.get_mut(&channel) {
            c.unread = true;
        }
        self.backend.mark_unread(channel, last_read).detach_ok_chat();
        self.rebuild_sidebar();
        cx.notify();
    }

    pub fn toggle_pins(&mut self, cx: &mut Context<Self>) {
        if self.chat.pins.take().is_some() {
            cx.notify();
            return;
        }
        let Some(channel) = self.selected_channel else { return };
        self.chat.pins = Some(PinsState { channel, loading: true, msgs: Vec::new() });
        if self.backend.has_unread_pins(channel) {
            drop(self.backend.ack_pins(channel));
        }
        let fut = self.backend.load_pins(channel);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = this.chat.pins.as_mut().filter(|p| p.channel == channel) {
                    p.loading = false;
                    match res {
                        Ok(msgs) => p.msgs = msgs,
                        Err(e) => this.chat.error = Some(e.0.into()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn jump_to_message(&mut self, channel: Id, id: Id, cx: &mut Context<Self>) {
        self.chat.pins = None;
        let vocal = self.channels.get(&channel).is_some_and(|c| c.guild.is_some() && matches!(c.kind, crate::model::ChannelKind::Voice | crate::model::ChannelKind::Stage));
        if vocal {
            crate::ui::call_view::set_chat_open(channel, true);
        }
        if self.selected_channel != Some(channel) {
            if vocal {
                self.open_channel_chat(channel, cx);
            } else {
                self.select_channel(channel, cx);
            }
        }
        self.jump_reveal(channel, id, JUMP_MAX_PAGES, cx);
        cx.notify();
    }

    pub(crate) fn resume_pending_jump(&mut self, channel: Id, cx: &mut Context<Self>) {
        if self.selected_channel != Some(channel) {
            return;
        }
        if let Some((c, id)) = self.chat.pending_jump.take()
            && c == channel
        {
            self.jump_reveal(c, id, JUMP_MAX_PAGES, cx);
        }
    }

    fn jump_reveal(&mut self, channel: Id, id: Id, pages: u32, cx: &mut Context<Self>) {
        self.chat.pending_jump = None;
        let Some(e) = self.messages.get_mut(&channel) else {
            self.chat.pending_jump = Some((channel, id));
            return;
        };
        if let Some(ix) = e.msgs.iter().position(|m| m.id == id) {
            self.message_list.scroll_to_reveal_item(ix + 1);
            self.flash_message(id, cx);
            return;
        }
        if e.loading || !e.loaded_once {
            if pages == JUMP_MAX_PAGES {
                self.chat.pending_jump = Some((channel, id));
            }
            return;
        }
        if e.msgs.first().is_some_and(|m| m.id.0 < id.0) {
            return;
        }
        if pages == JUMP_MAX_PAGES {
            self.jump_around(channel, id, cx);
            return;
        }
        if pages == 0 || e.reached_start {
            return;
        }
        let before = e.msgs.first().map(|m| m.id);
        e.loading = true;
        let fut = self.backend.load_messages(channel, before, 50);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.insert_history(channel, res.unwrap_or_default(), true, cx);
                if this.selected_channel == Some(channel) {
                    this.jump_reveal(channel, id, pages - 1, cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn flash_message(&mut self, id: Id, cx: &mut Context<Self>) {
        self.chat.flash = Some(id);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH_HOLD).await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.flash == Some(id) {
                    this.chat.flash = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub fn open_thread(&mut self, thread: ThreadRef, window: &mut Window, cx: &mut Context<Self>) {
        self.chat.create_thread = None;
        if crate::ui::search::results_shown(cx) {
            crate::ui::search::close(self, window, cx);
        }
        let id = thread.id;
        let input = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = ev {
                this.send_in_thread(window, cx);
            }
        });
        input.update(cx, |s, cx| s.focus(window, cx));
        self.chat.thread = Some(ThreadPanel { thread, input, _sub: sub });
        let loaded = self.messages.get(&id).is_some_and(|e| e.loaded_once);
        if !loaded {
            self.messages.entry(id).or_default().loading = true;
            let fut = self.backend.load_messages(id, None, 50);
            cx.spawn(async move |this, cx| {
                let res = fut.await;
                let _ = this.update(cx, |this, cx| this.insert_history(id, res.unwrap_or_default(), false, cx));
            })
            .detach();
        }
        cx.notify();
    }

    pub fn close_thread(&mut self, cx: &mut Context<Self>) {
        self.chat.thread = None;
        cx.notify();
    }

    pub(crate) fn close_thread_and_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_thread(cx);
        self.focus_chat_composer(window, cx);
    }

    pub(crate) fn focus_chat_composer(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(form) = self.active_thread_form() {
            form.starter.update(cx, |s, cx| s.focus(window, cx));
        } else if let Some(thread) = &self.chat.thread {
            thread.input.update(cx, |s, cx| s.focus(window, cx));
        } else {
            self.composer.update(cx, |s, cx| s.focus(window, cx));
        }
    }

    fn send_in_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.chat.thread.as_ref() else { return };
        let id = t.thread.id;
        let text = t.input.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        t.input.update(cx, |s, cx| s.set_value("", window, cx));
        let fut = self.backend.send_message(id, text, None);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| match res {
                Ok(m) => this.append_message(m, cx),
                Err(e) => {
                    this.chat.error = Some(e.0.into());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn prompt_attach(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(crate::t!("chat.upload")),
        });
        cx.spawn(async move |this, cx| match rx.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update(cx, |this, cx| this.add_paths(paths, cx));
            }
            Ok(Ok(None)) | Err(_) => {}
            Ok(Err(e)) => {
                tracing::warn!("file chooser failed: {e:#}");
                let _ = this.update(cx, |this, cx| {
                    this.chat.error = Some(crate::t!("chat.picker_failed", e = format!("{e:#}")));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn upload_progress(&mut self, key: u64, cx: &mut Context<Self>) -> Option<crate::backend::UploadProgress> {
        let s = self.chat.sending.iter().find(|s| s.key == key)?;
        let (sent, cancel) = (s.sent.clone(), s.cancel.clone());
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
            let alive = this
                .update(cx, |this, cx| {
                    let alive = this.chat.sending.iter().any(|s| s.key == key);
                    if alive {
                        cx.notify();
                    }
                    alive
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
        })
        .detach();
        Some(Arc::new(move |n: u64, _total: u64| {
            sent.store(n, std::sync::atomic::Ordering::Relaxed);
            !cancel.load(std::sync::atomic::Ordering::Relaxed)
        }))
    }

    pub fn cancel_upload(&mut self, key: u64, cx: &mut Context<Self>) {
        let Some(ix) = self.chat.sending.iter().position(|s| s.key == key) else { return };
        let s = self.chat.sending.remove(ix);
        s.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(n) = s.nonce {
            self.discard_local(s.channel, Id(n), cx);
        }
        cx.notify();
    }

    pub fn upload_limit(&self) -> u64 {
        self.selected_channel.map_or(20 << 20, |ch| self.backend.max_upload_size(ch))
    }

    pub fn upload_text_as_file(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        if self.push_upload("message.txt".into(), text.into_bytes()) {
            self.composer.update(cx, |s, cx| s.set_value("", window, cx));
        }
        cx.notify();
    }

    pub fn push_upload(&mut self, name: String, bytes: Vec<u8>) -> bool {
        let limit = self.upload_limit();
        if self.chat.uploads.len() >= MAX_FILES {
            self.chat.error = Some(crate::t!("chat.max_files", n = MAX_FILES));
            return false;
        }
        if bytes.is_empty() {
            self.chat.error = Some(crate::t!("chat.empty_file", name = name));
            return false;
        }
        if bytes.len() as u64 > limit {
            self.chat.error = Some(crate::t!("chat.too_large", name = name, max = limit >> 20));
            return false;
        }
        let total: u64 = self.chat.uploads.iter().map(|u| u.bytes.len() as u64).sum::<u64>() + bytes.len() as u64;
        if total > phoenix_api::rest::uploads::MAX_TOTAL_SIZE {
            self.chat.error =
                Some(crate::t!("chat.total_too_large", max = phoenix_api::rest::uploads::MAX_TOTAL_SIZE >> 20));
            return false;
        }
        let mut file = pending_file(name, bytes);
        if let Some(ch) = self.selected_channel {
            let key = self.chat.next_key;
            self.chat.next_key += 1;
            self.backend.preupload(ch, key, file.name.to_string(), file.bytes.clone());
            file.pre = Some(key);
        }
        self.chat.uploads.push(file);
        true
    }

    pub fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let max = self.upload_limit();
        let task = cx.background_spawn(async move {
            paths
                .into_iter()
                .filter(|p| p.is_file())
                .map(|p| {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
                    let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    if len > max {
                        return Err(crate::t!("chat.too_large", name = name, max = max >> 20).to_string());
                    }
                    std::fs::read(&p).map(|b| (name.clone(), b)).map_err(|e| format!("{name}: {e}"))
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let files = task.await;
            let _ = this.update(cx, |this, cx| {
                for f in files {
                    match f {
                        Ok((name, bytes)) => {
                            this.push_upload(name, bytes);
                        }
                        Err(e) => this.chat.error = Some(e.into()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn paste_clipboard(&mut self, item: &ClipboardItem, cx: &mut Context<Self>) -> bool {
        if self.selected_channel.is_none() {
            return false;
        }
        let mut took = false;
        for e in item.entries() {
            match e {
                ClipboardEntry::Image(img) => {
                    let ext = match img.format {
                        gpui::ImageFormat::Jpeg => "jpg",
                        gpui::ImageFormat::Gif => "gif",
                        gpui::ImageFormat::Webp => "webp",
                        gpui::ImageFormat::Bmp => "bmp",
                        gpui::ImageFormat::Svg => "svg",
                        _ => "png",
                    };
                    self.push_upload(format!("image.{ext}"), img.bytes.clone());
                    took = true;
                }
                ClipboardEntry::ExternalPaths(p) => {
                    self.add_paths(p.paths().to_vec(), cx);
                    took = true;
                }
                ClipboardEntry::String(t) => {
                    if let Some(paths) = file_uris(t.text()) {
                        self.add_paths(paths, cx);
                        took = true;
                    }
                }
            }
        }
        if took {
            cx.notify();
        }
        took
    }

    pub fn remove_upload(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.chat.uploads.len() {
            let f = self.chat.uploads.remove(ix);
            if let Some(k) = f.pre {
                self.backend.drop_preupload(k);
            }
        }
        cx.notify();
    }

    pub fn toggle_upload_spoiler(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(u) = self.chat.uploads.get_mut(ix) {
            u.spoiler = !u.spoiler;
            if u.spoiler && u.blurred.is_none() && u.image.is_some() {
                let bytes = u.bytes.clone();
                let key = u.bytes.clone();
                let task = cx.background_spawn(async move { super::spoiler::blurred_image(&bytes) });
                cx.spawn(async move |this, cx| {
                    let img = task.await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(u) = this.chat.uploads.get_mut(ix).filter(|u| Arc::ptr_eq(&u.bytes, &key)) {
                            u.blurred = img;
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn chat_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(ch) = self.selected_channel else { return true };
        if super::voice_message::on_enter(self, cx) {
            return true;
        }
        if super::slash::submit(self, window, cx) {
            return true;
        }
        let text = self.composer.read(cx).value().trim().to_string();
        if text.is_empty() && self.chat.uploads.is_empty() {
            return true;
        }
        if super::composer::too_long(self.backend.premium_tier2(), &text) {
            self.chat.error = Some(crate::t!("chat.too_long"));
            cx.notify();
            return true;
        }
        if super::composer::send_held(self, ch) {
            super::composer::emphasize_cooldown();
            cx.notify();
            return true;
        }
        if let Some(at) = super::scheduled::draft(ch).filter(|_| self.chat.uploads.is_empty()) {
            let text = super::rich_input::take_plain(&text);
            self.composer.update(cx, |s, cx| s.set_value("", window, cx));
            let mut msg = phoenix_api::rest::CreateMessage::text(text);
            if let Some(r) = self.replying_to.take() {
                msg = msg.reply_to(phoenix_api::models::Snowflake(ch.0), phoenix_api::models::Snowflake(r.0), self.chat.reply_mention);
            }
            self.chat.reply_mention = true;
            self.send_scheduled(ch, at, msg, cx);
            return true;
        }
        super::composer::note_sent(ch);
        let text = super::rich_input::take_plain(&text);
        self.composer.update(cx, |s, cx| s.set_value("", window, cx));
        let reply = self.replying_to.take().map(|r| (r, self.chat.reply_mention));
        self.chat.reply_mention = true;
        let hero_channel = super::announce::channel(self);
        let files: Vec<Upload> = std::mem::take(&mut self.chat.uploads)
            .into_iter()
            .map(|f| Upload {
                hero: super::announce::is_hero(hero_channel, &f),
                filename: f.name.to_string(),
                bytes: Arc::unwrap_or_clone(f.bytes),
                spoiler: f.spoiler,
                description: Some(f.description).filter(|d| !d.trim().is_empty()),
                pre: f.pre,
            })
            .collect();
        let key = self.chat.next_key;
        self.chat.next_key += 1;
        if !files.is_empty() {
            let size = files.iter().map(|f| f.bytes.len() as u64).sum();
            let (label, icon_class): (SharedString, &'static str) = if files.len() == 1 {
                (files[0].filename.clone().into(), super::attach::file_class(&files[0].filename))
            } else {
                (crate::t!("chat.n_files", n = files.len()), "unknown")
            };
            self.chat.sending.push(Sending { key, channel: ch, label, icon_class, size, started: Instant::now(), sent: Arc::new(0.into()), cancel: Arc::default(), nonce: None });
        }
        let upload = (!files.is_empty()).then_some(key);
        let nonce = self.send_optimistic(ch, text, reply, files, upload, cx);
        if let Some(s) = self.chat.sending.iter_mut().find(|s| s.key == key) {
            s.nonce = Some(nonce);
        }
        self.message_list.scroll_to_end();
        cx.notify();
        true
    }
}

pub fn file_uris(text: &str) -> Option<Vec<PathBuf>> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let path = line.strip_prefix("file://")?;
        let path = path.strip_prefix("localhost").unwrap_or(path);
        if !path.starts_with('/') {
            return None;
        }
        out.push(PathBuf::from(percent_decode(path)?));
    }
    (!out.is_empty()).then_some(out)
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

pub(crate) trait DetachChat {
    fn detach_ok_chat(self);
}
impl<T: Send + 'static> DetachChat for crate::backend::BoxFuture<T> {
    fn detach_ok_chat(self) {
        std::thread::spawn(move || {
            let _ = futures::executor::block_on(self);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn upload_end_to_end_on_the_mock(cx: &mut gpui::TestAppContext) {
        use crate::{backend::Backend, mock::MockBackend};
        let backend = Arc::new(MockBackend::synchronous());
        let ready = futures::executor::block_on(backend.login(crate::backend::Credentials::Token("x".into()))).unwrap();
        cx.update(gpui_component::init);
        let window = cx.add_window(|w, cx| PhoenixApp::new(backend.clone(), w, cx));
        let (ch, png_len) = window
            .update(cx, |app, _, cx| {
                app.apply_ready(ready, cx);
                let g = app.guilds[0].id;
                app.select_guild(Some(g), cx);
                let ch = app.guild_channels[&g]
                    .iter()
                    .copied()
                    .find(|c| app.channels[c].kind == crate::model::ChannelKind::Text)
                    .unwrap();
                app.select_channel(ch, cx);
                assert_eq!(app.upload_limit(), 20 << 20);
                assert!(!app.push_upload("empty.txt".into(), vec![]));
                assert!(!app.push_upload("big.bin".into(), vec![0; (20 << 20) + 1]));
                assert!(app.chat.error.as_ref().is_some_and(|e| e.contains("big.bin")));
                let png = {
                    let mut out = std::io::Cursor::new(Vec::new());
                    image::RgbaImage::new(2, 2).write_to(&mut out, image::ImageFormat::Png).unwrap();
                    out.into_inner()
                };
                let item = ClipboardItem { entries: vec![ClipboardEntry::Image(Image::from_bytes(gpui::ImageFormat::Png, png.clone()))] };
                assert!(app.paste_clipboard(&item, cx));
                assert!(!app.paste_clipboard(&ClipboardItem::new_string("just text".into()), cx));
                assert!(app.push_upload("notes.txt".into(), b"hello".to_vec()));
                assert_eq!(app.chat.uploads.len(), 2);
                assert_eq!(app.chat.uploads[0].name.as_ref(), "image.png");
                app.chat.uploads[0].spoiler = true;
                app.chat.uploads[1].description = "my notes".into();
                (ch, png.len())
            })
            .unwrap();
        window
            .update(cx, |app, w, cx| {
                assert!(app.chat_submit(w, cx));
                assert!(app.chat.uploads.is_empty(), "the tray empties on send");
                assert_eq!(app.chat.sending.len(), 1);
                assert_eq!(app.chat.sending[0].size, png_len as u64 + 5);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |app, _, _| {
                assert!(app.chat.sending.is_empty(), "the progress card goes away");
                let last = app.messages[&ch].msgs.last().unwrap();
                let names: Vec<&str> = last.attachments.iter().map(|a| a.filename.as_ref()).collect();
                assert_eq!(names, ["image.png", "notes.txt"]);
                assert!(last.attachments[0].is_image);
            })
            .unwrap();
        let sent = backend.sent_uploads();
        assert_eq!(sent.len(), 1);
        let f = &sent[0];
        assert_eq!((f[0].filename.as_str(), f[0].spoiler), ("image.png", true), "no SPOILER_ prefix: the client sends is_spoiler");
        assert_eq!((f[1].filename.as_str(), f[1].description.as_deref()), ("notes.txt", Some("my notes")));
        assert_eq!(f[1].bytes, b"hello");
    }

    #[test]
    fn pasted_file_uris() {
        assert_eq!(
            file_uris("file:///home/me/a%20b.png\r\nfile:///tmp/c.txt\n"),
            Some(vec![PathBuf::from("/home/me/a b.png"), PathBuf::from("/tmp/c.txt")])
        );
        assert_eq!(file_uris("hello"), None);
        assert_eq!(file_uris("file:///a.png\nhttps://x.y/z"), None);
        assert_eq!(file_uris("  "), None);
    }

    #[test]
    fn reaction_names() {
        assert_eq!(reaction_name("👍"), "👍");
        assert_eq!(reaction_name("<:blob:123>"), "blob");
        assert_eq!(reaction_name("<a:party:42>"), "party");
        assert_eq!(reaction_name(":old:"), "old");
    }

    #[test]
    fn reaction_toggle_counts() {
        let mut m = Message::default();
        assert!(apply_reaction(&mut m, "👍", None, true));
        assert_eq!((m.reactions[0].count, m.reactions[0].me), (1, true));
        assert!(!apply_reaction(&mut m, "👍", None, true));
        m.reactions[0].count = 3;
        assert!(apply_reaction(&mut m, "👍", None, false));
        assert_eq!((m.reactions[0].count, m.reactions[0].me), (2, false));
        m.reactions[0].count = 1;
        m.reactions[0].me = true;
        assert!(apply_reaction(&mut m, "👍", None, false));
        assert!(m.reactions.is_empty());
    }

    #[test]
    fn custom_reaction_matches_by_id() {
        let mut m = Message::default();
        m.reactions.push(Reaction { emoji: "<:a:7>".into(), count: 2, me: false, emoji_id: Some(Id(7)), ..Default::default() });
        assert!(apply_reaction(&mut m, "<:renamed:7>", Some(Id(7)), true));
        assert_eq!(m.reactions.len(), 1);
        assert_eq!(m.reactions[0].count, 3);
    }
}
