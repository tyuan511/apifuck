use serde_json::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    ClaudeCode,
    Codex,
    OpenCode,
    Pi,
}
impl Agent {
    pub const ALL: [Self; 4] = [Self::ClaudeCode, Self::Codex, Self::OpenCode, Self::Pi];
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::OpenCode => "OpenCode",
            Self::Pi => "Pi",
        }
    }
    pub fn instruction(self) -> &'static str {
        match self {
            Self::ClaudeCode => "在终端执行命令，然后在 Claude Code 中运行 /mcp 检查连接。",
            Self::Codex => "合并到 ~/.codex/config.toml，然后重新打开 Codex 会话。",
            Self::OpenCode => "合并到项目 opencode.json，然后运行 opencode mcp list 检查连接。",
            Self::Pi => {
                "使用内置 MCP 的 Pi，合并到 ~/.pi/agent/mcp.json；运行 pi mcp list，已有会话执行 /reload 后用 /mcp 检查。"
            }
        }
    }
    pub fn docs(self) -> &'static str {
        match self {
            Self::ClaudeCode => "https://code.claude.com/docs/en/mcp",
            Self::Codex => "https://developers.openai.com/codex/mcp",
            Self::OpenCode => "https://opencode.ai/docs/mcp-servers/",
            Self::Pi => {
                "https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md"
            }
        }
    }
    pub fn config(self, endpoint: &str, token: &str) -> String {
        let headers = json!({"Authorization":format!("Bearer {token}")});
        match self {
            Self::ClaudeCode=>format!("claude mcp add --transport http --scope user apifuck {} --header {}", shell_words::quote(endpoint), shell_words::quote(&format!("Authorization: Bearer {token}"))),
            Self::Codex=>format!("[mcp_servers.apifuck]\nurl = {}\nhttp_headers = {{ Authorization = {} }}\ntool_timeout_sec = 120",json!(endpoint),json!(format!("Bearer {token}"))),
            Self::OpenCode=>serde_json::to_string_pretty(&json!({"$schema":"https://opencode.ai/config.json","mcp":{"apifuck":{"type":"remote","url":endpoint,"headers":headers,"enabled":true,"oauth":false}}})).unwrap(),
            Self::Pi=>serde_json::to_string_pretty(&json!({"mcpServers":{"apifuck":{"url":endpoint,"headers":headers,"timeout":120}}})).unwrap(),
        }
    }
}
