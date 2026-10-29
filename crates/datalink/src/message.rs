use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::convert::{timestamp_to_string, word_wrap};
use crate::elements::{downlink, uplink, MessageElement};
use crate::status::Clock;
use crate::weather::{AtisMessage, MetarMessage, TafMessage};

macro_rules! numeric_enum {
    ($name:ident { $($variant:ident = $value:expr),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(from = "u8", into = "u8")]
        pub enum $name {
            $($variant = $value),+
        }

        impl From<u8> for $name {
            fn from(v: u8) -> Self {
                match v {
                    $($value => $name::$variant,)+
                    _ => numeric_enum!(@first $name, $($variant),+),
                }
            }
        }

        impl From<$name> for u8 {
            fn from(v: $name) -> u8 {
                v as u8
            }
        }
    };
    (@first $name:ident, $first:ident $(, $rest:ident)*) => { $name::$first };
}

numeric_enum!(MessageNetwork { Hoppie = 0, Fbw = 1 });
numeric_enum!(MessageDirection { Uplink = 0, Downlink = 1 });
numeric_enum!(MessageType {
    Freetext = 0,
    Metar = 1,
    Taf = 2,
    Atis = 3,
    Aoc = 4,
    Cpdlc = 5,
    Dcl = 6,
    Ocl = 7,
    Atc = 8,
});
numeric_enum!(ComStatus { Open = 0, Sending = 1, Sent = 2, Received = 3, Failed = 4 });
numeric_enum!(MonitoringState { Ignored = 0, Required = 1, Monitoring = 2, Cancelled = 3, Finished = 4 });

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerializationFormat {
    FmsDisplay,
    FmsDisplayMonitored,
    Mailbox,
    Printer,
    Network,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Timestamp {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub seconds: f64,
}

impl Timestamp {
    pub fn mailbox_timestamp(&self) -> String {
        format!("{}Z", timestamp_to_string(self.seconds))
    }

    pub fn fms_timestamp(&self) -> String {
        timestamp_to_string(self.seconds)
    }

    pub fn from_clock(clock: &Clock) -> Timestamp {
        Timestamp {
            year: clock.year,
            month: clock.month,
            day: clock.day_of_month,
            seconds: clock.seconds_of_day,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    #[serde(rename = "Network")]
    pub network: MessageNetwork,
    #[serde(rename = "UniqueMessageID")]
    pub unique_message_id: i64,
    #[serde(rename = "Timestamp")]
    pub timestamp: Timestamp,
    #[serde(rename = "Station")]
    pub station: String,
    #[serde(rename = "ComStatus")]
    pub com_status: ComStatus,
    #[serde(rename = "Type")]
    pub kind: MessageType,
    #[serde(rename = "Direction")]
    pub direction: MessageDirection,
    #[serde(rename = "Confirmed")]
    pub confirmed: bool,
    #[serde(rename = "Message")]
    pub message: String,
}

impl Header {
    pub fn new(kind: MessageType, direction: MessageDirection) -> Header {
        Header {
            network: MessageNetwork::Hoppie,
            unique_message_id: -1,
            timestamp: Timestamp::default(),
            station: String::new(),
            com_status: ComStatus::Open,
            kind,
            direction,
            confirmed: false,
            message: String::new(),
        }
    }
}

const SEPARATOR_FMS: &str = "{white}------------------------{end}\n";
const SEPARATOR_PLAIN: &str = "------------------------\n";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FreetextMessage {
    #[serde(flatten)]
    pub header: Header,
}

impl FreetextMessage {
    pub fn new() -> FreetextMessage {
        FreetextMessage {
            header: Header::new(MessageType::Freetext, MessageDirection::Downlink),
        }
    }

    pub fn render(&self, format: SerializationFormat) -> String {
        match format {
            SerializationFormat::FmsDisplay | SerializationFormat::FmsDisplayMonitored => {
                let mut message = String::new();
                for line in word_wrap(&self.header.message, 25) {
                    message.push_str(&format!("{{green}}{line}{{end}}\n"));
                }
                message.push_str(SEPARATOR_FMS);
                message
            }
            _ => self.header.message.clone(),
        }
    }
}

impl Default for FreetextMessage {
    fn default() -> Self {
        FreetextMessage::new()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CpdlcMessage {
    #[serde(flatten)]
    pub header: Header,
    #[serde(rename = "Content")]
    pub content: Vec<MessageElement>,
    #[serde(rename = "Response", default)]
    pub response: Option<Box<CpdlcMessage>>,
    #[serde(rename = "CurrentTransmissionId")]
    pub current_transmission_id: i64,
    #[serde(rename = "PreviousTransmissionId")]
    pub previous_transmission_id: i64,
    #[serde(rename = "MailboxRelevantMessage")]
    pub mailbox_relevant_message: bool,
    #[serde(rename = "CloseAutomatically")]
    pub close_automatically: bool,
    #[serde(rename = "MessageMonitoring")]
    pub message_monitoring: MonitoringState,
    #[serde(rename = "SemanticResponseRequired")]
    pub semantic_response_required: bool,
    #[serde(rename = "ReminderTimestamp", default)]
    pub reminder_timestamp: Option<Timestamp>,
}

fn splice_placeholder(content: &str, replacement: &str) -> String {
    match content.find("%s") {
        Some(idx) => format!("{}{}{}", &content[..idx], replacement, &content[idx + 2..]),
        None => {
            let mut chars = content.chars();
            chars.next();
            format!("{}{}", replacement, chars.as_str())
        }
    }
}

impl CpdlcMessage {
    pub fn new() -> CpdlcMessage {
        CpdlcMessage::with_kind(MessageType::Cpdlc)
    }

    pub(crate) fn with_kind(kind: MessageType) -> CpdlcMessage {
        CpdlcMessage {
            header: Header::new(kind, MessageDirection::Downlink),
            content: Vec::new(),
            response: None,
            current_transmission_id: -1,
            previous_transmission_id: -1,
            mailbox_relevant_message: true,
            close_automatically: true,
            message_monitoring: MonitoringState::Ignored,
            semantic_response_required: false,
            reminder_timestamp: None,
        }
    }

    fn serialize_content(&self, format: SerializationFormat, template: &str, element: &MessageElement) -> String {
        let mut content = template.to_string();
        for entry in &element.content {
            if format == SerializationFormat::Network {
                content = splice_placeholder(&content, &entry.value);
            } else if !entry.value.is_empty() {
                if element.is_freetext_element() {
                    content = entry.value.clone();
                } else if self.message_monitoring == MonitoringState::Monitoring
                    && format == SerializationFormat::FmsDisplayMonitored
                {
                    content = splice_placeholder(&content, &format!("{{magenta}}{}{{end}}", entry.value));
                } else {
                    content = splice_placeholder(&content, &format!("@{}@", entry.value));
                }
            } else {
                content = splice_placeholder(&content, "[      ]");
            }
        }
        content
    }

    fn extend_serialization_with_response(&self) -> bool {
        let response = match &self.response {
            Some(r) if !r.content.is_empty() => r,
            _ => return false,
        };
        !matches!(
            response.content[0].type_id.as_str(),
            "DM0" | "DM1" | "DM2" | "DM3" | "DM4" | "DM5" | "UM0" | "UM1" | "UM3" | "UM4" | "UM5"
        )
    }

    fn template_lines(&self, line_length: usize, format: SerializationFormat) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.content.is_empty() {
            for element in &self.content {
                let def = if self.header.direction == MessageDirection::Downlink {
                    downlink(&element.type_id)
                } else {
                    uplink(&element.type_id)
                };
                let template = def.and_then(|d| d.templates.first().copied()).unwrap_or("");
                lines.extend(word_wrap(&self.serialize_content(format, template, element), line_length));
            }
        } else {
            for entry in self.header.message.split('_') {
                lines.extend(word_wrap(entry, line_length));
            }
        }
        lines
    }

    pub fn render(&self, format: SerializationFormat) -> String {
        let line_length = if format == SerializationFormat::Mailbox { 30 } else { 25 };
        let lines = self.template_lines(line_length, format);
        let direction_word = if self.header.direction == MessageDirection::Uplink { "FROM" } else { "TO" };

        match format {
            SerializationFormat::Network => {
                let previous = if self.previous_transmission_id != -1 {
                    self.previous_transmission_id.to_string()
                } else {
                    String::new()
                };
                let expected = self
                    .content
                    .first()
                    .map(|c| c.expected_response.code())
                    .unwrap_or("undefined");
                format!(
                    "/data2/{}/{}/{}/{}",
                    self.current_transmission_id,
                    previous,
                    expected,
                    lines.join(" ")
                )
            }
            SerializationFormat::Mailbox => lines.join("\n"),
            SerializationFormat::FmsDisplay | SerializationFormat::FmsDisplayMonitored => {
                let mut message = format!(
                    "{{cyan}}{} {} {}{{end}}\n",
                    self.header.timestamp.mailbox_timestamp(),
                    direction_word,
                    self.header.station
                );
                for line in lines {
                    let line = line.replace('@', "");
                    if format == SerializationFormat::FmsDisplayMonitored {
                        message.push_str(&line);
                    } else {
                        message.push_str(&format!("{{green}}{line}{{end}}\n"));
                    }
                }
                message.push_str(SEPARATOR_FMS);
                if self.extend_serialization_with_response() {
                    message.push_str(&self.response.as_ref().unwrap().render(format));
                }
                message
            }
            SerializationFormat::Printer => {
                let mut message = format!(
                    "{} {} {}}}\n",
                    self.header.timestamp.mailbox_timestamp(),
                    direction_word,
                    self.header.station
                );
                for line in lines {
                    message.push_str(&line.replace('@', ""));
                    message.push('\n');
                }
                message.push_str(SEPARATOR_PLAIN);
                if self.extend_serialization_with_response() {
                    message.push_str(&self.response.as_ref().unwrap().render(format));
                }
                message
            }
        }
    }
}

impl Default for CpdlcMessage {
    fn default() -> Self {
        CpdlcMessage::new()
    }
}

fn drop_blank_lines(lines: &[String]) -> String {
    lines
        .join("\n")
        .split_inclusive('\n')
        .filter(|segment| match segment.strip_suffix('\n') {
            Some(body) => !body.chars().all(char::is_whitespace),
            None => true,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DclMessage {
    #[serde(flatten)]
    pub cpdlc: CpdlcMessage,
    #[serde(rename = "Callsign")]
    pub callsign: String,
    #[serde(rename = "Origin")]
    pub origin: String,
    #[serde(rename = "Destination")]
    pub destination: String,
    #[serde(rename = "AcType")]
    pub ac_type: String,
    #[serde(rename = "Atis")]
    pub atis: String,
    #[serde(rename = "Gate")]
    pub gate: String,
    #[serde(rename = "Freetext")]
    pub freetext: Vec<String>,
}

impl DclMessage {
    pub fn new() -> DclMessage {
        let mut cpdlc = CpdlcMessage::with_kind(MessageType::Dcl);
        cpdlc.close_automatically = false;
        DclMessage {
            cpdlc,
            callsign: String::new(),
            origin: String::new(),
            destination: String::new(),
            ac_type: String::new(),
            atis: String::new(),
            gate: String::new(),
            freetext: Vec::new(),
        }
    }

    pub fn render(&self, format: SerializationFormat) -> String {
        let mut out = String::new();
        if format == SerializationFormat::Network {
            out.push_str("REQUEST PREDEP CLEARANCE \n");
            out.push_str(&format!("{} {} TO {} \n", self.callsign, self.ac_type, self.destination));
            let stand = if self.gate.is_empty() { String::new() } else { format!(" STAND {}", self.gate) };
            out.push_str(&format!("AT {}{} \n", self.origin, stand));
            out.push_str(&format!("ATIS {}", self.atis));
        } else {
            if format != SerializationFormat::Mailbox {
                out.push_str(&format!(
                    "{} TO {}\n",
                    self.cpdlc.header.timestamp.mailbox_timestamp(),
                    self.cpdlc.header.station
                ));
            }
            out.push_str(&format!("DEPART REQUEST\n{}\n", self.callsign));
            let gate = if self.gate.is_empty() { String::new() } else { format!(" GATE:{}", self.gate) };
            out.push_str(&format!("FROM:{}{}\n", self.origin, gate));
            out.push_str(&format!("TO:{} ATIS:{}\n", self.destination, self.atis));
            out.push_str(&format!("A/C TYPE:{}", self.ac_type));
            let freetext = drop_blank_lines(&self.freetext);
            if !freetext.is_empty() {
                out.push('\n');
                out.push_str(&freetext);
            }
        }
        out
    }
}

impl Default for DclMessage {
    fn default() -> Self {
        DclMessage::new()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OclMessage {
    #[serde(flatten)]
    pub cpdlc: CpdlcMessage,
    #[serde(rename = "Callsign")]
    pub callsign: String,
    #[serde(rename = "Destination")]
    pub destination: String,
    #[serde(rename = "EntryPoint")]
    pub entry_point: String,
    #[serde(rename = "EntryTime")]
    pub entry_time: String,
    #[serde(rename = "RequestedMach")]
    pub requested_mach: String,
    #[serde(rename = "RequestedFlightlevel")]
    pub requested_flightlevel: String,
    #[serde(rename = "Freetext")]
    pub freetext: Vec<String>,
}

impl OclMessage {
    pub fn new() -> OclMessage {
        let mut cpdlc = CpdlcMessage::with_kind(MessageType::Ocl);
        cpdlc.close_automatically = false;
        OclMessage {
            cpdlc,
            callsign: String::new(),
            destination: String::new(),
            entry_point: String::new(),
            entry_time: String::new(),
            requested_mach: String::new(),
            requested_flightlevel: String::new(),
            freetext: Vec::new(),
        }
    }

    pub fn render(&self, format: SerializationFormat) -> String {
        let mut out = format!("OCEANIC REQUEST\n{} \n", self.callsign);
        out.push_str(&format!("ENTRY POINT:{}\nAT:{} \n", self.entry_point, self.entry_time));
        out.push_str(&format!("REQ:{} {}", self.requested_mach, self.requested_flightlevel));

        let freetext = drop_blank_lines(&self.freetext);
        if !freetext.is_empty() {
            out.push('\n');
            out.push_str(&freetext);
        }

        if format == SerializationFormat::Network {
            out = format!("/data2/{}//N/{}", self.cpdlc.current_transmission_id, out);
        } else if format != SerializationFormat::Mailbox {
            out = format!(
                "{} TO {}\n{}",
                self.cpdlc.header.timestamp.mailbox_timestamp(),
                self.cpdlc.header.station,
                out
            );
        }
        out
    }
}

impl Default for OclMessage {
    fn default() -> Self {
        OclMessage::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnyMessage {
    Freetext(FreetextMessage),
    Metar(MetarMessage),
    Taf(TafMessage),
    Atis(AtisMessage),
    Cpdlc(CpdlcMessage),
    Dcl(DclMessage),
    Ocl(OclMessage),
}

impl AnyMessage {
    pub fn header(&self) -> &Header {
        match self {
            AnyMessage::Freetext(m) => &m.header,
            AnyMessage::Metar(m) => &m.weather.header,
            AnyMessage::Taf(m) => &m.weather.header,
            AnyMessage::Atis(m) => &m.weather.header,
            AnyMessage::Cpdlc(m) => &m.header,
            AnyMessage::Dcl(m) => &m.cpdlc.header,
            AnyMessage::Ocl(m) => &m.cpdlc.header,
        }
    }

    pub fn header_mut(&mut self) -> &mut Header {
        match self {
            AnyMessage::Freetext(m) => &mut m.header,
            AnyMessage::Metar(m) => &mut m.weather.header,
            AnyMessage::Taf(m) => &mut m.weather.header,
            AnyMessage::Atis(m) => &mut m.weather.header,
            AnyMessage::Cpdlc(m) => &mut m.header,
            AnyMessage::Dcl(m) => &mut m.cpdlc.header,
            AnyMessage::Ocl(m) => &mut m.cpdlc.header,
        }
    }

    /// Port of Conversion.messageDataToMessage; unknown types give None.
    pub fn from_json(data: &Value) -> Option<AnyMessage> {
        let kind = data.get("Type")?.as_u64()? as u8;
        Some(match MessageType::from(kind) {
            MessageType::Atis => AnyMessage::Atis(serde_json::from_value(data.clone()).ok()?),
            MessageType::Cpdlc => AnyMessage::Cpdlc(serde_json::from_value(data.clone()).ok()?),
            MessageType::Dcl => AnyMessage::Dcl(serde_json::from_value(data.clone()).ok()?),
            MessageType::Freetext => AnyMessage::Freetext(serde_json::from_value(data.clone()).ok()?),
            MessageType::Metar => AnyMessage::Metar(serde_json::from_value(data.clone()).ok()?),
            MessageType::Ocl => AnyMessage::Ocl(serde_json::from_value(data.clone()).ok()?),
            MessageType::Taf => AnyMessage::Taf(serde_json::from_value(data.clone()).ok()?),
            _ => return None,
        })
    }

    pub fn to_json(&self) -> Value {
        match self {
            AnyMessage::Freetext(m) => serde_json::to_value(m),
            AnyMessage::Metar(m) => serde_json::to_value(m),
            AnyMessage::Taf(m) => serde_json::to_value(m),
            AnyMessage::Atis(m) => serde_json::to_value(m),
            AnyMessage::Cpdlc(m) => serde_json::to_value(m),
            AnyMessage::Dcl(m) => serde_json::to_value(m),
            AnyMessage::Ocl(m) => serde_json::to_value(m),
        }
        .unwrap_or(Value::Null)
    }

    pub fn render(&self, format: SerializationFormat) -> String {
        match self {
            AnyMessage::Freetext(m) => m.render(format),
            AnyMessage::Metar(m) => m.weather.render(format),
            AnyMessage::Taf(m) => m.weather.render(format),
            AnyMessage::Atis(m) => m.weather.render(format),
            AnyMessage::Cpdlc(m) => m.render(format),
            AnyMessage::Dcl(m) => m.render(format),
            AnyMessage::Ocl(m) => m.render(format),
        }
    }
}
