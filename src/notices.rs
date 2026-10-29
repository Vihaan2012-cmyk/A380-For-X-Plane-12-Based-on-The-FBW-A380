use std::sync::Mutex;

const KEEP: usize = 20;

static NOTICES: Mutex<(u64, Vec<(u64, String)>)> = Mutex::new((0, Vec::new()));

pub fn post(message: impl Into<String>) {
    let message = message.into();
    crate::log(&format!("efb notice: {message}"));
    if let Ok(mut n) = NOTICES.lock() {
        n.0 += 1;
        let seq = n.0;
        n.1.push((seq, message));
        let excess = n.1.len().saturating_sub(KEEP);
        n.1.drain(..excess);
    }
}

pub fn since(after: u64) -> (u64, Vec<String>) {
    NOTICES.lock().map(|n| (n.0, n.1.iter().filter(|(s, _)| *s > after).map(|(_, m)| m.clone()).collect())).unwrap_or_default()
}

pub fn json(query: &str) -> String {
    let after = query.split('&').find_map(|kv| kv.strip_prefix("after=")).and_then(|v| v.parse().ok()).unwrap_or(0);
    let (seq, messages) = since(after);
    serde_json::json!({ "seq": seq, "messages": messages }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_efb_gets_only_the_notices_it_has_not_seen() {
        let (start, _) = since(u64::MAX);
        post("first");
        post("second");
        let (seq, all) = since(start);
        assert_eq!(all, vec!["first".to_string(), "second".to_string()]);
        let (_, newer) = since(seq - 1);
        assert_eq!(newer, vec!["second".to_string()]);
        assert!(since(seq).1.is_empty());
    }
}
