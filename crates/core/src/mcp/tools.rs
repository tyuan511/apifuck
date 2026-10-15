use serde_json::{Value, json};
use std::sync::OnceLock;

pub fn list() -> &'static [Value] {
    static TOOLS: OnceLock<Vec<Value>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        let text = || json!({"type":"string","minLength":1});
        let selector = || json!({"selector":text()});
        let parent_name = || json!({"parent":{"type":"string","default":""},"name":text()});
        let request = || json!({"selector":text(),"environment":{"type":["string","null"],"description":"Omit to use the desktop selection; null means no environment."}});
        let mut tools = vec![];
        let mut add = |name: &str, description: &str, properties: Value, required: &[&str], read_only: bool| {
            tools.push(json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":read_only,"destructiveHint":!read_only,"openWorldHint":!read_only}}));
        };
        add("get_workspace", "Read the currently open native desktop project, documents and selected environment. Only saved YAML is exposed.", json!({}), &[], true);
        add("list_projects", "Read recently opened projects.", json!({}), &[], true);
        add("open_project", "Switch the desktop to an existing OpenCollection project. Requires no unsaved drafts or active requests.", json!({"path":text()}), &["path"], false);
        add("create_project", "Create an OpenCollection project at an empty directory and open it.", json!({"path":text(),"name":text()}), &["path","name"], false);
        add("read_document", "Read full YAML and parsed data for a request, folder, environment or root collection. Use its source as expectedSource when editing.", selector(), &["selector"], true);
        add("create_request", "Create a named HTTP, GraphQL or WebSocket request with generated file identity.", {let mut p=parent_name();p["protocol"]=json!({"enum":["http","graphql","websocket"]});p}, &["name","protocol"], false);
        add("create_folder", "Create a folder with generated file identity.", parent_name(), &["name"], false);
        add("create_environment", "Create an environment without activating it.", json!({"name":text()}), &["name"], false);
        add("update_document", "Replace parsed document data, preserving comments outside edited YAML nodes. Covers names, URLs, auth, bodies, parameters, variables, scripts, tests and docs. Read first, preserve unchanged fields, and send the exact previous source in expectedSource. Does not rename files.", json!({"selector":text(),"expectedSource":{"type":"string"},"value":{"type":"object"}}), &["selector","expectedSource","value"], false);
        add("save_document_source", "Save validated YAML source, preserving its formatting and rejecting concurrent edits. Read first for expectedSource.", json!({"selector":text(),"expectedSource":{"type":"string"},"source":text()}), &["selector","expectedSource","source"], false);
        add("duplicate_request", "Copy a saved request to a new generated path, with a new display name.", {let mut p=parent_name();p["selector"]=text();p}, &["selector","name"], false);
        add("move_document", "Explicitly rename/move a request, folder or environment file inside the project. For folders supply the new folder.yml path. Read first for expectedSource.", json!({"selector":text(),"target":text(),"expectedSource":{"type":"string"}}), &["selector","target","expectedSource"], false);
        add("trash_document", "Move a request, environment or folder subtree into .apifuck/trash. Cannot delete the root collection. Read first for expectedSource.", json!({"selector":text(),"expectedSource":{"type":"string"}}), &["selector","expectedSource"], false);
        add("reorder_document", "Place a document before a sibling by updating core sequence values.", json!({"selector":text(),"before":text()}), &["selector","before"], false);
        add("set_environment", "Select the active desktop environment by name, or clear it with null.", json!({"environment":{"type":["string","null"]}}), &["environment"], false);
        add("preview_variables", "Resolve static inherited variables without executing scripts.", request(), &["selector"], true);
        add("send_request", "Execute a saved HTTP/GraphQL/SSE request with core inheritance, auth, scripts, tests and local environment writes. Returns a job ID; poll get_job.", request(), &["selector"], false);
        add("get_job", "Read an HTTP, collection runner or WebSocket job, including result/error and latest 100 events. Completed jobs are retained until evicted (32 total).", json!({"id":text()}), &["id"], true);
        add("cancel_job", "Cancel an active request/runner or close a WebSocket.", json!({"id":text()}), &["id"], false);
        add("connect_websocket", "Start a saved WebSocket through the core engine. Returns a job ID; get_job reports connection events and messages.", request(), &["selector"], false);
        add("send_websocket_message", "Send text on a connected MCP WebSocket job.", json!({"id":text(),"message":{"type":"string","maxLength":1048576}}), &["id","message"], false);
        add("export_request", "Export saved HTTP/GraphQL as code using static core resolution, without executing scripts.", {let mut p=request();p["language"]=json!({"enum":["curl","javascript","python","go"]});p}, &["selector","language"], true);
        add("import_curl", "Parse cURL into a new request. Never executes the shell command.", {let mut p=parent_name();p["command"]=text();p}, &["name","command"], false);
        add("convert_bru", "Convert a Bru/OpenCollection request, environment or folder in memory without writing files.", json!({"source":text(),"kind":{"enum":["request","folder","environment"]},"direction":{"enum":["import","export"]}}), &["source","kind","direction"], true);
        add("import_document", "Add validated OpenCollection YAML at a new relative path. Use convert_bru first for Bru input. Cannot overwrite existing files or internal app state.", json!({"path":text(),"source":text()}), &["path","source"], false);
        add("run_collection", "Run saved HTTP/GraphQL requests through the core runner; returns a job ID. WebSockets are skipped.", json!({"environment":{"type":["string","null"]},"folder":{"type":"string"},"iterations":{"type":"integer","minimum":1,"maximum":10000},"delayMs":{"type":"integer","minimum":0,"maximum":60000},"stopOnFailure":{"type":"boolean"}}), &[], false);
        tools
    })
}

pub fn validate(name: &str, arguments: &Value) -> anyhow::Result<()> {
    let tool = list()
        .iter()
        .find(|t| t["name"] == name)
        .ok_or_else(|| anyhow::anyhow!("Unknown MCP tool: {name}"))?;
    jsonschema::validator_for(&tool["inputSchema"])?
        .validate(arguments)
        .map_err(|e| anyhow::anyhow!("{e}"))
}
pub fn mutates_workspace(name: &str) -> bool {
    matches!(
        name,
        "open_project"
            | "create_project"
            | "create_request"
            | "create_folder"
            | "create_environment"
            | "update_document"
            | "save_document_source"
            | "duplicate_request"
            | "move_document"
            | "trash_document"
            | "reorder_document"
            | "set_environment"
            | "import_curl"
            | "import_document"
    )
}
