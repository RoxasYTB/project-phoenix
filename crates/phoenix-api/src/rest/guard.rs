use std::sync::atomic::{AtomicBool, Ordering};

use reqwest::Method;
use serde_json::{Map, Value, json};

use super::{Body, RestClient};
use crate::error::{Error, Result};

pub const CHANGE_PHONE_REASON: &str = "user_settings_update";

pub const AGENT_ENV: &str = "PHX_AGENT";

static WRITES_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn agent_session() -> bool {
    std::env::var_os(AGENT_ENV).is_some()
}

pub fn permitted(on: bool, agent: bool) -> bool {
    on && !agent
}

pub fn set_writes_enabled(on: bool) -> bool {
    let on = permitted(on, agent_session());
    WRITES_ENABLED.store(on, Ordering::SeqCst);
    on
}

pub fn writes_enabled() -> bool {
    permitted(WRITES_ENABLED.load(Ordering::SeqCst), agent_session())
}

pub fn read_only(method: &Method) -> bool {
    *method == Method::GET
}

pub fn blocked(method: &Method, path: impl Into<String>) -> Error {
    Error::WriteBlocked { method: method.to_string(), path: path.into() }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuardedWrite {
    pub method: Method,
    pub path: String,
    pub body: Option<Value>,
    pub form: Option<super::form::FormBody>,
}

fn write(method: Method, path: impl Into<String>, body: Option<Value>) -> GuardedWrite {
    GuardedWrite { method, path: path.into(), body, form: None }
}

fn me_patch(fields: &[(&str, Option<&str>)]) -> GuardedWrite {
    let mut body = Map::new();
    for (k, v) in fields.iter().filter_map(|(k, v)| v.map(|v| (*k, v))) {
        body.insert(k.to_owned(), Value::String(v.to_owned()));
    }
    write(Method::PATCH, "/users/@me", Some(Value::Object(body)))
}

impl GuardedWrite {
    pub fn change_password(password: &str, new_password: &str) -> Self {
        me_patch(&[("password", Some(password)), ("new_password", Some(new_password))])
    }

    pub fn change_username(username: &str, discriminator: Option<&str>, password: &str) -> Self {
        me_patch(&[("username", Some(username)), ("password", Some(password)), ("discriminator", discriminator)])
    }

    pub fn change_email(email: &str, email_token: Option<&str>, password: &str) -> Self {
        me_patch(&[("email", Some(email)), ("email_token", email_token), ("password", Some(password))])
    }

    pub fn send_email_code() -> Self {
        write(Method::PUT, "/users/@me/email", None)
    }

    pub fn verify_email_code(code: &str) -> Self {
        write(Method::POST, "/users/@me/email/verify-code", Some(json!({ "code": code })))
    }

    pub fn username_attempt(username: &str) -> Self {
        write(Method::POST, "/users/@me/pomelo-attempt", Some(json!({ "username": username })))
    }

    pub fn begin_add_phone(phone: &str) -> Self {
        write(Method::POST, "/users/@me/phone", Some(json!({ "phone": phone, "change_phone_reason": CHANGE_PHONE_REASON })))
    }

    pub fn verify_phone(phone: &str, code: &str) -> Self {
        write(Method::POST, "/phone-verifications/verify", Some(json!({ "phone": phone, "code": code })))
    }

    pub fn add_phone(phone_token: &str, password: &str) -> Self {
        write(
            Method::POST,
            "/users/@me/phone",
            Some(json!({ "phone_token": phone_token, "password": password, "change_phone_reason": CHANGE_PHONE_REASON })),
        )
    }

    pub fn remove_phone(password: &str) -> Self {
        write(Method::DELETE, "/users/@me/phone", Some(json!({ "password": password, "change_phone_reason": CHANGE_PHONE_REASON })))
    }

    pub fn disable_sms(password: &str) -> Self {
        write(Method::POST, "/users/@me/mfa/sms/disable", Some(json!({ "password": password })))
    }

    pub fn disable_totp() -> Self {
        write(Method::POST, "/users/@me/mfa/totp/disable", None)
    }

    pub fn backup_codes_challenge(password: &str) -> Self {
        write(Method::POST, "/auth/verify/view-backup-codes-challenge", Some(json!({ "password": password })))
    }

    pub fn disable_account(password: &str) -> Self {
        write(Method::POST, "/users/@me/disable", Some(json!({ "password": password })))
    }

    pub fn delete_account(password: &str) -> Self {
        write(Method::POST, "/users/@me/delete", Some(json!({ "password": password })))
    }

    pub fn rename_webauthn(credential: &str, name: &str) -> Self {
        write(Method::PATCH, format!("/users/@me/mfa/webauthn/credentials/{credential}"), Some(json!({ "name": name })))
    }

    pub fn delete_webauthn(credential: &str) -> Self {
        write(Method::DELETE, format!("/users/@me/mfa/webauthn/credentials/{credential}"), None)
    }

    pub fn put_profile_widgets(widgets: Value) -> Self {
        write(Method::PUT, "/users/@me/widgets", Some(json!({ "widgets": widgets })))
    }

    pub fn add_role_members(guild: u64, role: u64, members: &[u64]) -> Self {
        let ids: Vec<String> = members.iter().map(u64::to_string).collect();
        write(Method::PATCH, format!("/guilds/{guild}/roles/{role}/members"), Some(json!({ "member_ids": ids })))
    }

    pub fn set_member_roles(guild: u64, user: u64, roles: &[u64]) -> Self {
        let ids: Vec<String> = roles.iter().map(u64::to_string).collect();
        write(Method::PATCH, format!("/guilds/{guild}/members/{user}"), Some(json!({ "roles": ids })))
    }

    pub fn unban(guild: u64, user: u64) -> Self {
        write(Method::DELETE, format!("/guilds/{guild}/bans/{user}"), None)
    }

    pub fn create_role(guild: u64, name: &str) -> Self {
        write(
            Method::POST,
            format!("/guilds/{guild}/roles"),
            Some(json!({ "name": name, "color": 0, "colors": { "primary_color": 0, "secondary_color": null, "tertiary_color": null }, "permissions": "0" })),
        )
    }

    pub fn duplicate_role(guild: u64, name: &str, role: &Value) -> Self {
        let mut body = Map::new();
        body.insert("name".into(), json!(name));
        for k in ["color", "colors", "permissions", "hoist", "mentionable", "icon", "unicode_emoji", "description"] {
            if let Some(v) = role.get(k).filter(|v| !v.is_null()) {
                body.insert(k.into(), v.clone());
            }
        }
        write(Method::POST, format!("/guilds/{guild}/roles"), Some(Value::Object(body)))
    }

    pub fn delete_role(guild: u64, role: u64) -> Self {
        write(Method::DELETE, format!("/guilds/{guild}/roles/{role}"), None)
    }

    pub fn prune(guild: u64, days: u32, roles: &[u64]) -> Self {
        let roles: Vec<String> = roles.iter().map(u64::to_string).collect();
        write(Method::POST, format!("/guilds/{guild}/prune"), Some(json!({ "days": days, "compute_prune_count": false, "include_roles": roles })))
    }

    pub fn create_webhook(channel: u64, name: &str) -> Self {
        write(Method::POST, format!("/channels/{channel}/webhooks"), Some(json!({ "name": name })))
    }

    pub fn wishlist_visibility(wishlist: &str, public: bool) -> Self {
        write(Method::PATCH, format!("/users/@me/wishlists/{wishlist}"), Some(json!({ "visibility": u8::from(public) })))
    }

    pub fn wishlist_add_sku(sku: &str) -> Self {
        write(Method::POST, "/users/@me/wishlist/items", Some(json!({ "sku_id": sku })))
    }

    pub fn wishlist_remove_sku(wishlist: &str, sku: &str) -> Self {
        write(Method::DELETE, format!("/users/@me/wishlists/{wishlist}/items/{sku}"), None)
    }

    pub fn crosspost(channel: u64, message: u64) -> Self {
        write(Method::POST, format!("/channels/{channel}/messages/{message}/crosspost"), None)
    }

    pub fn voice_channel_status(channel: u64, status: &str) -> Self {
        write(Method::PUT, format!("/channels/{channel}/voice-status"), Some(json!({ "status": status })))
    }

    pub fn change_call_region(channel: u64, region: &str) -> Self {
        write(Method::PATCH, format!("/channels/{channel}/call"), Some(json!({ "region": region })))
    }

    pub fn thread_from_message(channel: u64, message: u64, name: &str) -> Self {
        write(Method::POST, format!("/channels/{channel}/messages/{message}/threads"), Some(json!({ "name": name })))
    }

    pub fn put_onboarding(guild: u64, body: Value) -> Self {
        write(Method::PUT, format!("/guilds/{guild}/onboarding"), Some(body))
    }

    pub fn put_home_settings(guild: u64, body: Value) -> Self {
        write(Method::PUT, format!("/guilds/{guild}/new-member-welcome"), Some(body))
    }

    pub fn put_channel_overwrite(channel: u64, overwrite: u64, body: Value) -> Self {
        write(Method::PUT, format!("/channels/{channel}/permissions/{overwrite}"), Some(body))
    }

    pub fn automod_alert_action(guild: u64, channel: u64, message: u64, action_type: u8) -> Self {
        write(
            Method::POST,
            format!("/guilds/{guild}/auto-moderation/alert-action"),
            Some(json!({ "message_id": message.to_string(), "channel_id": channel.to_string(), "alert_action_type": action_type })),
        )
    }

    pub fn automod_clear_mention_raid(guild: u64) -> Self {
        write(Method::POST, format!("/guilds/{guild}/auto-moderation/clear-mention-raid"), None)
    }

    pub fn edit_guild(guild: u64, body: Value) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}"), Some(body))
    }

    pub fn member_verification(guild: u64, form_fields: Value, enabled: bool) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}/member-verification"), Some(json!({ "form_fields": form_fields, "enabled": enabled })))
    }

    pub fn guild_profile_visibility(guild: u64, visibility: u8) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}/profile"), Some(json!({ "visibility": visibility })))
    }

    pub fn discovery_metadata(guild: u64, body: Value) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}/discovery-metadata"), Some(body))
    }

    pub fn discovery_category(guild: u64, category: u64, add: bool) -> Self {
        write(if add { Method::PUT } else { Method::DELETE }, format!("/guilds/{guild}/discovery-categories/{category}"), None)
    }

    pub fn start_webauthn_register() -> Self {
        write(Method::POST, "/users/@me/mfa/webauthn/credentials", Some(json!({})))
    }

    pub fn join_guild(guild: u64) -> Self {
        write(Method::PUT, format!("/guilds/{guild}/members/@me"), None)
    }

    pub fn create_sticker(guild: u64, sticker: &super::form::StickerUpload) -> Self {
        GuardedWrite { form: Some(sticker.form()), ..write(Method::POST, format!("/guilds/{guild}/stickers"), None) }
    }

    pub fn create_sound(guild: u64, sound: &super::form::SoundUpload) -> Self {
        write(Method::POST, format!("/guilds/{guild}/soundboard-sounds"), Some(sound.body()))
    }

    pub fn report_raid(guild: u64) -> Self {
        write(Method::POST, format!("/guilds/{guild}/auto-moderation/report-raid"), None)
    }

    pub fn quest_enroll(quest: u64, location: u8) -> Self {
        write(Method::POST, format!("/quests/{quest}/enroll"), Some(json!({ "location": location, "metadata_sealed": null, "traffic_metadata_sealed": null })))
    }

    pub fn quest_dismiss(quest: u64, content: u8) -> Self {
        write(Method::POST, format!("/quests/{quest}/dismissible-content/{content}/dismiss"), Some(json!({ "traffic_metadata_sealed": null })))
    }

    pub fn quest_video_progress(quest: u64, timestamp: f64) -> Self {
        write(Method::POST, format!("/quests/{quest}/video-progress"), Some(json!({ "timestamp": timestamp })))
    }

    pub fn accept_game_friend(user: u64, application: u64) -> Self {
        write(Method::PUT, format!("/users/@me/game-relationships/{user}/{application}"), Some(json!({ "type": 1 })))
    }

    pub fn remove_game_relationship(user: u64, application: u64) -> Self {
        write(Method::DELETE, format!("/users/@me/game-relationships/{user}/{application}"), None)
    }

    pub fn ring_call(channel: u64) -> Self {
        write(Method::POST, format!("/channels/{channel}/call/ring"), Some(json!({ "recipients": null })))
    }

    pub fn batch_channel_update(guild: u64, body: Value) -> Self {
        write(Method::PATCH, format!("/guilds/{guild}/channels"), Some(body))
    }

    pub fn edit_channel(channel: u64, body: Value) -> Self {
        write(Method::PATCH, format!("/channels/{channel}"), Some(body))
    }

    pub fn sku_purchase(sku: u64, gift: bool, amount: i64, currency: &str) -> Self {
        write(
            Method::POST,
            format!("/store/skus/{sku}/purchase"),
            Some(json!({ "gift": gift, "sku_subscription_plan_id": null, "expected_amount": amount, "expected_currency": currency, "apply_wallet_balance": false })),
        )
    }

    pub fn add_recipient(channel: u64, user: u64) -> Self {
        write(Method::PUT, format!("/channels/{channel}/recipients/{user}"), None)
    }

    pub fn remove_user_reaction(channel: u64, message: u64, emoji: &crate::models::ReactionEmoji, user: u64) -> Self {
        let e: String = url::form_urlencoded::byte_serialize(emoji.route_form().as_bytes()).collect();
        write(Method::DELETE, format!("/channels/{channel}/messages/{message}/reactions/{e}/{user}"), None)
    }

    pub fn set_guild_identity(guild: u64, enabled: bool) -> Self {
        write(Method::PUT, "/users/@me/clan", Some(json!({ "identity_guild_id": guild.to_string(), "identity_enabled": enabled })))
    }

    pub fn burst_reaction(channel: u64, message: u64, emoji: &crate::models::ReactionEmoji, add: bool) -> Self {
        let e: String = url::form_urlencoded::byte_serialize(emoji.route_form().as_bytes()).collect();
        let base = format!("/channels/{channel}/messages/{message}/reactions/{e}");
        if add {
            write(Method::PUT, format!("{base}/@me?location=Message&type=1"), None)
        } else {
            write(Method::DELETE, format!("{base}/1/@me?location=Message&burst=true"), None)
        }
    }

    pub fn remove_user_burst_reaction(channel: u64, message: u64, emoji: &crate::models::ReactionEmoji, user: u64) -> Self {
        let e: String = url::form_urlencoded::byte_serialize(emoji.route_form().as_bytes()).collect();
        write(Method::DELETE, format!("/channels/{channel}/messages/{message}/reactions/{e}/1/{user}?location=Message&burst=true"), None)
    }

    pub fn set_note(user: u64, note: &str) -> Self {
        write(Method::PUT, format!("/users/@me/notes/{user}"), Some(json!({ "note": note })))
    }

    pub fn refusal(&self) -> Error {
        blocked(&self.method, self.path.clone())
    }
}

impl RestClient {
    pub async fn guarded_write(&self, w: &GuardedWrite) -> Result<Value> {
        if !writes_enabled() {
            return Err(w.refusal());
        }
        self.send_guarded(w).await
    }

    async fn send_guarded(&self, w: &GuardedWrite) -> Result<Value> {
        let body = match (&w.form, &w.body) {
            (Some(f), _) => Body::Form(f.clone()),
            (None, Some(v)) => Body::Json(v.clone()),
            (None, None) => Body::None,
        };
        let (_, bytes) = self.raw(w.method.clone(), &w.path, body).await?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_write() -> Vec<GuardedWrite> {
        vec![
            GuardedWrite::change_password("old", "new"),
            GuardedWrite::batch_channel_update(5, json!([{ "id": "6", "position": 0 }])),
            GuardedWrite::voice_channel_status(7, "status"),
            GuardedWrite::set_guild_identity(5, true),
            GuardedWrite::set_note(3, "note"),
            GuardedWrite::remove_user_reaction(1, 2, &crate::models::ReactionEmoji { id: None, name: Some("👍".into()), animated: false }, 3),
            GuardedWrite::burst_reaction(1, 2, &crate::models::ReactionEmoji { id: None, name: Some("👍".into()), animated: false }, true),
            GuardedWrite::burst_reaction(1, 2, &crate::models::ReactionEmoji { id: None, name: Some("👍".into()), animated: false }, false),
            GuardedWrite::remove_user_burst_reaction(1, 2, &crate::models::ReactionEmoji { id: None, name: Some("👍".into()), animated: false }, 3),
            GuardedWrite::change_username("name", None, "pw"),
            GuardedWrite::change_email("a@b.c", Some("tok"), "pw"),
            GuardedWrite::send_email_code(),
            GuardedWrite::verify_email_code("123456"),
            GuardedWrite::username_attempt("name"),
            GuardedWrite::begin_add_phone("+33600000000"),
            GuardedWrite::verify_phone("+33600000000", "123456"),
            GuardedWrite::add_phone("tok", "pw"),
            GuardedWrite::remove_phone("pw"),
            GuardedWrite::disable_sms("pw"),
            GuardedWrite::disable_totp(),
            GuardedWrite::backup_codes_challenge("pw"),
            GuardedWrite::wishlist_visibility("1", true),
            GuardedWrite::wishlist_add_sku("2"),
            GuardedWrite::crosspost(3, 4),
            GuardedWrite::thread_from_message(3, 4, "t"),
            GuardedWrite::put_onboarding(5, json!({ "default_channel_ids": ["6"] })),
            GuardedWrite::put_home_settings(5, json!({ "guild_id": "5" })),
            GuardedWrite::put_channel_overwrite(6, 5, json!({ "id": "5", "type": 0 })),
            GuardedWrite::change_call_region(5, "rotterdam"),
            GuardedWrite::automod_alert_action(5, 6, 7, 3),
            GuardedWrite::automod_clear_mention_raid(5),
            GuardedWrite::edit_guild(5, json!({ "owner_configured_content_level": 3 })),
            GuardedWrite::member_verification(5, json!([]), false),
            GuardedWrite::guild_profile_visibility(5, 3),
            GuardedWrite::discovery_metadata(5, json!({ "keywords": [] })),
            GuardedWrite::discovery_category(5, 49, true),
            GuardedWrite::discovery_category(5, 49, false),
            GuardedWrite::start_webauthn_register(),
            GuardedWrite::create_sticker(5, &super::super::form::tests::sticker()),
            GuardedWrite::create_sound(5, &super::super::form::tests::sound()),
            GuardedWrite::report_raid(5),
            GuardedWrite::quest_enroll(8, 11),
            GuardedWrite::ring_call(9),
            GuardedWrite::add_recipient(9, 2),
            GuardedWrite::disable_account("pw"),
            GuardedWrite::unban(5, 7),
            GuardedWrite::add_role_members(5, 6, &[7]),
            GuardedWrite::set_member_roles(5, 7, &[]),
            GuardedWrite::rename_webauthn("9", "Clé"),
            GuardedWrite::delete_webauthn("9"),
            GuardedWrite::delete_account("pw"),
            GuardedWrite::create_role(5, "new role"),
            GuardedWrite::prune(5, 7, &[6]),
            GuardedWrite::create_webhook(6, "Captain Hook"),
            GuardedWrite::sku_purchase(7, true, 599, "eur"),
            GuardedWrite::quest_dismiss(8, 11),
            GuardedWrite::quest_video_progress(8, 0.),
        ]
    }

    #[test]
    fn only_get_is_read_only() {
        assert!(read_only(&Method::GET));
        for m in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::HEAD, Method::OPTIONS] {
            assert!(!read_only(&m), "{m}");
        }
        assert!(blocked(&Method::DELETE, "/guilds/1/members/2").is_write_blocked());
    }

    #[test]
    fn bodies_match_the_client() {
        let pw = GuardedWrite::change_password("old", "new");
        assert_eq!((pw.method.clone(), pw.path.as_str()), (Method::PATCH, "/users/@me"));
        assert_eq!(pw.body, Some(json!({ "password": "old", "new_password": "new" })));
        let un = GuardedWrite::change_username("neo", None, "pw");
        assert_eq!(un.body, Some(json!({ "username": "neo", "password": "pw" })));
        let em = GuardedWrite::change_email("a@b.c", Some("tok"), "pw");
        assert_eq!(em.body, Some(json!({ "email": "a@b.c", "email_token": "tok", "password": "pw" })));
        assert_eq!(GuardedWrite::send_email_code().method, Method::PUT);
        let rm = GuardedWrite::remove_phone("pw");
        assert_eq!((rm.method, rm.body), (Method::DELETE, Some(json!({ "password": "pw", "change_phone_reason": "user_settings_update" }))));
        assert_eq!(GuardedWrite::wishlist_visibility("9", false).body, Some(json!({ "visibility": 0 })));
        let rm = GuardedWrite::wishlist_remove_sku("9", "42");
        assert_eq!((rm.method, rm.path.as_str(), rm.body), (Method::DELETE, "/users/@me/wishlists/9/items/42", None));
        assert_eq!(GuardedWrite::crosspost(3, 4).path, "/channels/3/messages/4/crosspost");
        let ob = GuardedWrite::put_onboarding(5, json!({ "prompts": [] }));
        assert_eq!((ob.method, ob.path.as_str(), ob.body), (Method::PUT, "/guilds/5/onboarding", Some(json!({ "prompts": [] }))));
        let home = GuardedWrite::put_home_settings(5, json!({ "enabled": false }));
        assert_eq!((home.method, home.path.as_str()), (Method::PUT, "/guilds/5/new-member-welcome"));
        let region = GuardedWrite::change_call_region(5, "rotterdam");
        assert_eq!((region.method, region.path.as_str(), region.body), (Method::PATCH, "/channels/5/call", Some(json!({ "region": "rotterdam" }))));
        let am = GuardedWrite::automod_alert_action(5, 6, 7, 3);
        assert_eq!(
            (am.method, am.path.as_str(), am.body),
            (Method::POST, "/guilds/5/auto-moderation/alert-action", Some(json!({ "message_id": "7", "channel_id": "6", "alert_action_type": 3 })))
        );
        assert_eq!(GuardedWrite::automod_clear_mention_raid(5).path, "/guilds/5/auto-moderation/clear-mention-raid");
        let mv = GuardedWrite::member_verification(5, json!([{ "field_type": "TERMS" }]), true);
        assert_eq!(
            (mv.method, mv.path.as_str(), mv.body),
            (Method::PATCH, "/guilds/5/member-verification", Some(json!({ "form_fields": [{ "field_type": "TERMS" }], "enabled": true })))
        );
        assert_eq!(GuardedWrite::guild_profile_visibility(5, 1).body, Some(json!({ "visibility": 1 })));
        assert_eq!(GuardedWrite::edit_guild(5, json!({})).path, "/guilds/5");
        let add = GuardedWrite::discovery_category(5, 49, true);
        assert_eq!((add.method, add.path.as_str(), add.body), (Method::PUT, "/guilds/5/discovery-categories/49", None));
        assert_eq!(GuardedWrite::discovery_category(5, 49, false).method, Method::DELETE);
        let thread = GuardedWrite::edit_channel(8, json!({ "archived": false }));
        assert_eq!((thread.method, thread.path.as_str()), (Method::PATCH, "/channels/8"));
        let raid = GuardedWrite::report_raid(5);
        assert_eq!((raid.method, raid.path.as_str(), raid.body), (Method::POST, "/guilds/5/auto-moderation/report-raid", None));
        let quest = GuardedWrite::quest_enroll(8, 11);
        assert_eq!(
            (quest.method, quest.path.as_str(), quest.body),
            (Method::POST, "/quests/8/enroll", Some(json!({ "location": 11, "metadata_sealed": null, "traffic_metadata_sealed": null })))
        );
        let ring = GuardedWrite::ring_call(9);
        assert_eq!((ring.method, ring.path.as_str(), ring.body), (Method::POST, "/channels/9/call/ring", Some(json!({ "recipients": null }))));
        let game = GuardedWrite::accept_game_friend(3, 4);
        assert_eq!((game.method, game.path.as_str(), game.body), (Method::PUT, "/users/@me/game-relationships/3/4", Some(json!({ "type": 1 }))));
        let drop = GuardedWrite::remove_game_relationship(3, 4);
        assert_eq!((drop.method, drop.path.as_str(), drop.body), (Method::DELETE, "/users/@me/game-relationships/3/4", None));
        assert_eq!(GuardedWrite::quest_dismiss(8, 11).path, "/quests/8/dismissible-content/11/dismiss");
        assert_eq!(GuardedWrite::quest_video_progress(8, 2.5).body, Some(json!({ "timestamp": 2.5 })));
        let buy = GuardedWrite::sku_purchase(7, false, 120, "discord_orb");
        assert_eq!((buy.method.clone(), buy.path.as_str()), (Method::POST, "/store/skus/7/purchase"));
        assert_eq!(buy.body.as_ref().and_then(|b| b.get("expected_currency")).and_then(Value::as_str), Some("discord_orb"));
        assert_eq!(buy.body.as_ref().and_then(|b| b.get("gift")), Some(&json!(false)));
        let add = GuardedWrite::add_recipient(9, 2);
        assert_eq!((add.method, add.path.as_str(), add.body), (Method::PUT, "/channels/9/recipients/2", None));
        let ub = GuardedWrite::unban(5, 7);
        assert_eq!((ub.method, ub.path.as_str()), (Method::DELETE, "/guilds/5/bans/7"));
        let dis = GuardedWrite::disable_account("pw");
        assert_eq!((dis.method, dis.path.as_str(), dis.body), (Method::POST, "/users/@me/disable", Some(json!({ "password": "pw" }))));
        assert_eq!(GuardedWrite::delete_account("pw").path, "/users/@me/delete");
        let rn = GuardedWrite::rename_webauthn("9", "Clé");
        assert_eq!((rn.method, rn.path.as_str(), rn.body), (Method::PATCH, "/users/@me/mfa/webauthn/credentials/9", Some(json!({ "name": "Clé" }))));
        assert_eq!(GuardedWrite::delete_webauthn("9").method, Method::DELETE);
        let wd = GuardedWrite::put_profile_widgets(json!([{ "data": { "type": "favorite_games", "games": [] } }]));
        assert_eq!((wd.method, wd.path.as_str()), (Method::PUT, "/users/@me/widgets"));
        assert_eq!(wd.body, Some(json!({ "widgets": [{ "data": { "type": "favorite_games", "games": [] } }] })));
        let add = GuardedWrite::add_role_members(5, 6, &[7, 8]);
        assert_eq!((add.method, add.path.as_str(), add.body), (Method::PATCH, "/guilds/5/roles/6/members", Some(json!({ "member_ids": ["7", "8"] }))));
        assert_eq!(GuardedWrite::set_member_roles(5, 7, &[9]).body, Some(json!({ "roles": ["9"] })));
        let role = GuardedWrite::create_role(5, "nouveau rôle");
        assert_eq!(role.path, "/guilds/5/roles");
        assert_eq!(role.body.unwrap()["permissions"], json!("0"));
        let dup = GuardedWrite::duplicate_role(5, "Copie de Modo", &json!({ "id": "6", "name": "Modo", "color": 255, "hoist": true, "icon": null, "permissions": "8" }));
        assert_eq!((dup.method, dup.path.as_str()), (Method::POST, "/guilds/5/roles"));
        assert_eq!(dup.body, Some(json!({ "name": "Copie de Modo", "color": 255, "hoist": true, "permissions": "8" })));
        let del = GuardedWrite::delete_role(5, 6);
        assert_eq!((del.method, del.path.as_str(), del.body), (Method::DELETE, "/guilds/5/roles/6", None));
        let pr = GuardedWrite::prune(5, 30, &[6, 7]);
        assert_eq!(pr.body, Some(json!({ "days": 30, "compute_prune_count": false, "include_roles": ["6", "7"] })));
        let hook = GuardedWrite::create_webhook(6, "Spidey Bot");
        assert_eq!((hook.method, hook.path.as_str()), (Method::POST, "/channels/6/webhooks"));
        let clan = GuardedWrite::set_guild_identity(5, true);
        assert_eq!(
            (clan.method, clan.path.as_str(), clan.body),
            (Method::PUT, "/users/@me/clan", Some(json!({ "identity_guild_id": "5", "identity_enabled": true })))
        );
    }

    #[tokio::test]
    async fn every_write_is_refused_without_touching_the_network() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let rest = RestClient::with_base(Some("test-token".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        for w in every_write() {
            assert!(!read_only(&w.method), "{} {} is not a write", w.method, w.path);
            let err = rest.guarded_write(&w).await.unwrap_err();
            assert!(err.is_write_blocked(), "{} {} was not blocked: {err}", w.method, w.path);
        }
        let accepted = tokio::time::timeout(std::time::Duration::from_millis(200), listener.accept()).await;
        assert!(accepted.is_err(), "a guarded write opened a connection");
    }

    #[test]
    fn agents_can_never_enable_writes() {
        assert!(permitted(true, false));
        assert!(!permitted(true, true));
        assert!(!permitted(false, false));
        assert!(!permitted(false, true));
        assert!(!writes_enabled());
        if agent_session() {
            assert!(!set_writes_enabled(true));
            assert!(!writes_enabled());
        }
    }

    async fn one_request(listener: tokio::net::TcpListener, reply: &'static str) -> Vec<u8> {
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
        let resp = format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len());
        sock.write_all(resp.as_bytes()).await.unwrap();
        buf
    }

    #[tokio::test]
    async fn an_allowed_write_goes_through_the_normal_send_path() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(one_request(listener, r#"{"id":"42"}"#));
        let rest = RestClient::with_base(Some("test-token".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        let out = rest.send_guarded(&GuardedWrite::create_role(5, "mods")).await.unwrap();
        assert_eq!(out, json!({ "id": "42" }));
        let req = String::from_utf8(server.await.unwrap()).unwrap();
        let lower = req.to_ascii_lowercase();
        assert!(req.starts_with("POST /guilds/5/roles HTTP/1.1"), "{req}");
        assert!(lower.contains("authorization: test-token"));
        assert!(lower.contains("x-super-properties:"));
        assert!(lower.contains("content-type: application/json"));
        assert!(req.contains(r#""name":"mods""#));
    }

    #[tokio::test]
    async fn an_allowed_sticker_upload_is_sent_as_multipart() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(one_request(listener, ""));
        let rest = RestClient::with_base(Some("test-token".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        let out = rest.send_guarded(&GuardedWrite::create_sticker(5, &super::super::form::tests::sticker())).await.unwrap();
        assert_eq!(out, Value::Null);
        let req = String::from_utf8_lossy(&server.await.unwrap()).to_ascii_lowercase();
        assert!(req.starts_with("post /guilds/5/stickers http/1.1"), "{req}");
        assert!(req.contains("content-type: multipart/form-data; boundary=----webkitformboundary"));
        assert!(req.contains("x-discord-original-md5:"));
        assert!(req.contains("name=\"tags\""));
    }

    #[test]
    fn removing_a_reactor_targets_the_user_route() {
        let w = GuardedWrite::remove_user_reaction(1, 2, &crate::models::ReactionEmoji { id: Some(crate::models::Snowflake(9)), name: Some("blob".into()), animated: false }, 3);
        assert_eq!(w.method, Method::DELETE);
        assert_eq!(w.path, "/channels/1/messages/2/reactions/blob%3A9/3");
    }
}
