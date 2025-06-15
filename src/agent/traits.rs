pub struct CallToolArgs {
    pub tool_name: String,
    pub tool_args: serde_json::Value,
    pub _app_handle: Option<AppHandle>,
    pub _message_id: Option<String>,
}
