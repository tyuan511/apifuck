use anyhow::{Context as _, Result, anyhow, ensure};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}
#[derive(Default, Clone)]
pub struct ScriptReport {
    pub logs: Vec<String>,
    pub tests: Vec<TestResult>,
    pub variables: Value,
}

fn runtime(deadline: Instant) -> Result<rquickjs::Runtime> {
    let runtime = rquickjs::Runtime::new()?;
    runtime.set_memory_limit(32 * 1024 * 1024);
    runtime.set_max_stack_size(4 * 1024 * 1024);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    Ok(runtime)
}

/// Cache only trusted library bytecode, never a VM, context or user script.
/// Compile with the running QuickJS version so there is no persisted bytecode
/// compatibility boundary. Failed initialization can be retried by a later hook.
fn runtime_bytecode(deadline: Instant) -> Result<Arc<[u8]>> {
    static BYTECODE: Mutex<Option<Arc<[u8]>>> = Mutex::new(None);
    let mut cached = BYTECODE
        .lock()
        .map_err(|_| anyhow!("脚本运行库缓存不可用"))?;
    if let Some(bytes) = cached.as_ref() {
        return Ok(bytes.clone());
    }
    let runtime = runtime(deadline)?;
    let context = rquickjs::Context::full(&runtime)?;
    let bytes: Arc<[u8]> = context
        .with(|ctx| {
            rquickjs::Module::declare(
                ctx,
                "apifuck:script-runtime",
                include_str!("../resources/javascript/runtime.bundle.js"),
            )?
            .write(Default::default())
        })?
        .into();
    *cached = Some(bytes.clone());
    Ok(bytes)
}

/// Each hook has a fresh lexical scope; env/vars mutations cross hook boundaries.
pub fn run(
    code: &str,
    request: &mut Value,
    response: Option<&mut Value>,
    env: &mut Value,
    vars: &mut Value,
    mutations: &mut Value,
    report: &mut ScriptReport,
) -> Result<()> {
    if code.trim().is_empty() {
        return Ok(());
    }
    // AJV code generation needs more native stack than Tokio's worker default.
    // Build the VM on a bounded dedicated stack; no QuickJS value crosses threads.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("apifuck-script".into())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, move || {
                run_inner(code, request, response, env, vars, mutations, report)
            })
            .context("无法创建脚本线程")?
            .join()
            .map_err(|_| anyhow!("脚本线程异常退出"))?
    })
}
fn run_inner(
    code: &str,
    request: &mut Value,
    response: Option<&mut Value>,
    env: &mut Value,
    vars: &mut Value,
    mutations: &mut Value,
    report: &mut ScriptReport,
) -> Result<()> {
    if code.trim().is_empty() {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let bytecode = runtime_bytecode(deadline)?;
    let runtime = runtime(deadline)?;
    let context = rquickjs::Context::full(&runtime)?;
    let seed = json!({"request":request,"response":response.as_deref(),"env":env,"vars":vars});
    let bootstrap = format!(
        r#"
const state = {seed};
const changes = {{}};
const output = [];
const results = [];
const runtimeChanges = {{}};
const console = {{ log: (...xs) => output.push(xs.map(x => typeof x === 'string' ? x : JSON.stringify(x)).join(' ')), error: (...xs) => output.push(xs.join(' ')), warn: (...xs) => output.push(xs.join(' ')) }};
function jsonPath(value, path) {{
 if (!path || path === '$') return value;
 const parts = path.replace(/^\$\.?/, '').replace(/\[(\d+)\]/g, '.$1').split('.');
 return parts.reduce((v,k) => v == null ? undefined : v[k], value);
}}
const api = {{
 config: state.request, request: state.request, response: state.response,
 env: {{get: k => state.env[k], set: (k,v) => {{ state.env[k] = v; changes[k] = v; }} }},
 vars: {{get: k => state.vars[k], set: (k,v) => {{state.vars[k] = v;runtimeChanges[k]=v;}} }}
}};
if (api.response) api.response.json = path => jsonPath(JSON.parse(api.response.body), path);
const fuck = api;
const req = state.request;
req.getUrl = () => req.url;
req.setUrl = value => req.url = value;
req.getMethod = () => req.method;
req.setMethod = value => req.method = value;
req.setHeader = (name,value) => {{ req.headers = (req.headers || []).filter(h => h.name.toLowerCase() !== name.toLowerCase()); req.headers.push({{name,value:String(value)}}); }};
req.getHeader = name => (req.headers || []).find(h => h.name.toLowerCase() === name.toLowerCase())?.value;
req.setBody = value => req.body = {{type:'json',data:typeof value === 'string' ? value : JSON.stringify(value)}};
const res = state.response ? Object.assign((path) => jsonPath(JSON.parse(state.response.body),path), {{
status:state.response.status, headers:state.response.headers, responseTime:state.response.time,
get body() {{try {{return JSON.parse(state.response.body);}} catch {{return state.response.body;}} }},
getStatus:()=>state.response.status,getHeaders:()=>state.response.headers,
getHeader:name=>state.response.headers[name.toLowerCase()],getBody:()=>{{try{{return JSON.parse(state.response.body);}}catch{{return state.response.body;}}}},
setBody:value=>state.response.body=typeof value==='string'?value:JSON.stringify(value),
jq:path=>/^\$?res\.(status|headers|responseTime)/.test(path)?jsonQuery(path.replace(/^\$?res\./,''),{{status:state.response.status,headers:state.response.headers,responseTime:state.response.time}}):jsonQuery(path.replace(/^\$?res\.body\.?/,''),JSON.parse(state.response.body))
}}) : null;
const $res=res;const $req=req;
req.getHeaders=()=>Object.fromEntries((req.headers||[]).map(h=>[h.name,h.value]));
req.getBody=()=>req.body?.type==='json'?JSON.parse(req.body.data):req.body?.data;
req.deleteHeader=name=>req.headers=(req.headers||[]).filter(h=>h.name.toLowerCase()!==name.toLowerCase());
const bru = {{getEnvVar:api.env.get,setEnvVar:api.env.set,getVar:api.vars.get,setVar:api.vars.set,hasVar:k=>Object.hasOwn(state.vars,k),hasEnvVar:k=>Object.hasOwn(state.env,k),getAllVars:()=>state.vars,getAllEnvVars:()=>state.env}};
function test(name, fn) {{try{{const pending=fn();if(pending && typeof pending.then==='function') throw new Error('Async tests are not yet supported');results.push({{name,passed:true,error:null}});}}catch(e){{results.push({{name,passed:false,error:String(e)+String(e.stack||'')}});}}}}
"#
    );
    context.with(|ctx| -> Result<()> {
        // SAFETY: These immutable bytes were produced above from the embedded
        // library by this process's QuickJS. They outlive this fresh runtime.
        let module = unsafe { rquickjs::Module::load(ctx.clone(), &bytecode) }?;
        module.eval()?.1.finish::<()>()?;
        ctx.eval::<(), _>(bootstrap).map_err(|e| anyhow!("脚本初始化失败：{e}"))?;
        let source = format!("JSON.stringify((() => {{ try {{ (() => {{\n{code}\n}})(); return {{request:state.request,response:state.response,env:state.env,vars:state.vars,mutations:changes,logs:output,tests:results,runtimeChanges}}; }} catch(e) {{return {{error:String(e) + String(e.stack || '')}};}} }})())");
        let output: String = ctx.eval(source).map_err(|e| anyhow!("脚本执行失败或超过 2 秒限制：{e}"))?;
        let result: Value = serde_json::from_str(&output)?;
        ensure!(result.get("error").is_none(), "脚本错误：{}", result["error"].as_str().unwrap_or("未知错误"));
        *request = result["request"].clone(); *env = result["env"].clone(); *vars = result["vars"].clone();
        if let Some(response) = response { *response = result["response"].clone(); }
        for (key,value) in result["mutations"].as_object().into_iter().flatten() { mutations[key] = value.clone(); }
        report.logs.extend(result["logs"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned));
        for(key,value)in result["runtimeChanges"].as_object().into_iter().flatten(){report.variables[key]=value.clone();}
        report.tests.extend(serde_json::from_value::<Vec<TestResult>>(result["tests"].clone())?);
        Ok(())
    })
}
