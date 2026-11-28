use phoenix_api::rest::guild_manage::AdminCall;
use phoenix_api::rest::guild_server::ServerCall;
use phoenix_api::rest::stage::StageCall;
use reqwest::Method;

pub use phoenix_api::rest::guard::{GuardedWrite, agent_session, blocked, permitted, read_only, set_writes_enabled, writes_enabled};

use super::ActionRequest;
use crate::commands::{ModAction, ParityRequest};

fn moderation_call(c: &AdminCall) -> bool {
    matches!(
        c,
        AdminCall::Kick { .. }
            | AdminCall::Ban { .. }
            | AdminCall::Unban { .. }
            | AdminCall::AddRoleMember { .. }
            | AdminCall::RemoveRoleMember { .. }
            | AdminCall::IncidentActions { .. }
            | AdminCall::Server(ServerCall::DeleteGuild { .. })
    )
}

fn stage_moderation(c: &StageCall) -> bool {
    matches!(
        c,
        StageCall::SetUserSuppress { .. } | StageCall::InviteToSpeak { .. } | StageCall::MoveUserToAudience { .. } | StageCall::End { .. }
    )
}

fn moderation_action(req: &ActionRequest) -> Option<(Method, String)> {
    match req {
        ActionRequest::Admin(c) | ActionRequest::AdminMfa(c, _, _) if moderation_call(c) => {
            let r = phoenix_api::rest::guild_manage::build(c);
            Some((r.method, r.path))
        }
        ActionRequest::SetRole { guild, user, role, add } => {
            Some((if *add { Method::PUT } else { Method::DELETE }, format!("/guilds/{}/members/{}/roles/{}", guild.0, user.0, role.0)))
        }
        ActionRequest::SetMemberRoles { guild, user, .. } => Some((Method::PATCH, format!("/guilds/{}/members/{}", guild.0, user.0))),
        ActionRequest::Stage(c) if stage_moderation(c) => {
            let r = phoenix_api::rest::stage::build(c);
            Some((r.method, r.path))
        }
        _ => None,
    }
}

fn moderation_parity(req: &ParityRequest) -> Option<(Method, String)> {
    let ParityRequest::Moderate { guild, user, action } = req else { return None };
    let (g, u) = (guild.0, user.0);
    Some(match action {
        ModAction::Timeout(_) => (Method::PATCH, format!("/guilds/{g}/members/{u}")),
        ModAction::Kick => (Method::DELETE, format!("/guilds/{g}/members/{u}")),
        ModAction::Ban(_) => (Method::PUT, format!("/guilds/{g}/bans/{u}")),
        ModAction::Unban => (Method::DELETE, format!("/guilds/{g}/bans/{u}")),
    })
}

pub fn refuse_action(allowed: bool, req: &ActionRequest) -> Option<phoenix_api::Error> {
    if allowed {
        return None;
    }
    moderation_action(req).map(|(m, p)| blocked(&m, p))
}

pub fn refuse_parity(allowed: bool, req: &ParityRequest) -> Option<phoenix_api::Error> {
    if allowed {
        return None;
    }
    moderation_parity(req).map(|(m, p)| blocked(&m, p))
}

fn action_method(req: &ActionRequest) -> Option<Method> {
    match req {
        ActionRequest::Admin(c) | ActionRequest::AdminMfa(c, _, _) => Some(phoenix_api::rest::guild_manage::build(c).method),
        ActionRequest::Reminders | ActionRequest::ReportMenu(_) | ActionRequest::Reactors { .. } | ActionRequest::EmojiSource(_) => Some(Method::GET),
        _ => None,
    }
}

pub fn read_only_action(req: &ActionRequest) -> bool {
    action_method(req).is_some_and(|m| read_only(&m))
}

pub fn read_only_write(w: &GuardedWrite) -> bool {
    read_only(&w.method)
}

pub fn read_only_parity(req: &ParityRequest) -> bool {
    matches!(
        req,
        ParityRequest::ForumPosts { .. }
            | ParityRequest::ContentInventory
            | ParityRequest::ScheduledEvents { .. }
            | ParityRequest::Onboarding { .. }
            | ParityRequest::ServerGuide { .. }
            | ParityRequest::FetchThread { .. }
            | ParityRequest::ArchivedThreads { .. }
            | ParityRequest::Mentions { .. }
            | ParityRequest::SavedMessages
            | ParityRequest::ScheduledMessages
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_writes_stay_off_by_default_and_examples_never_enable_them() {
        assert!(!crate::ui::settings::LocalPrefs::default().protected_writes());
        assert!(!writes_enabled());
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "rs") {
                let src = std::fs::read_to_string(&path).unwrap();
                assert!(!src.contains("set_writes_enabled"), "{} enables protected writes", path.display());
                seen += 1;
            }
        }
        assert!(seen > 0);
    }
    use crate::model::Id;
    use phoenix_api::{RestClient, models::Snowflake};

    const G: Snowflake = Snowflake(1);
    const U: Snowflake = Snowflake(2);

    #[test]
    fn reactor_lists_are_reads() {
        let emoji = crate::model::Reaction { emoji: "👍".into(), ..Default::default() };
        for burst in [false, true] {
            assert!(read_only_action(&ActionRequest::Reactors { channel: Id(1), msg: Id(2), emoji: emoji.clone(), burst }));
        }
    }

    #[test]
    fn server_deletion_is_protected_with_or_without_the_two_factor_code() {
        let call = || AdminCall::Server(ServerCall::DeleteGuild { guild: G });
        for req in [ActionRequest::Admin(call()), ActionRequest::AdminMfa(call(), phoenix_api::rest::guild_manage::AdminMfaMethod::Authenticator, crate::discord_backend::store::Secret::new("123456"))] {
            assert!(!read_only_action(&req));
            assert!(refuse_action(false, &req).is_some());
            assert!(refuse_action(true, &req).is_none());
        }
    }

    #[test]
    fn mod_view_reads_pass() {
        for c in [AdminCall::Member { guild: G, user: U }, AdminCall::AuditLogTarget { guild: G, target: U, limit: 50 }, AdminCall::Roles { guild: G }] {
            assert!(read_only_action(&ActionRequest::Admin(c)));
        }
    }

    #[test]
    fn mod_view_writes_are_refused() {
        for c in [
            AdminCall::Kick { guild: G, user: U },
            AdminCall::Ban { guild: G, user: U, delete_message_seconds: 0 },
            AdminCall::MemberSupplemental { guild: G, users: vec![U] },
            AdminCall::AddRoleMember { guild: G, role: Snowflake(3), user: U },
            AdminCall::FollowChannel { channel: Snowflake(4), webhook_channel: Snowflake(5) },
        ] {
            assert!(!read_only_action(&ActionRequest::Admin(c)));
        }
        assert!(!read_only_action(&ActionRequest::SetMemberRoles { guild: Id(1), user: Id(2), roles: vec![] }));
        assert!(!read_only_action(&ActionRequest::SetRole { guild: Id(1), user: Id(2), role: Id(3), add: true }));
        for action in [ModAction::Kick, ModAction::Ban(0), ModAction::Timeout(60), ModAction::Timeout(0), ModAction::Unban] {
            assert!(!read_only_parity(&ParityRequest::Moderate { guild: Id(1), user: Id(2), action }));
        }
        assert!(read_only_parity(&ParityRequest::SavedMessages));
    }

    #[test]
    fn guarded_writes_share_the_same_policy() {
        for w in [
            GuardedWrite::change_password("a", "b"),
            GuardedWrite::wishlist_add_sku("1"),
            GuardedWrite::crosspost(3, 4),
            GuardedWrite::put_onboarding(5, serde_json::json!({})),
            GuardedWrite::put_home_settings(5, serde_json::json!({})),
            GuardedWrite::change_call_region(5, "rotterdam"),
            GuardedWrite::automod_alert_action(5, 6, 7, 3),
            GuardedWrite::automod_clear_mention_raid(5),
        ] {
            assert!(!read_only_write(&w));
            assert!(w.refusal().is_write_blocked());
        }
        let kick = phoenix_api::rest::guild_manage::build(&AdminCall::Kick { guild: G, user: U });
        assert!(!read_only(&kick.method));
        assert!(blocked(&kick.method, "/guilds/1/members/2").is_write_blocked());
    }

    fn sink() -> crate::backend::EventSink {
        std::sync::Arc::new(|_| {})
    }

    fn rest_at(addr: std::net::SocketAddr) -> RestClient {
        RestClient::with_base(Some("test-token".into()), phoenix_api::ClientProperties::linux_desktop(), format!("http://{addr}"))
    }

    fn moderate(action: ModAction) -> ParityRequest {
        ParityRequest::Moderate { guild: Id(1), user: Id(2), action }
    }

    async fn untouched(listener: &tokio::net::TcpListener) -> bool {
        tokio::time::timeout(std::time::Duration::from_millis(200), listener.accept()).await.is_err()
    }

    async fn one_request(listener: tokio::net::TcpListener) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = sock.read(&mut chunk).await.unwrap();
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).into_owned();
            if let Some(end) = text.find("\r\n\r\n") {
                let len = text[..end]
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                    .unwrap_or(0);
                if buf.len() >= end + 4 + len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        sock.write_all(b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n").await.unwrap();
        String::from_utf8_lossy(&buf).into_owned()
    }

    async fn refused_offline(allowed: bool) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rest = rest_at(listener.local_addr().unwrap());
        for action in [ModAction::Kick, ModAction::Ban(0), ModAction::Timeout(60), ModAction::Timeout(0), ModAction::Unban] {
            let err = crate::commands::exec::request_with(allowed, rest.clone(), moderate(action), sink()).await.unwrap_err();
            assert!(err.0.starts_with("write blocked: "), "{}", err.0);
        }
        let ban = crate::commands::exec::request_with(allowed, rest.clone(), moderate(ModAction::Ban(0)), sink()).await.unwrap_err();
        assert_eq!(ban.0, "write blocked: PUT /guilds/1/bans/2");
        for c in [
            AdminCall::Kick { guild: G, user: U },
            AdminCall::Ban { guild: G, user: U, delete_message_seconds: 0 },
            AdminCall::Unban { guild: G, user: U },
            AdminCall::AddRoleMember { guild: G, role: Snowflake(3), user: U },
            AdminCall::RemoveRoleMember { guild: G, role: Snowflake(3), user: U },
            AdminCall::IncidentActions { guild: G, invites_until: None, dms_until: None, lockdown_hours: Some(1) },
        ] {
            let err = crate::actions::exec::request_with(allowed, rest.clone(), ActionRequest::Admin(c), sink()).await.unwrap_err();
            assert!(err.0.starts_with("write blocked: "), "{}", err.0);
        }
        for req in [
            ActionRequest::SetRole { guild: Id(1), user: Id(2), role: Id(3), add: true },
            ActionRequest::SetRole { guild: Id(1), user: Id(2), role: Id(3), add: false },
            ActionRequest::SetMemberRoles { guild: Id(1), user: Id(2), roles: vec![] },
        ] {
            let err = crate::actions::exec::request_with(allowed, rest.clone(), req, sink()).await.unwrap_err();
            assert!(err.0.starts_with("write blocked: "), "{}", err.0);
        }
        assert!(untouched(&listener).await, "a refused moderation action opened a connection");
    }

    #[tokio::test]
    async fn moderation_is_refused_by_default() {
        assert!(!writes_enabled());
        refused_offline(writes_enabled()).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rest = rest_at(listener.local_addr().unwrap());
        let err = crate::commands::exec::request(rest.clone(), moderate(ModAction::Kick), sink()).await.unwrap_err();
        assert_eq!(err.0, "write blocked: DELETE /guilds/1/members/2");
        let err = crate::actions::exec::request(rest, ActionRequest::Admin(AdminCall::Kick { guild: G, user: U }), sink()).await.unwrap_err();
        assert_eq!(err.0, "write blocked: DELETE /guilds/1/members/2");
        assert!(untouched(&listener).await);
    }

    #[tokio::test]
    async fn agents_can_never_moderate() {
        assert!(!permitted(true, true));
        refused_offline(permitted(true, true)).await;
        if agent_session() {
            assert!(!set_writes_enabled(true));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let rest = rest_at(listener.local_addr().unwrap());
            assert!(crate::commands::exec::request(rest, moderate(ModAction::Ban(0)), sink()).await.is_err());
            assert!(untouched(&listener).await);
        }
    }

    #[tokio::test]
    async fn allowed_moderation_goes_through_the_normal_send_path() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rest = rest_at(listener.local_addr().unwrap());
        let server = tokio::spawn(one_request(listener));
        let out = crate::commands::exec::request_with(true, rest, moderate(ModAction::Ban(60)), sink()).await.unwrap();
        assert!(matches!(out, crate::commands::ParityReply::Done));
        let req = server.await.unwrap();
        let lower = req.to_ascii_lowercase();
        assert!(req.starts_with("PUT /guilds/1/bans/2 HTTP/1.1"), "{req}");
        assert!(lower.contains("authorization: test-token"));
        assert!(lower.contains("x-super-properties:"));
        assert!(req.contains(r#""delete_message_seconds":60"#), "{req}");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rest = rest_at(listener.local_addr().unwrap());
        let server = tokio::spawn(one_request(listener));
        let out = crate::actions::exec::request_with(true, rest, ActionRequest::Admin(AdminCall::Kick { guild: G, user: U }), sink()).await.unwrap();
        assert!(matches!(out, crate::actions::ActionReply::Json(serde_json::Value::Null)));
        assert!(server.await.unwrap().starts_with("DELETE /guilds/1/members/2 HTTP/1.1"));
    }

    #[test]
    fn stage_moderator_calls_are_moderation() {
        use phoenix_api::rest::stage::StageCall as S;
        let ch = Snowflake(9);
        for c in [S::SetUserSuppress { guild: G, channel: ch, user: U, suppress: false }, S::MoveUserToAudience { guild: G, channel: ch, user: U }, S::End { channel: ch }] {
            assert!(refuse_action(false, &ActionRequest::Stage(c.clone())).is_some());
            assert!(refuse_action(true, &ActionRequest::Stage(c)).is_none());
        }
        assert!(refuse_action(false, &ActionRequest::Stage(S::RequestToSpeak { guild: G, channel: ch, at: None })).is_none());
    }

    #[test]
    fn other_admin_calls_are_not_moderation() {
        for c in [AdminCall::JoinThread { thread: Snowflake(4), location: "x".into() }, AdminCall::Member { guild: G, user: U }, AdminCall::ScheduledMessages] {
            assert!(refuse_action(false, &ActionRequest::Admin(c)).is_none());
        }
        assert!(refuse_parity(false, &ParityRequest::SavedMessages).is_none());
        assert!(refuse_parity(true, &moderate(ModAction::Kick)).is_none());
    }
}
