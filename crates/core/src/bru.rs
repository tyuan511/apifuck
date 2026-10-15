//! Fixed upstream Bruno parser/converters, embedded in a host-free QuickJS context.
use anyhow::{Context, Result, anyhow, ensure};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const BRIDGE_SOURCE: &str = include_str!("../resources/javascript/bridge.bundle.js");
struct Converter {
    context: rquickjs::Context,
    runtime: rquickjs::Runtime,
    deadline: Arc<Mutex<Instant>>,
}
impl Converter {
    fn new() -> Result<Self> {
        let runtime = rquickjs::Runtime::new()?;
        runtime.set_memory_limit(128 * 1024 * 1024);
        runtime.set_max_stack_size(2 * 1024 * 1024);
        let deadline = Arc::new(Mutex::new(Instant::now() + Duration::from_secs(30)));
        let end = deadline.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || {
            Instant::now() >= *end.lock().unwrap()
        })));
        let context = rquickjs::Context::full(&runtime)?;
        context.with(|ctx| -> Result<()> {
            ctx.eval::<(), _>("globalThis.console={log(){},warn(){},error(){}};")?;
            ctx.eval::<(), _>(BRIDGE_SOURCE).map_err(|e| {
                let caught = ctx.catch();
                anyhow!(
                    "Bru 转换器加载失败：{e}; {:?}",
                    caught.as_exception().and_then(|e| e.message())
                )
            })?;
            Ok(())
        })?;
        Ok(Self {
            context,
            runtime,
            deadline,
        })
    }
    fn convert(&self, source: &str, kind: &str, direction: &str) -> Result<String> {
        *self.deadline.lock().unwrap() = Instant::now() + Duration::from_secs(10);
        self.context.with(|ctx| {
            ctx.eval::<(), _>(format!(
                "convertBru({}, {}, {})",
                json!(kind),
                json!(source),
                json!(direction)
            ))
        })?;
        while self.runtime.is_job_pending() {
            self.runtime
                .execute_pending_job()
                .map_err(|_| anyhow!("Bru 转换任务失败"))?;
        }
        let result: String = self.context.with(|ctx| ctx.globals().get("bruResult"))?;
        let result: Value = serde_json::from_str(&result)?;
        ensure!(
            result.get("error").is_none(),
            "Bru 转换失败：{}",
            result["error"].as_str().unwrap_or("未知错误")
        );
        let output = result["value"]
            .as_str()
            .context("转换器没有返回文档")?
            .to_owned();
        self.runtime.run_gc();
        Ok(output)
    }
}
thread_local! {static CONVERTER:RefCell<Option<Converter>>=const{RefCell::new(None)};}
pub fn convert(source: &str, kind: &str, direction: &str) -> Result<String> {
    ensure!(
        matches!(kind, "request" | "folder" | "environment"),
        "未知 Bru 文档类型"
    );
    ensure!(matches!(direction, "import" | "export"), "未知转换方向");
    CONVERTER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(Converter::new()?);
        }
        slot.as_ref().unwrap().convert(source, kind, direction)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn http_bru_roundtrip_keeps_scripts_body_and_disabled_headers() {
        let bru = "meta {\n  name: Test\n  type: http\n  seq: 1\n}\npost {\n  url: https://example.com\n  body: json\n  auth: none\n}\nheaders {\n  X-Client: native\n  ~X-Disabled: value\n}\nbody:json {\n  {\"hello\":\"world\"}\n}\nscript:pre-request {\n  bru.setVar('name', 'Ada');\n}\ntests {\n  test('ok', () => expect(res.status).to.equal(200));\n}\n";
        let yaml = convert(bru, "request", "import").unwrap();
        let value: Value = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(value["http"]["method"], "POST");
        assert_eq!(value["http"]["headers"][1]["disabled"], true);
        assert!(value["runtime"]["scripts"].as_array().unwrap().len() >= 2);
        let exported = convert(&yaml, "request", "export").unwrap();
        let again: Value =
            serde_yaml::from_str(&convert(&exported, "request", "import").unwrap()).unwrap();
        assert_eq!(value, again);
    }
    #[test]
    fn environments_and_invalid_bru() {
        let yaml = convert("vars {\n  host: localhost\n}\n", "environment", "import").unwrap();
        assert!(yaml.contains("localhost"));
        assert!(convert("this is not a request", "request", "import").is_err());
    }
}
