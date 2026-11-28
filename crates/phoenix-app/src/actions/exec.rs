use phoenix_api::{
    RestClient,
    models as api,
    rest::guild_admin,
};

use super::{ActionReply, ActionRequest, EventPlace};
use crate::{
    backend::{AppEvent, BackendError, EventSink},
    discord_backend::convert::{id, message, message_users, sf, user},
    model::Reaction,
};

fn e(err: phoenix_api::Error) -> BackendError {
    BackendError(err.to_string())
}

pub fn emoji(r: &Reaction) -> api::ReactionEmoji {
    api::ReactionEmoji { id: r.emoji_id.map(sf), name: Some(crate::ui::chat::state::reaction_name(&r.emoji)), animated: false }
}

pub fn event_place(p: &EventPlace) -> guild_admin::EventPlace {
    match p {
        EventPlace::Voice(c) => guild_admin::EventPlace::Voice(sf(*c)),
        EventPlace::Stage(c) => guild_admin::EventPlace::Stage(sf(*c)),
        EventPlace::External(s) => guild_admin::EventPlace::External(s.clone()),
    }
}

pub fn folder_protos(folders: &[crate::ui::home::data::GuildFolder]) -> Vec<phoenix_api::rest::settings::FolderProto> {
    folders
        .iter()
        .map(|f| phoenix_api::rest::settings::FolderProto {
            guilds: f.guilds.iter().map(|g| g.0).collect(),
            id: f.id,
            name: f.name.as_ref().map(|n| n.to_string()).filter(|n| !n.is_empty()),
            color: f.color.map(u64::from),
        })
        .collect()
}

pub async fn request(rest: RestClient, req: ActionRequest, emit: EventSink) -> Result<ActionReply, BackendError> {
    request_with(super::guard::writes_enabled(), rest, req, emit).await
}

pub(crate) async fn request_with(allowed: bool, rest: RestClient, req: ActionRequest, emit: EventSink) -> Result<ActionReply, BackendError> {
    if let Some(err) = super::guard::refuse_action(allowed, &req) {
        return Err(e(err));
    }
    use ActionRequest as R;
    Ok(match req {
        R::Admin(c) => match rest.admin(&c).await {
            Ok(value) => ActionReply::Json(value),
            Err(err) if matches!(c, phoenix_api::rest::guild_manage::AdminCall::Server(phoenix_api::rest::guild_server::ServerCall::DeleteGuild { .. })) => {
                ActionReply::AdminMfaRequired(phoenix_api::rest::guild_manage::admin_mfa_methods(&err).ok_or_else(|| e(err))?)
            }
            Err(err) => return Err(e(err)),
        },
        R::AdminMfa(c, method, code) => match rest.admin_with_mfa(&c, method, code.expose()).await {
            Ok(value) => ActionReply::Json(value),
            Err(phoenix_api::Error::Api { body, .. }) if matches!(body.code, 60008 | 60011 | 50035) => ActionReply::AdminMfaRejected,
            Err(err) => ActionReply::AdminMfaRequired(phoenix_api::rest::guild_manage::admin_mfa_methods(&err).ok_or_else(|| e(err))?),
        },
        R::FavoriteEmoji { key, on } => {
            rest.set_favorite_emoji(key, on).await.map_err(e)?;
            ActionReply::Done
        }
        R::FavoriteSticker { id, on } => {
            rest.set_favorite_sticker(id, on).await.map_err(e)?;
            ActionReply::Done
        }
        R::SaveFolders(folders) => {
            rest.save_guild_folders(&folder_protos(&folders)).await.map_err(e)?;
            ActionReply::Done
        }
        R::Forward { from, msg, guild, to, comment } => {
            for ch in to {
                rest.forward_message(sf(ch), sf(from), sf(msg), guild.map(sf)).await.map_err(e)?;
                if !comment.trim().is_empty() {
                    rest.send_message(sf(ch), phoenix_api::rest::CreateMessage::text(comment.trim())).await.map_err(e)?;
                }
            }
            ActionReply::Done
        }
        R::CreateThread { channel, msg, name } => {
            let c = rest.create_thread_from_message(sf(channel), sf(msg), &name).await.map_err(e)?;
            ActionReply::Channel(id(c.id))
        }
        R::CreateStandaloneThread { channel, name, private, announcement } => {
            let kind = phoenix_api::rest::thread_create::thread_type(private, announcement);
            let c = rest.create_standalone_thread(sf(channel), &name, kind, 4320).await.map_err(e)?;
            ActionReply::Channel(id(c.id))
        }
        R::Remind { channel, msg, due_unix } => {
            rest.set_reminder(sf(channel), sf(msg), due_unix).await.map_err(e)?;
            ActionReply::Done
        }
        R::Reminders => {
            let list = rest.get_reminders().await.map_err(e)?;
            let msgs: Vec<&api::Message> = list.iter().filter_map(|r| r.message.as_ref()).collect();
            let mut users = Vec::new();
            for m in &msgs {
                message_users(m, &mut users);
            }
            if !users.is_empty() {
                emit(AppEvent::Users(users));
            }
            ActionReply::Messages(msgs.into_iter().map(message).collect())
        }
        R::Reactors { channel, msg, emoji: r, burst } => {
            let us = rest.get_reactions(sf(channel), sf(msg), &emoji(&r), 100, burst).await.map_err(e)?;
            ActionReply::Users(us.iter().map(user).collect())
        }
        R::EmojiSource(id) => ActionReply::Json(rest.get_emoji_source(id).await.map_err(e)?),
        R::RemoveReactions { channel, msg, emoji: r } => {
            rest.remove_reactions(sf(channel), sf(msg), r.as_ref().map(emoji).as_ref()).await.map_err(e)?;
            ActionReply::Done
        }
        R::ReportMenu(t) => ActionReply::ReportMenu(Box::new(rest.get_report_menu(t).await.map_err(e)?)),
        R::SubmitReport { menu, target, steps } => {
            rest.submit_report(&menu, target, &steps).await.map_err(e)?;
            ActionReply::Done
        }
        R::SetRole { guild, user: u, role, add } => {
            if add {
                rest.add_member_role(sf(guild), sf(u), sf(role)).await
            } else {
                rest.remove_member_role(sf(guild), sf(u), sf(role)).await
            }
            .map_err(e)?;
            ActionReply::Done
        }
        R::SetMemberRoles { guild, user: u, roles } => {
            let roles: Vec<_> = roles.into_iter().map(sf).collect();
            rest.set_member_roles(sf(guild), sf(u), &roles).await.map_err(e)?;
            ActionReply::Done
        }
        R::UnassignRoleConnection { guild, role } => {
            rest.unassign_role_connection(sf(guild), sf(role)).await.map_err(e)?;
            ActionReply::Done
        }
        R::SetNick { guild, user: u, nick } => {
            rest.set_member_nick(sf(guild), u.map(sf), &nick).await.map_err(e)?;
            ActionReply::Done
        }
        R::FriendNick { user: u, nick } => {
            rest.set_friend_nickname(sf(u), &nick).await.map_err(e)?;
            ActionReply::Done
        }
        R::Ignore { user: u, ignored } => {
            rest.set_ignored(sf(u), ignored).await.map_err(e)?;
            ActionReply::Done
        }
        R::VoiceMod { guild, user: u, op } => {
            rest.voice_moderate(sf(guild), sf(u), op).await.map_err(e)?;
            ActionReply::Done
        }
        R::Stage(c) => {
            rest.stage(&c).await.map_err(e)?;
            ActionReply::Done
        }
        R::CloneChannel { guild, channel, name } => {
            let src = rest.get_channel(sf(channel)).await.map_err(e)?;
            let c = rest.clone_channel(sf(guild), &src, &name).await.map_err(e)?;
            ActionReply::Channel(id(c.id))
        }
        R::MoveChannels { guild, entries } => {
            let v = entries.into_iter().map(|(c, pos, parent)| guild_admin::move_entry(sf(c), pos, parent.map(|p| p.map(sf)))).collect();
            rest.move_channels(sf(guild), v).await.map_err(e)?;
            ActionReply::Done
        }
        R::CreateEvent { guild, name, description, start_unix, end_unix, place, recurrence, image } => {
            let mut body = guild_admin::event_body(&name, &description, start_unix, end_unix, &event_place(&place));
            body["recurrence_rule"] = recurrence.unwrap_or(serde_json::Value::Null);
            if let Some(img) = image {
                body["image"] = serde_json::Value::String(img);
            }
            let id = rest.create_scheduled_event(sf(guild), body).await.map_err(e)?;
            ActionReply::Json(serde_json::json!({ "id": id.to_string() }))
        }
    })
}
