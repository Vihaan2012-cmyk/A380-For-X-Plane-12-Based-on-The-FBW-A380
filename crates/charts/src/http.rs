use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn get(url: &str) -> HttpRequest {
        HttpRequest { method: "GET".into(), url: url.into(), headers: Vec::new(), body: Vec::new() }
    }

    pub fn post_form(url: &str, fields: &[(&str, &str)]) -> HttpRequest {
        let body = crate::codec::form_encode(fields);
        HttpRequest {
            method: "POST".into(),
            url: url.into(),
            headers: vec![("Content-Type".into(), "application/x-www-form-urlencoded".into())],
            body: body.into_bytes(),
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> HttpRequest {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn bearer(self, token: &str) -> HttpRequest {
        self.header("Authorization", &format!("Bearer {token}"))
    }

    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

pub trait HttpClient: Send + Sync {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String>;
}

struct Scripted {
    method: String,
    url_prefix: String,
    reply: HttpResponse,
}

/// Replies in script order to the first entry whose method and URL prefix match; records what was sent.
#[derive(Default)]
pub struct ScriptedHttp {
    script: Mutex<VecDeque<Scripted>>,
    sent: Mutex<Vec<HttpRequest>>,
}

impl ScriptedHttp {
    pub fn new() -> ScriptedHttp {
        ScriptedHttp::default()
    }

    pub fn reply(self, method: &str, url_prefix: &str, status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> ScriptedHttp {
        self.script.lock().unwrap().push_back(Scripted {
            method: method.into(),
            url_prefix: url_prefix.into(),
            reply: HttpResponse { status, content_type: content_type.into(), body: body.into() },
        });
        self
    }

    pub fn json(self, method: &str, url_prefix: &str, status: u16, body: &str) -> ScriptedHttp {
        self.reply(method, url_prefix, status, "application/json", body.as_bytes().to_vec())
    }

    pub fn sent(&self) -> Vec<HttpRequest> {
        self.sent.lock().unwrap().clone()
    }

    pub fn remaining(&self) -> usize {
        self.script.lock().unwrap().len()
    }
}

impl HttpClient for ScriptedHttp {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        self.sent.lock().unwrap().push(request.clone());
        let mut script = self.script.lock().unwrap();
        let at = script
            .iter()
            .position(|s| s.method.eq_ignore_ascii_case(&request.method) && request.url.starts_with(&s.url_prefix))
            .ok_or_else(|| format!("no scripted reply for {} {}", request.method, request.url))?;
        Ok(script.remove(at).unwrap().reply)
    }
}

impl<T: HttpClient + ?Sized> HttpClient for std::sync::Arc<T> {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        (**self).send(request)
    }
}
