use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Body, RestClient, moderation::rfc3339};
use crate::error::Result;
use crate::models::{Channel, ChannelId, GuildId, Message, MessageId, ReactionEmoji, Snowflake};

pub const REFERENCE_FORWARD: u8 = 1;

pub fn emoji_source_path(emoji: u64) -> String {
    format!("/emojis/{emoji}/source")
}

pub fn forward_body(channel: ChannelId, message: MessageId, guild: Option<GuildId>, nonce: u64) -> Value {
    let mut r = json!({ "type": REFERENCE_FORWARD, "channel_id": channel, "message_id": message });
    if let Some(g) = guild {
        r["guild_id"] = json!(g);
    }
    json!({ "message_reference": r, "nonce": nonce.to_string(), "content": "", "flags": 0, "tts": false })
}

pub fn thread_body(name: &str) -> Value {
    let name: String = name.trim().chars().take(100).collect();
    json!({ "name": if name.is_empty() { "New Thread".to_owned() } else { name }, "auto_archive_duration": 4320, "location": "Message" })
}

pub fn reminder_body(channel: ChannelId, message: MessageId, due_unix: u64) -> Value {
    json!({ "channel_id": channel, "message_id": message, "due_at": rfc3339(due_unix), "save_data": { "type": 0 } })
}

#[derive(Debug, Clone)]
pub struct Reminder {
    pub message: Option<Message>,
    pub channel_id: Option<Snowflake>,
    pub message_id: Option<Snowflake>,
    pub due_at: String,
}

pub fn parse_reminders(v: &Value) -> Vec<Reminder> {
    let mut out = Vec::new();
    let mut scan = |arr: Option<&Vec<Value>>, need_due: bool| {
        for e in arr.into_iter().flatten() {
            let due = e
                .get("due_at")
                .or_else(|| e.get("save_data").and_then(|s| s.get("due_at")))
                .and_then(Value::as_str)
                .map(str::to_owned);
            if need_due && due.is_none() {
                continue;
            }
            let sd = e.get("save_data").unwrap_or(e);
            let idf = |k: &str| e.get(k).or_else(|| sd.get(k)).and_then(Value::as_str).and_then(|s| s.parse().ok()).map(Snowflake);
            out.push(Reminder {
                message: e.get("message").and_then(|m| serde_json::from_value(m.clone()).ok()),
                channel_id: idf("channel_id"),
                message_id: idf("message_id"),
                due_at: due.unwrap_or_default(),
            });
        }
    };
    scan(v.get("reminders").and_then(Value::as_array), false);
    scan(v.get("results").and_then(Value::as_array), true);
    out
}

#[derive(Debug, Clone, Deserialize)]
struct Created {
    id: Snowflake,
}

impl RestClient {
    pub async fn forward_message(&self, to: ChannelId, from: ChannelId, message: MessageId, guild: Option<GuildId>) -> Result<Message> {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        self.call(Method::POST, &format!("/channels/{to}/messages"), Body::Json(forward_body(from, message, guild, nonce << 22))).await
    }

    pub async fn create_thread_from_message(&self, channel: ChannelId, message: MessageId, name: &str) -> Result<Channel> {
        self.call(Method::POST, &format!("/channels/{channel}/messages/{message}/threads"), Body::Json(thread_body(name))).await
    }

    pub async fn set_reminder(&self, channel: ChannelId, message: MessageId, due_unix: u64) -> Result<()> {
        self.call_empty(Method::PUT, "/users/@me/saved-messages", Body::Json(reminder_body(channel, message, due_unix))).await
    }

    pub async fn get_reminders(&self) -> Result<Vec<Reminder>> {
        let v: Value = self.call(Method::GET, "/users/@me/saved-messages", Body::None).await?;
        Ok(parse_reminders(&v))
    }

    pub async fn get_emoji_source(&self, emoji: u64) -> Result<Value> {
        self.call(Method::GET, &emoji_source_path(emoji), Body::None).await
    }

    pub async fn remove_reactions(&self, channel: ChannelId, message: MessageId, emoji: Option<&ReactionEmoji>) -> Result<()> {
        self.call_empty(Method::DELETE, &reactions_path(channel, message, emoji), Body::None).await
    }

    pub async fn post_for_id(&self, path: &str, body: Value) -> Result<Snowflake> {
        let c: Created = self.call(Method::POST, path, Body::Json(body)).await?;
        Ok(c.id)
    }
}

pub fn reactions_path(channel: ChannelId, message: MessageId, emoji: Option<&ReactionEmoji>) -> String {
    let base = format!("/channels/{channel}/messages/{message}/reactions");
    match emoji {
        Some(e) => {
            let enc: String = url::form_urlencoded::byte_serialize(e.route_form().as_bytes()).collect();
            format!("{base}/{enc}")
        }
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward() {
        let v = forward_body(Snowflake(1), Snowflake(2), Some(Snowflake(3)), 42);
        assert_eq!(v["message_reference"]["type"], 1);
        assert_eq!(v["message_reference"]["message_id"], "2");
        assert_eq!(v["message_reference"]["channel_id"], "1");
        assert_eq!(v["message_reference"]["guild_id"], "3");
        assert_eq!(v["nonce"], "42");
        assert!(forward_body(Snowflake(1), Snowflake(2), None, 1)["message_reference"].get("guild_id").is_none());
    }

    #[test]
    fn thread_and_reminder() {
        assert_eq!(thread_body("  hi ")["name"], "hi");
        assert_eq!(thread_body("")["name"], "New Thread");
        assert_eq!(thread_body(&"x".repeat(300))["name"].as_str().unwrap().len(), 100);
        let r = reminder_body(Snowflake(1), Snowflake(2), 60);
        assert_eq!(r["due_at"], "1970-01-01T00:01:00Z");
        assert_eq!(r["message_id"], "2");
    }

    #[test]
    fn reminders_parse() {
        let v: Value = serde_json::from_str(
            r#"{"results":[{"save_data":{"channel_id":"5","message_id":"6","due_at":"2026-01-01T00:00:00Z"}},{"save_data":{"type":0}}],
                "reminders":[{"channel_id":"7","message_id":"8","due_at":"x"}]}"#,
        )
        .unwrap();
        let r = parse_reminders(&v);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].message_id, Some(Snowflake(8)));
        assert_eq!(r[1].channel_id, Some(Snowflake(5)));
        assert_eq!(r[1].due_at, "2026-01-01T00:00:00Z");
    }

    #[test]
    fn reaction_paths() {
        assert_eq!(reactions_path(Snowflake(1), Snowflake(2), None), "/channels/1/messages/2/reactions");
        let e = ReactionEmoji { id: Some(Snowflake(9)), name: Some("cat".into()), animated: false };
        assert_eq!(reactions_path(Snowflake(1), Snowflake(2), Some(&e)), "/channels/1/messages/2/reactions/cat%3A9");
    }
}
