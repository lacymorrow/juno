use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ErrorToastAction {
    pub label: String,
    pub command: String,
    pub args: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ErrorToastPayload {
    pub message: String,
    pub action: Option<ErrorToastAction>,
}
