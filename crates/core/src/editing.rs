//! Apply form edits without rewriting unrelated YAML comments and formatting.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::str::FromStr;
pub fn update(source: &str, value: &Value) -> Result<String> {
    let previous: Value = serde_yaml::from_str(source)?;
    let file = yaml_edit::YamlFile::from_str(source).context("无法编辑 YAML 语法树")?;
    let document = file.document().context("缺少 YAML 文档")?;
    let mapping = document.as_mapping().context("文档根节点必须是映射")?;
    fn apply(mapping: yaml_edit::Mapping, old: &Value, new: &Value) -> Result<()> {
        let old = old.as_object().context("原节点不是映射")?;
        let new = new.as_object().context("新节点不是映射")?;
        for (key, value) in new {
            if old.get(key) == Some(value) {
                continue;
            }
            if old.get(key).is_some_and(Value::is_object)
                && value.as_object().is_some_and(|map| !map.is_empty())
                && let Some(child) = mapping
                    .get(key.as_str())
                    .and_then(|v| v.as_mapping().cloned())
            {
                apply(child, &old[key], value)?;
                continue;
            }
            let fragment =
                yaml_edit::Document::from_str(&serde_yaml::to_string(&json!({"value":value}))?)?;
            let node = fragment
                .as_mapping()
                .and_then(|m| m.get("value"))
                .context("无法构造字段")?;
            mapping.set(key.as_str(), node);
        }
        for key in old.keys().filter(|key| !new.contains_key(*key)) {
            mapping.remove(key.as_str());
        }
        Ok(())
    }
    apply(mapping, &previous, value)?;
    let result = file.to_string();
    ensure!(
        serde_yaml::from_str::<Value>(&result)? == *value,
        "编辑后语义不一致，未保存文件"
    );
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unmodified_comments_and_quoted_values() {
        let source = "# API comment\ninfo:\n  name: 'Hello' # keep\n  type: http\nhttp:\n  method: GET\n  url: https://example.com\n  # Preserve this header comment\n  headers:\n    - name: Accept\n      value: application/json\n";
        let mut value: Value = serde_yaml::from_str(source).unwrap();
        value["http"]["method"] = json!("POST");
        value["http"]["body"] = json!({"type":"json","data":"{\"hello\":\"world\"}"});
        let result = update(source, &value).unwrap();
        assert!(result.contains("# API comment"));
        assert!(result.contains("name: 'Hello' # keep"));
        assert!(result.contains("# Preserve this header comment"));
    }
    #[test]
    fn removes_auth_without_removing_other_fields() {
        let source = "http:\n  url: https://example.com\n  auth: inherit\n";
        let new = json!({"http":{"url":"https://example.com"}});
        assert_eq!(
            serde_yaml::from_str::<Value>(&update(source, &new).unwrap()).unwrap(),
            new
        );
    }
}
