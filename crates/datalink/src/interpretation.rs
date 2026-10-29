use crate::message::CpdlcMessage;

pub static NON_AUTOMATIC_CLOSING: &[&str] = &[
    "UM127", "UM128", "UM129", "UM130", "UM131", "UM132", "UM133", "UM134", "UM135", "UM136", "UM137", "UM138",
    "UM139", "UM140", "UM141", "UM142", "UM143", "UM144", "UM145", "UM146", "UM147", "UM148", "UM151", "UM152",
    "UM180", "UM181", "UM182", "UM228", "UM231", "UM232",
];

pub static REQUEST_MESSAGES: &[&str] = &[
    "DM15", "DM16", "DM17", "DM18", "DM19", "DM20", "DM21", "DM22", "DM23", "DM24", "DM25", "DM26", "DM27", "DM51",
    "DM52", "DM53", "DM54", "DM69", "DM70", "DM71", "DM72", "DM73", "DM74",
];

pub struct SemanticAnswer {
    pub uplink: &'static str,
    pub positive_or_negative: bool,
    pub modifiable: bool,
    pub messages: &'static [&'static str],
}

macro_rules! answer {
    ($u:expr, $pn:expr, $m:expr, [$($d:expr),+]) => {
        SemanticAnswer { uplink: $u, positive_or_negative: $pn, modifiable: $m, messages: &[$($d),+] }
    };
}

pub static SEMANTIC_ANSWERS: &[SemanticAnswer] = &[
    answer!("UM128", false, false, ["DM28"]),
    answer!("UM129", false, false, ["DM37"]),
    answer!("UM130", false, false, ["DM31"]),
    answer!("UM131", false, true, ["DM57"]),
    answer!("UM132", false, true, ["DM33"]),
    answer!("UM133", false, true, ["DM32"]),
    answer!("UM134", false, true, ["DM34"]),
    answer!("UM135", false, true, ["DM38"]),
    answer!("UM136", false, true, ["DM39"]),
    answer!("UM137", false, true, ["DM40"]),
    answer!("UM138", false, true, ["DM46"]),
    answer!("UM139", false, true, ["DM45"]),
    answer!("UM140", false, true, ["DM42"]),
    answer!("UM141", false, true, ["DM43"]),
    answer!("UM142", false, true, ["DM44"]),
    answer!("UM144", false, true, ["DM47"]),
    answer!("UM145", false, true, ["DM35"]),
    answer!("UM146", false, true, ["DM36"]),
    answer!("UM147", false, true, ["DM48"]),
    answer!("UM148", true, true, ["DM81", "DM82"]),
    answer!("UM151", false, true, ["DM83"]),
    answer!("UM152", true, true, ["DM85", "DM86"]),
    answer!("UM175", false, false, ["DM72"]),
    answer!("UM180", false, false, ["DM76"]),
    answer!("UM181", false, true, ["DM67"]),
    answer!("UM182", false, true, ["DM79"]),
    answer!("UM184", false, true, ["DM67"]),
    answer!("UM228", false, true, ["DM104"]),
    answer!("UM231", false, true, ["DM106"]),
    answer!("UM232", false, true, ["DM109"]),
];

pub fn semantic_answer(type_id: &str) -> Option<&'static SemanticAnswer> {
    SEMANTIC_ANSWERS.iter().find(|a| a.uplink == type_id)
}

fn first_type_id(message: &CpdlcMessage) -> Option<&str> {
    message.content.first().map(|c| c.type_id.as_str())
}

pub fn message_remains_on_mailbox(message: &CpdlcMessage) -> bool {
    first_type_id(message).is_some_and(|id| NON_AUTOMATIC_CLOSING.contains(&id))
}

pub fn semantic_answer_required(message: &CpdlcMessage) -> bool {
    first_type_id(message).is_some_and(|id| id == "UM143" || semantic_answer(id).is_some())
}

pub fn is_modifiable(message: &CpdlcMessage) -> bool {
    first_type_id(message)
        .and_then(semantic_answer)
        .is_some_and(|a| a.modifiable)
}

pub fn has_negative_response(message: &CpdlcMessage) -> bool {
    let entry = match first_type_id(message).and_then(semantic_answer) {
        Some(a) if a.positive_or_negative => a,
        _ => return false,
    };
    match message.response.as_ref().and_then(|r| r.content.first()) {
        Some(first) => first.type_id != entry.messages[1],
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{downlink, uplink};

    fn message(id: &str) -> CpdlcMessage {
        let mut m = CpdlcMessage::new();
        m.content.push(uplink(id).unwrap().to_element());
        m
    }

    #[test]
    fn every_referenced_message_exists_in_the_catalogue() {
        for id in NON_AUTOMATIC_CLOSING {
            assert!(uplink(id).is_some(), "{id}");
        }
        let missing: Vec<&&str> = REQUEST_MESSAGES.iter().filter(|id| downlink(id).is_none()).collect();
        assert_eq!(missing, vec![&"DM24", &"DM73"]);
        for answer in SEMANTIC_ANSWERS {
            assert!(uplink(answer.uplink).is_some(), "{}", answer.uplink);
            for id in answer.messages {
                assert!(downlink(id).is_some(), "{id}");
            }
        }
    }

    #[test]
    fn mailbox_queries() {
        assert!(message_remains_on_mailbox(&message("UM130")));
        assert!(!message_remains_on_mailbox(&message("UM20")));
        assert!(semantic_answer_required(&message("UM143")));
        assert!(semantic_answer_required(&message("UM148")));
        assert!(!semantic_answer_required(&message("UM20")));
        assert!(is_modifiable(&message("UM132")));
        assert!(!is_modifiable(&message("UM128")));
        assert!(!semantic_answer_required(&CpdlcMessage::new()));
    }

    #[test]
    fn negative_response_detection() {
        let mut m = message("UM148");
        let mut response = CpdlcMessage::new();
        response.content.push(downlink("DM81").unwrap().to_element());
        m.response = Some(Box::new(response));
        assert!(has_negative_response(&m));

        let mut response = CpdlcMessage::new();
        response.content.push(downlink("DM82").unwrap().to_element());
        m.response = Some(Box::new(response));
        assert!(!has_negative_response(&m));
        assert!(!has_negative_response(&message("UM132")));
    }
}
