use phoenix_api::rest::reporting::ReportMenu;

use super::{ActionReply, ActionRequest};
use crate::{backend::BackendError, model::User};

pub fn report_menu(name: &str) -> ReportMenu {
    let json = format!(
        r#"{{"name":"{name}","variant":"mock","version":"1","language":"en","root_node_id":1,"success_node_id":9,"fail_node_id":10,
        "nodes":{{"1":{{"id":1,"key":"start","header":"What's going on?","subheader":"Pick the option that fits best.","children":[["Spam",2],["Abuse or harassment",3],["Something else",3]]}},
                 "2":{{"id":2,"key":"spam","header":"Spam","info":"Unwanted ads, scams or repeated messages.","button":{{"type":"submit","target":9}}}},
                 "3":{{"id":3,"key":"abuse","header":"Tell us more","children":[["Targeted at me",2],["Targeted at someone else",2]]}},
                 "9":{{"id":9,"key":"done","header":"Thanks for the report","subheader":"Mock backend: nothing was sent.","button":{{"type":"done"}}}}}}}}"#
    );
    serde_json::from_str(&json).unwrap_or_default()
}

pub fn mock_emoji_source(id: u64) -> serde_json::Value {
    let emojis: Vec<_> = (0..8u64).map(|i| serde_json::json!({ "id": (id + i).to_string(), "name": format!("phoenix_{i}"), "animated": false, "require_colons": true })).collect();
    serde_json::json!({
        "type": "GUILD",
        "guild": { "id": "1374100000000000001", "name": "Phoenix Lounge", "icon": null, "features": ["DISCOVERABLE"], "approximate_presence_count": 1284, "emojis": emojis }
    })
}

pub fn reply(req: ActionRequest, users: Vec<User>) -> Result<ActionReply, BackendError> {
    use ActionRequest as R;
    Ok(match req {
        R::Reactors { emoji, burst, .. } => {
            let n = (if burst { emoji.burst_count } else { emoji.count }) as usize;
            ActionReply::Users(users.into_iter().take(n.min(6)).collect())
        }
        R::ReportMenu(t) => ActionReply::ReportMenu(Box::new(report_menu(t.menu_name()))),
        R::Reminders => ActionReply::Messages(Vec::new()),
        R::EmojiSource(id) => ActionReply::Json(mock_emoji_source(id)),
        R::CreateThread { channel, .. } | R::CloneChannel { channel, .. } => ActionReply::Channel(channel),
        R::CreateStandaloneThread { channel, .. } => ActionReply::Channel(channel),
        R::Admin(c) | R::AdminMfa(c, _, _) => ActionReply::Json(super::admin::mock_reply(&c)),
        _ => ActionReply::Done,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Id;
    use phoenix_api::rest::reporting::ReportTarget;

    #[test]
    fn mock_menu_walks() {
        let m = report_menu("message");
        assert_eq!(m.node(1).unwrap().children.len(), 3);
        assert_eq!(m.node(2).unwrap().button.as_ref().unwrap().kind, "submit");
        let t = ReportTarget::Message { channel: phoenix_api::models::Snowflake(1), message: phoenix_api::models::Snowflake(2) };
        assert!(matches!(reply(ActionRequest::ReportMenu(t), Vec::new()), Ok(ActionReply::ReportMenu(_))));
        assert!(matches!(reply(ActionRequest::Ignore { user: Id(1), ignored: true }, Vec::new()), Ok(ActionReply::Done)));
    }
}
