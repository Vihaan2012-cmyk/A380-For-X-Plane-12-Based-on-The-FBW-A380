use serde_json::Value;

const DENIED: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "apikey",
    "api_key",
    "clientid",
    "client_id",
    "credential",
    "cookie",
    "logon",
    "authcode",
    "auth_code",
    "refresh",
    "simbriefuser",
    "simbrief_user",
    "navigraph",
    "hoppie",
    "email",
];

pub fn is_denied(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    DENIED.iter().any(|d| k.contains(d))
}

pub fn scrub(value: &mut Value) -> usize {
    match value {
        Value::Object(map) => {
            let before = map.len();
            map.retain(|k, _| !is_denied(k));
            let mut removed = before - map.len();
            for v in map.values_mut() {
                removed += scrub(v);
            }
            removed
        }
        Value::Array(items) => items.iter_mut().map(scrub).sum(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn credentials_are_removed_at_any_depth_and_the_rest_kept() {
        let mut v = json!({
            "fuel_kg": 1000,
            "simbriefUserId": "x",
            "efb": { "hoppieLogonCode": "y", "brightness": 0.8, "pages": [{ "navigraphToken": "z", "id": 2 }] },
            "ACCESS_TOKEN": "t"
        });
        assert_eq!(scrub(&mut v), 4);
        assert_eq!(v, json!({ "fuel_kg": 1000, "efb": { "brightness": 0.8, "pages": [{ "id": 2 }] } }));
    }
}
