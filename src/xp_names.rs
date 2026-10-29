//! Names the cockpit displays use (JS Library for X-Plane) for the variables
//! and bridge calls this plugin keeps under other names internally.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

const TABLE: &str = include_str!("xp_names.txt");

struct Names {
    exact: HashMap<&'static str, &'static str>,
    prefixes: Vec<(&'static str, &'static str)>,
    wire_in: HashMap<&'static str, &'static str>,
    wire_out: HashMap<&'static str, &'static str>,
}

fn names() -> &'static Names {
    static N: OnceLock<Names> = OnceLock::new();
    N.get_or_init(|| {
        let mut n = Names { exact: HashMap::new(), prefixes: Vec::new(), wire_in: HashMap::new(), wire_out: HashMap::new() };
        for line in TABLE.lines() {
            let Some((head, internal)) = line.split_once('\t') else { continue };
            match head.split_at(1) {
                ("=", name) => {
                    n.exact.insert(name, internal);
                }
                (">", name) => n.prefixes.push((name, internal)),
                ("@", name) => {
                    n.wire_in.insert(name, internal);
                    n.wire_out.insert(internal, name);
                }
                _ => {}
            }
        }
        n.prefixes.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
        n
    })
}

/// The plugin's own name for a display variable name.
pub fn var_internal(name: &str) -> Cow<'_, str> {
    let n = names();
    if let Some(internal) = n.exact.get(name) {
        return Cow::Borrowed(internal);
    }
    for (prefix, internal) in &n.prefixes {
        if let Some(rest) = name.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return Cow::Owned(format!("{internal}{rest}"));
            }
        }
    }
    Cow::Borrowed(name)
}

/// The plugin's own name for a call or event the displays send.
pub fn wire_in(name: &str) -> Cow<'_, str> {
    names().wire_in.get(name).map_or(Cow::Borrowed(name), |s| Cow::Borrowed(s))
}

/// The displays' name for an event the plugin sends, with the channel and
/// event names inside a channel event's payload renamed too.
pub fn event_out(name: &str, json: &str) -> (String, String) {
    let n = names();
    let out_name = n.wire_out.get(name).map_or(name, |s| s).to_string();
    if name != "EVENT_FROM_VIEW_LISTENER" {
        return (out_name, json.to_string());
    }
    let Ok(serde_json::Value::Array(mut items)) = serde_json::from_str::<serde_json::Value>(json) else {
        return (out_name, json.to_string());
    };
    for item in items.iter_mut().take(2) {
        if let serde_json::Value::String(s) = item {
            if let Some(renamed) = n.wire_out.get(s.as_str()) {
                *s = renamed.to_string();
            }
        }
    }
    (out_name, serde_json::Value::Array(items).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_prefixes_keep_their_index() {
        let (prefix, internal) = names().prefixes.iter().find(|(_, i)| i.ends_with(':')).copied().expect("an indexed prefix");
        assert_eq!(var_internal(&format!("{prefix}3")), format!("{internal}3"));
    }

    #[test]
    fn unknown_names_pass_through() {
        assert_eq!(var_internal("L:A32NX_ANYTHING"), "L:A32NX_ANYTHING");
        assert_eq!(wire_in("LOAD_AIRPORT"), "LOAD_AIRPORT");
    }

    #[test]
    fn wire_names_round_trip() {
        assert_eq!(wire_in("relay.send"), "GENERIC_DATA");
        let (name, json) = event_out("EVENT_FROM_VIEW_LISTENER", r#"["JS_LISTENER_GENERICDATA","k","d"]"#);
        assert_eq!(name, "channel.event");
        assert_eq!(json, r#"["relay","k","d"]"#);
    }
}
