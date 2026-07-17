use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpRequest {
    pub request_id: u64,
    pub method: String,
    pub path: String,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpResponseHeader {
    pub request_id: u64,
    pub status: u16,
    pub headers: Vec<HttpHeader>,
}

impl HttpRequest {
    pub fn header_pairs(&self) -> Vec<(String, String)> {
        self.headers
            .iter()
            .map(|h| (h.name.clone(), h.value.clone()))
            .collect()
    }
}

impl HttpResponseHeader {
    pub fn header_pairs(&self) -> Vec<(String, String)> {
        self.headers
            .iter()
            .map(|h| (h.name.clone(), h.value.clone()))
            .collect()
    }
}
