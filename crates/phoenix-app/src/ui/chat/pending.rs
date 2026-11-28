use std::{cell::RefCell, collections::HashMap};

use gpui::{AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div};

use crate::{
    app::PhoenixApp,
    backend::{AppEvent, BackendError, Upload},
    model::{FLAG_EPHEMERAL, Id, ImageRef, Message, SendState, User},
    theme::{generated::idx, z},
};

use super::{state::DetachChat, tk};

pub const SYSTEM_BOT: Id = Id(1);

#[derive(Clone)]
struct Payload {
    content: String,
    reply: Option<(Id, bool)>,
    files: Vec<Upload>,
    upload: Option<u64>,
}

thread_local! {
    static PAYLOADS: RefCell<HashMap<u64, Payload>> = RefCell::new(HashMap::new());
    static DISCARDED: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
}

pub fn make_nonce() -> u64 {
    phoenix_api::models::Snowflake::now().0
}

pub fn is_local(m: &Message) -> bool {
    m.send_state != SendState::Sent || m.flags & FLAG_EPHEMERAL != 0
}

pub fn local_copy(app: &PhoenixApp, channel: Id, nonce: u64, content: &str, reply: Option<(Id, bool)>) -> Message {
    Message {
        id: Id(nonce),
        channel,
        author: app.me.id,
        timestamp: (phoenix_api::models::Snowflake(nonce).timestamp_ms() / 1000) as i64,
        content: content.to_owned().into(),
        reply_to: reply.map(|r| r.0),
        reply_mentions_author: reply.is_some_and(|r| r.1),
        nonce: Some(nonce),
        send_state: SendState::Sending,
        ..Default::default()
    }
}

const CLYDE_AVATAR: &str = "https://discord.com/assets/9380e4b5bd8d267c.png";

pub fn ephemeral_footer(entity: &Entity<PhoenixApp>, msg: &Message) -> AnyElement {
    let (e, ch, id) = (entity.clone(), msg.channel, msg.id);
    div()
        .mt(z(4.))
        .h(z(16.))
        .flex()
        .items_center()
        .gap(z(4.))
        .text_size(z(12.))
        .line_height(z(16.))
        .text_color(tk(idx::TEXT_MUTED))
        .child(super::icons::icon(super::icons::ChatIcon::Eye, 16., tk(idx::TEXT_MUTED)))
        .child(crate::t!("chat.ephemeral_only_you"))
        .child(
            div()
                .id(("eph-dismiss", id.0))
                .text_color(tk(idx::TEXT_LINK))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_click(move |_, _, cx| e.update(cx, |a, cx| a.discard_local(ch, id, cx)))
                .child(crate::t!("chat.ephemeral_dismiss")),
        )
        .into_any_element()
}

impl PhoenixApp {
    pub(crate) fn send_optimistic(
        &mut self,
        channel: Id,
        content: String,
        reply: Option<(Id, bool)>,
        files: Vec<Upload>,
        upload: Option<u64>,
        cx: &mut Context<Self>,
    ) -> u64 {
        let nonce = make_nonce();
        super::announce::take_next(nonce);
        let local = local_copy(self, channel, nonce, &content, reply);
        PAYLOADS.with(|p| p.borrow_mut().insert(nonce, Payload { content, reply, files, upload }));
        self.append_message(local, cx);
        self.post_pending(channel, nonce, cx);
        nonce
    }

    fn post_pending(&mut self, channel: Id, nonce: u64, cx: &mut Context<Self>) {
        let Some(p) = PAYLOADS.with(|p| p.borrow().get(&nonce).cloned()) else { return };
        let upload = p.upload;
        let progress = upload.and_then(|k| self.upload_progress(k, cx));
        let fut = self.backend.send_message_ex(channel, p.content, p.reply, p.files, nonce, progress);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(k) = upload {
                    this.chat.sending.retain(|s| s.key != k);
                }
                if DISCARDED.with(|d| d.borrow_mut().remove(&nonce)) {
                    if let Ok(m) = res {
                        this.backend.delete_message(channel, m.id).detach_ok_chat();
                    }
                    return;
                }
                match res {
                    Ok(mut m) => {
                        m.nonce.get_or_insert(nonce);
                        super::announce::after_sent(this, channel, nonce, m.id, cx);
                        if !this.take_echo(&m, cx) {
                            this.append_message(m, cx);
                        }
                    }
                    Err(e) => this.mark_send_failed(channel, nonce, &e, cx),
                }
            });
        })
        .detach();
    }

    pub(crate) fn take_echo(&mut self, m: &Message, cx: &mut Context<Self>) -> bool {
        let Some(nonce) = m.nonce else { return false };
        if m.author != self.me.id {
            return false;
        }
        if DISCARDED.with(|d| d.borrow().contains(&nonce)) {
            return true;
        }
        let ch = m.channel;
        let Some(ix) = self
            .messages
            .get(&ch)
            .and_then(|e| e.msgs.iter().rposition(|x| x.nonce == Some(nonce) && x.send_state != SendState::Sent))
        else {
            return false;
        };
        PAYLOADS.with(|p| p.borrow_mut().remove(&nonce));
        let Some(mut entry) = self.messages.remove(&ch) else { return false };
        entry.rendered[ix] = self.render_msg(m, ix.checked_sub(1).map(|p| &entry.msgs[p]));
        entry.msgs[ix] = m.clone();
        self.messages.insert(ch, entry);
        super::welcome::record_message(self, m);
        if self.selected_channel == Some(ch) {
            super::rows::refresh(&self.message_list, ix + 1);
        }
        cx.notify();
        true
    }

    fn set_send_state(&mut self, ch: Id, id: Id, state: SendState, cx: &mut Context<Self>) -> Option<usize> {
        let mut entry = self.messages.remove(&ch)?;
        let ix = entry.index_of(id);
        if let Some(ix) = ix {
            entry.msgs[ix].send_state = state;
            entry.rendered[ix] = self.render_msg(&entry.msgs[ix], ix.checked_sub(1).map(|p| &entry.msgs[p]));
            if self.selected_channel == Some(ch) {
                super::rows::refresh(&self.message_list, ix + 1);
            }
        }
        self.messages.insert(ch, entry);
        cx.notify();
        ix
    }

    fn mark_send_failed(&mut self, ch: Id, nonce: u64, e: &BackendError, cx: &mut Context<Self>) {
        let Some(ix) = self.set_send_state(ch, Id(nonce), SendState::Failed, cx) else { return };
        if e.is_rejected() {
            self.insert_system_notice(ch, ix + 1, nonce, cx);
        }
    }

    fn insert_system_notice(&mut self, ch: Id, at: usize, nonce: u64, cx: &mut Context<Self>) {
        self.users.entry(SYSTEM_BOT).or_insert_with(|| User {
            id: SYSTEM_BOT,
            name: "clyde".into(),
            display_name: "Clyde".into(),
            avatar: ImageRef::Url(CLYDE_AVATAR.into()),
            bot: true,
            verified_bot: true,
            flags: 0,
            tag: None,
            tag_guild: None,
            nameplate: None,
            name_style: None,
            decoration: None,
        });
        let notice = Message {
            id: Id(nonce + 1),
            channel: ch,
            author: SYSTEM_BOT,
            timestamp: chrono_now(),
            content: format!("{}\n\n{}", crate::t!("chat.send_failed"), crate::t!("chat.send_failed_reasons")).into(),
            flags: FLAG_EPHEMERAL,
            ..Default::default()
        };
        let Some(mut entry) = self.messages.remove(&ch) else { return };
        let at = at.min(entry.msgs.len());
        let r = self.render_msg(&notice, at.checked_sub(1).map(|p| &entry.msgs[p]));
        entry.msgs.insert(at, notice);
        entry.rendered.insert(at, r);
        self.messages.insert(ch, entry);
        if self.selected_channel == Some(ch) {
            super::rows::insert(&self.message_list, at + 1, 1);
        }
        cx.notify();
    }

    pub fn retry_send(&mut self, ch: Id, id: Id, cx: &mut Context<Self>) {
        let known = PAYLOADS.with(|p| p.borrow().contains_key(&id.0));
        if !known || self.set_send_state(ch, id, SendState::Sending, cx).is_none() {
            return;
        }
        self.post_pending(ch, id.0, cx);
    }

    pub fn discard_local(&mut self, ch: Id, id: Id, cx: &mut Context<Self>) {
        let local = self.messages.get(&ch).and_then(|e| e.index_of(id).map(|i| &e.msgs[i])).is_some_and(is_local);
        if !local {
            return;
        }
        let sending = self.messages.get(&ch).and_then(|e| e.index_of(id).map(|i| &e.msgs[i])).filter(|m| m.send_state == SendState::Sending).and_then(|m| m.nonce);
        if let Some(nonce) = sending
            && PAYLOADS.with(|p| p.borrow().contains_key(&nonce))
        {
            DISCARDED.with(|d| d.borrow_mut().insert(nonce));
        }
        PAYLOADS.with(|p| p.borrow_mut().remove(&id.0));
        self.handle_event(AppEvent::MessageDelete { channel: ch, id }, cx);
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_is_a_time_snowflake() {
        let n = make_nonce();
        assert_eq!(n & ((1 << 22) - 1), 0, "worker / sequence bits are zero, like the client");
        let ms = phoenix_api::models::Snowflake(n).timestamp_ms();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        assert!(now.abs_diff(ms) < 5_000);
    }

    #[test]
    fn local_states() {
        let mut m = Message::default();
        assert!(!is_local(&m));
        m.send_state = SendState::Sending;
        assert!(is_local(&m));
        m.send_state = SendState::Failed;
        assert!(is_local(&m));
        m.send_state = SendState::Sent;
        m.flags = FLAG_EPHEMERAL;
        assert!(is_local(&m));
    }

    #[test]
    fn rejected_errors_are_marked() {
        assert!(BackendError::rejected("HTTP 400").is_rejected());
        assert!(!BackendError("transport".into()).is_rejected());
    }
}
