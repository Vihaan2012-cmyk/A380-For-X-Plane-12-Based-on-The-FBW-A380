use serde::{Deserialize, Serialize};

use crate::catalogue::{DOWNLINK, UPLINK};
use crate::fans::FansMode;
use crate::validation::{self, parse_int_prefix};
use crate::status::AtsuStatusCodes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedResponse {
    NotRequired,
    WilcoUnable,
    AffirmNegative,
    Roger,
    No,
    Yes,
}

impl ExpectedResponse {
    pub fn code(self) -> &'static str {
        match self {
            ExpectedResponse::NotRequired => "NE",
            ExpectedResponse::WilcoUnable => "WU",
            ExpectedResponse::AffirmNegative => "AN",
            ExpectedResponse::Roger => "R",
            ExpectedResponse::No => "N",
            ExpectedResponse::Yes => "Y",
        }
    }

    pub fn from_code(code: &str) -> Option<ExpectedResponse> {
        Some(match code {
            "NE" => ExpectedResponse::NotRequired,
            "WU" => ExpectedResponse::WilcoUnable,
            "AN" => ExpectedResponse::AffirmNegative,
            "R" => ExpectedResponse::Roger,
            "N" => ExpectedResponse::No,
            "Y" => ExpectedResponse::Yes,
            _ => return None,
        })
    }
}

impl Serialize for ExpectedResponse {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for ExpectedResponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        ExpectedResponse::from_code(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown expected response {text}")))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
pub enum ContentType {
    Unknown,
    Level,
    Position,
    Time,
    Direction,
    Distance,
    Speed,
    Frequency,
    Procedure,
    Degree,
    VerticalRate,
    LegType,
    LegTypeDistance,
    LegTypeTime,
    AtcUnit,
    Squawk,
    Altimeter,
    Atis,
    Fuel,
    PersonsOnBoard,
    Freetext,
}

const CONTENT_TYPES: [ContentType; 21] = [
    ContentType::Unknown,
    ContentType::Level,
    ContentType::Position,
    ContentType::Time,
    ContentType::Direction,
    ContentType::Distance,
    ContentType::Speed,
    ContentType::Frequency,
    ContentType::Procedure,
    ContentType::Degree,
    ContentType::VerticalRate,
    ContentType::LegType,
    ContentType::LegTypeDistance,
    ContentType::LegTypeTime,
    ContentType::AtcUnit,
    ContentType::Squawk,
    ContentType::Altimeter,
    ContentType::Atis,
    ContentType::Fuel,
    ContentType::PersonsOnBoard,
    ContentType::Freetext,
];

impl From<u8> for ContentType {
    fn from(v: u8) -> Self {
        CONTENT_TYPES.get(v as usize).copied().unwrap_or(ContentType::Unknown)
    }
}

impl From<ContentType> for u8 {
    fn from(v: ContentType) -> u8 {
        v as u8
    }
}

pub struct ContentDef {
    pub kind: ContentType,
    pub index_start: i32,
    pub index_end: i32,
    pub monitoring: bool,
    pub value: &'static str,
}

pub struct ElementDef {
    pub type_id: &'static str,
    pub templates: &'static [&'static str],
    pub fans: &'static [FansMode],
    pub urgent: bool,
    pub response: ExpectedResponse,
    pub content: &'static [ContentDef],
}

impl ElementDef {
    pub fn to_element(&self) -> MessageElement {
        MessageElement {
            type_id: self.type_id.to_string(),
            fans_modes: self.fans.iter().map(|f| f.code()).collect(),
            urgent: self.urgent,
            content: self
                .content
                .iter()
                .map(|c| MessageContent {
                    kind: c.kind,
                    index_start: c.index_start,
                    index_end: c.index_end,
                    monitoring: c.monitoring,
                    value: c.value.to_string(),
                })
                .collect(),
            expected_response: self.response,
        }
    }

    pub fn supports(&self, fans: FansMode) -> bool {
        self.fans.contains(&fans)
    }
}

pub fn downlink(type_id: &str) -> Option<&'static ElementDef> {
    DOWNLINK.iter().find(|e| e.type_id == type_id)
}

pub fn uplink(type_id: &str) -> Option<&'static ElementDef> {
    UPLINK.iter().find(|e| e.type_id == type_id)
}

pub fn element_def(type_id: &str) -> Option<&'static ElementDef> {
    if type_id.starts_with("DM") {
        downlink(type_id)
    } else {
        uplink(type_id)
    }
}

pub fn downlink_element(type_id: &str) -> Option<MessageElement> {
    downlink(type_id).map(ElementDef::to_element)
}

pub fn uplink_element(type_id: &str) -> Option<MessageElement> {
    uplink(type_id).map(ElementDef::to_element)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MessageContent {
    #[serde(rename = "Type")]
    pub kind: ContentType,
    pub index_start: i32,
    pub index_end: i32,
    pub monitoring: bool,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MessageElement {
    pub type_id: String,
    pub fans_modes: Vec<u8>,
    pub urgent: bool,
    pub content: Vec<MessageContent>,
    pub expected_response: ExpectedResponse,
}

impl MessageElement {
    pub fn is_freetext_element(&self) -> bool {
        self.type_id == "UM169" || self.type_id == "UM183"
    }

    pub fn supports(&self, fans: FansMode) -> bool {
        self.fans_modes.contains(&fans.code())
    }
}

fn in_range(index: i32, len: usize) -> bool {
    index > -1 && (index as usize) < len
}

fn take(value: &mut [String], index: i32) -> String {
    std::mem::replace(&mut value[index as usize], "%s".to_string())
}

fn replace_if(
    content: &mut MessageContent,
    value: &mut Vec<String>,
    ok: impl Fn(&str) -> bool,
) -> bool {
    if in_range(content.index_start, value.len()) && ok(&value[content.index_start as usize]) {
        content.value = take(value, content.index_start);
        return true;
    }
    false
}

fn is_ascii_upper_letter(s: &str) -> bool {
    s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase()
}

fn leg_type_distance(content: &mut MessageContent, value: &mut Vec<String>) -> bool {
    if in_range(content.index_start, value.len()) {
        let word = &value[content.index_start as usize];
        if validation::digits_between(word, 1, 2) {
            let distance: i64 = word.parse().unwrap_or(0);
            if (1..100).contains(&distance) {
                content.value = take(value, content.index_start);
                return true;
            }
        }
    }
    false
}

fn leg_type_time(content: &mut MessageContent, value: &mut Vec<String>) -> bool {
    if content.index_start > -1 && (content.index_start as usize) + 1 < value.len() {
        let i = content.index_start as usize;
        if validation::digits_between(&value[i], 1, 1)
            && matches!(value[i + 1].as_str(), "MIN" | "MINS" | "MINUTES")
        {
            let minutes: i64 = value[i].parse().unwrap_or(0);
            if (1..10).contains(&minutes) {
                content.value = take(value, content.index_start);
                return true;
            }
        }
    }
    false
}

impl MessageContent {
    pub fn new(kind: ContentType, index_start: i32, index_end: i32) -> MessageContent {
        MessageContent {
            kind,
            index_start,
            index_end,
            monitoring: false,
            value: String::new(),
        }
    }

    /// Matches this slot against the words of an uplink; on success stores the
    /// value in the slot and replaces the word by the `%s` placeholder.
    pub fn validate_and_replace(&mut self, value: &mut Vec<String>, fans: FansMode) -> bool {
        use validation as v;
        let ok = AtsuStatusCodes::Ok;
        match self.kind {
            ContentType::Level => {
                if replace_if(self, value, |w| v::validate_scratchpad_altitude(w, fans) == ok) {
                    return true;
                }
                if in_range(self.index_end, value.len())
                    && v::validate_scratchpad_altitude(&value[self.index_end as usize], fans) == ok
                {
                    self.value = take(value, self.index_end);
                    return true;
                }
                false
            }
            ContentType::Position => replace_if(self, value, |w| {
                v::validate_scratchpad_waypoint(w) == ok
                    && v::validate_scratchpad_time(w, true) != ok
                    && v::validate_scratchpad_time(w, false) != ok
            }),
            ContentType::Time => {
                if !in_range(self.index_start, value.len()) {
                    return false;
                }
                let word = value[self.index_start as usize].clone();
                if v::validate_scratchpad_time(&word, true) == ok {
                    self.value = take(value, self.index_start);
                    true
                } else if v::validate_scratchpad_time(&word, false) == ok {
                    take(value, self.index_start);
                    self.value = format!("{word}Z");
                    true
                } else {
                    false
                }
            }
            ContentType::Direction => replace_if(self, value, |w| w == "LEFT" || w == "RIGHT"),
            ContentType::Distance => replace_if(self, value, |w| v::validate_scratchpad_distance(w) == ok),
            ContentType::Speed => replace_if(self, value, |w| v::validate_scratchpad_speed(w, fans) == ok),
            ContentType::Frequency => replace_if(self, value, |w| v::validate_vhf_frequency(w) == ok),
            ContentType::Procedure => replace_if(self, value, |w| v::validate_scratchpad_procedure(w) == ok),
            ContentType::Degree => replace_if(self, value, |w| v::validate_scratchpad_degree(w) == ok),
            ContentType::VerticalRate => {
                if self.index_start > -1 && (self.index_start as usize) + 3 < value.len() {
                    let i = self.index_start as usize;
                    if value[i + 1] == "FEET" && value[i + 2] == "PER" && value[i + 3] == "MINUTE" {
                        self.value = format!("{} FEET PER MINUTE", value[i]);
                        value[i] = "%s".to_string();
                        return true;
                    }
                }
                false
            }
            ContentType::AtcUnit => {
                if !in_range(self.index_start, value.len()) {
                    return false;
                }
                let i = self.index_start as usize;
                if i + 1 < value.len() && value[i + 1] == "CTR" {
                    self.value = format!("{} {}", value[i], value[i + 1]);
                    value.remove(i + 1);
                } else {
                    self.value = value[i].clone();
                }
                value[i] = "%s".to_string();
                true
            }
            ContentType::Squawk => replace_if(self, value, |w| {
                validation::digits_between(w, 4, 4) && w.parse::<i64>().map(|n| (0..7777).contains(&n)).unwrap_or(false)
            }),
            ContentType::Freetext => {
                if !in_range(self.index_start, value.len()) {
                    return false;
                }
                let start = self.index_start as usize;
                let end = if self.index_end == -1 {
                    value.len()
                } else {
                    ((self.index_end as usize) + 1).min(value.len())
                };
                self.value = if start < end { value[start..end].join(" ") } else { String::new() };
                value.truncate(start);
                value.push("%s".to_string());
                true
            }
            ContentType::LegTypeDistance => leg_type_distance(self, value),
            ContentType::LegTypeTime => leg_type_time(self, value),
            ContentType::LegType => {
                let mut time = MessageContent::new(ContentType::LegTypeTime, self.index_start, self.index_end);
                if leg_type_time(&mut time, value) {
                    return true;
                }
                let mut distance = MessageContent::new(ContentType::LegTypeDistance, self.index_start, self.index_end);
                leg_type_distance(&mut distance, value)
            }
            ContentType::Altimeter => {
                if self.index_start >= 1 && in_range(self.index_start, value.len()) {
                    let i = self.index_start as usize;
                    let prev = value[i - 1].as_str();
                    let word = value[i].as_str();
                    let matched = if prev == "ALTIMETER" {
                        word.len() == 5
                            && word.as_bytes()[2] == b'.'
                            && validation::digits_between(&word[..2], 2, 2)
                            && validation::digits_between(&word[3..], 2, 2)
                    } else if prev == "QNH" {
                        validation::digits_between(word, 3, 4)
                    } else {
                        false
                    };
                    if matched {
                        self.value = take(value, self.index_start);
                        return true;
                    }
                }
                false
            }
            ContentType::Atis => replace_if(self, value, is_ascii_upper_letter),
            ContentType::Fuel => replace_if(self, value, |w| validation::digits_between(w, 1, 6)),
            ContentType::PersonsOnBoard => replace_if(self, value, |w| validation::digits_between(w, 1, 3)),
            ContentType::Unknown => false,
        }
    }
}

pub fn parse_transmission_id(text: &str) -> i64 {
    parse_int_prefix(text).unwrap_or(-1)
}
