pub mod admin;
pub mod channel;
pub mod exec;
pub mod guard;
pub mod message;
pub mod mock;
pub mod pins;
pub mod thread;
pub mod user;

use std::cell::RefCell;

use gpui::{AnyElement, Context, Window};
use phoenix_api::rest::{members::VoiceMod, reporting::{ReportMenu, ReportStep, ReportTarget}};

use crate::{
    app::PhoenixApp,
    backend::BackendError,
    model::{Id, Message, Reaction, User},
};

#[derive(Clone, Debug, PartialEq)]
pub enum EventPlace {
    Voice(Id),
    Stage(Id),
    External(String),
}

#[derive(Clone, Debug)]
pub enum ActionRequest {
    Forward { from: Id, msg: Id, guild: Option<Id>, to: Vec<Id>, comment: String },
    CreateThread { channel: Id, msg: Id, name: String },
    CreateStandaloneThread { channel: Id, name: String, private: bool, announcement: bool },
    Remind { channel: Id, msg: Id, due_unix: u64 },
    Reminders,
    Reactors { channel: Id, msg: Id, emoji: Reaction, burst: bool },
    EmojiSource(u64),
    RemoveReactions { channel: Id, msg: Id, emoji: Option<Reaction> },
    ReportMenu(ReportTarget),
    SubmitReport { menu: Box<ReportMenu>, target: ReportTarget, steps: Vec<ReportStep> },
    SetRole { guild: Id, user: Id, role: Id, add: bool },
    SetMemberRoles { guild: Id, user: Id, roles: Vec<Id> },
    UnassignRoleConnection { guild: Id, role: Id },
    SetNick { guild: Id, user: Option<Id>, nick: String },
    FriendNick { user: Id, nick: String },
    Ignore { user: Id, ignored: bool },
    VoiceMod { guild: Id, user: Id, op: VoiceMod },
    CloneChannel { guild: Id, channel: Id, name: String },
    MoveChannels { guild: Id, entries: Vec<(Id, i32, Option<Option<Id>>)> },
    CreateEvent { guild: Id, name: String, description: String, start_unix: u64, end_unix: Option<u64>, place: EventPlace, recurrence: Option<serde_json::Value>, image: Option<String> },
    Admin(phoenix_api::rest::guild_manage::AdminCall),
    AdminMfa(phoenix_api::rest::guild_manage::AdminCall, phoenix_api::rest::guild_manage::AdminMfaMethod, crate::discord_backend::store::Secret),
    SaveFolders(Vec<crate::ui::home::data::GuildFolder>),
    FavoriteEmoji { key: String, on: bool },
    FavoriteSticker { id: u64, on: bool },
    Stage(phoenix_api::rest::stage::StageCall),
}

#[derive(Clone, Debug)]
pub enum ActionReply {
    Done,
    Channel(Id),
    Users(Vec<User>),
    Messages(Vec<Message>),
    ReportMenu(Box<ReportMenu>),
    Json(serde_json::Value),
    AdminMfaRequired(Vec<phoenix_api::rest::guild_manage::AdminMfaMethod>),
    AdminMfaRejected,
}

pub use crate::ui::modals::Modal;

thread_local! {
    pub(crate) static MODAL: RefCell<Option<Modal>> = const { RefCell::new(None) };
}

pub fn open_modal(m: Modal, window: Option<&mut Window>) {
    MODAL.with(|s| *s.borrow_mut() = Some(m));
    if let Some(w) = window {
        w.refresh();
    }
}

pub fn close_modal() {
    MODAL.with(|s| *s.borrow_mut() = None);
}

pub fn with_modal<R>(f: impl FnOnce(&mut Option<Modal>) -> R) -> R {
    MODAL.with(|s| f(&mut s.borrow_mut()))
}

pub fn overlays(app: &mut PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Vec<AnyElement> {
    crate::ui::modals::prepare(window, cx);
    crate::ui::modals::render(app, cx).into_iter().collect()
}

impl PhoenixApp {
    pub fn action(
        &mut self,
        req: ActionRequest,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<ActionReply, BackendError>, &mut Context<Self>) + 'static,
    ) {
        let fut = self.backend.action(req);
        cx.spawn(async move |this, cx| {
            let res = fut.await;
            let _ = this.update(cx, |this, cx| {
                done(this, res, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn action_toast(&mut self, req: ActionRequest, ok: Option<gpui::SharedString>, cx: &mut Context<Self>) {
        self.action(req, cx, move |a, r, cx| match r {
            Ok(_) => {
                if let Some(ok) = ok {
                    a.parity_toast(ok, cx)
                }
            }
            Err(e) => a.parity_toast(e.0, cx),
        });
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub const ADMINISTRATOR: u64 = 1 << 3;
pub const MANAGE_CHANNELS: u64 = 1 << 4;
pub const MANAGE_GUILD: u64 = 1 << 5;
pub const MANAGE_MESSAGES: u64 = 1 << 13;
pub const MUTE_MEMBERS: u64 = 1 << 22;
pub const DEAFEN_MEMBERS: u64 = 1 << 23;
pub const MOVE_MEMBERS: u64 = 1 << 24;
pub const MANAGE_NICKNAMES: u64 = 1 << 27;
pub const MANAGE_ROLES: u64 = 1 << 28;
pub const CREATE_PUBLIC_THREADS: u64 = 1 << 35;
pub const MODERATE_MEMBERS: u64 = 1 << 40;

pub fn has(app: &PhoenixApp, guild: Option<Id>, bit: u64) -> bool {
    guild.is_some_and(|g| crate::ui::inbox::has_perm(app.backend.guild_permissions(g), bit))
}
