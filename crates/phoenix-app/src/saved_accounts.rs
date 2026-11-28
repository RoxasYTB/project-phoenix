use serde_json::{Value, json};

pub const MAX: usize = 5;
const FILE: &str = "accounts.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedAccount {
    pub id: u64,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

impl SavedAccount {
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().filter(|n| !n.is_empty()).unwrap_or(&self.username)
    }

    pub fn avatar_ref(&self) -> crate::model::ImageRef {
        match &self.avatar {
            Some(h) => {
                let ext = if h.starts_with("a_") { "gif" } else { "png" };
                crate::model::ImageRef::Url(format!("https://cdn.discordapp.com/avatars/{}/{h}.{ext}?size=80", self.id).into())
            }
            None => crate::model::ImageRef::Default(((self.id >> 22) % 6) as u8),
        }
    }
}

pub fn parse(v: &Value) -> Vec<SavedAccount> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| {
                    Some(SavedAccount {
                        id: e.get("id")?.as_str()?.parse().ok()?,
                        username: e.get("username")?.as_str()?.to_owned(),
                        global_name: e.get("global_name").and_then(Value::as_str).map(str::to_owned),
                        avatar: e.get("avatar").and_then(Value::as_str).map(str::to_owned),
                    })
                })
                .take(MAX)
                .collect()
        })
        .unwrap_or_default()
}

pub fn to_json(list: &[SavedAccount]) -> Value {
    Value::Array(list.iter().map(|a| json!({"id": a.id.to_string(), "username": a.username, "global_name": a.global_name, "avatar": a.avatar})).collect())
}

pub fn upsert(list: &mut Vec<SavedAccount>, acct: SavedAccount) {
    list.retain(|a| a.id != acct.id);
    list.insert(0, acct);
    list.truncate(MAX);
}

pub fn load() -> Vec<SavedAccount> {
    if crate::paths::is_sandboxed() {
        return Vec::new();
    }
    crate::paths::config_file(FILE).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| parse(&v)).unwrap_or_default()
}

pub fn save(list: &[SavedAccount]) {
    if crate::paths::is_sandboxed() {
        return;
    }
    let Some(p) = crate::paths::config_file(FILE) else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, to_json(list).to_string());
}

pub fn remember(acct: SavedAccount) {
    let mut list = load();
    if list.first() == Some(&acct) {
        return;
    }
    upsert(&mut list, acct);
    save(&list);
}

pub fn forget(id: u64) {
    let mut list = load();
    let before = list.len();
    list.retain(|a| a.id != id);
    if list.len() != before {
        save(&list);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(id: u64) -> SavedAccount {
        SavedAccount { id, username: format!("u{id}"), global_name: None, avatar: None }
    }

    #[test]
    fn roundtrip_keeps_no_secret_and_orders_by_last_use() {
        let mut list = vec![acct(1), acct(2)];
        upsert(&mut list, acct(2));
        assert_eq!(list.iter().map(|a| a.id).collect::<Vec<_>>(), vec![2, 1]);
        let v = to_json(&list);
        assert!(!v.to_string().contains("token"));
        assert_eq!(parse(&v), list);
    }

    #[test]
    fn list_is_capped_like_the_client() {
        let mut list = Vec::new();
        for i in 0..8 {
            upsert(&mut list, acct(i));
        }
        assert_eq!(list.len(), MAX);
        assert_eq!(list[0].id, 7);
    }

    #[test]
    fn display_name_and_default_avatar() {
        let mut a = acct(175928847299117063);
        assert_eq!(a.display_name(), "u175928847299117063");
        a.global_name = Some("Nom".into());
        assert_eq!(a.display_name(), "Nom");
        assert!(matches!(a.avatar_ref(), crate::model::ImageRef::Default(_)));
    }
}
