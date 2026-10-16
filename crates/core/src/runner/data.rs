use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub fn read(path: &Path) -> Result<Vec<Value>> {
    ensure!(
        std::fs::metadata(path)?.len() <= MAX_BYTES as u64,
        "数据文件不能超过 8 MiB"
    );
    parse(
        &std::fs::read_to_string(path)?,
        path.extension().and_then(|v| v.to_str()).unwrap_or(""),
    )
}
pub fn validate(rows: &[Value]) -> Result<()> {
    ensure!(rows.len() <= 10000, "数据行数不能超过 10000");
    for (index, row) in rows.iter().enumerate() {
        let row = row
            .as_object()
            .with_context(|| format!("第 {} 行必须为 JSON 对象", index + 1))?;
        ensure!(row.len() <= 256, "数据列数不能超过 256");
        ensure!(
            row.keys().all(|key| !key.trim().is_empty()),
            "数据变量名不能为空"
        );
    }
    Ok(())
}
pub fn parse(source: &str, extension: &str) -> Result<Vec<Value>> {
    ensure!(source.len() <= MAX_BYTES, "数据文件不能超过 8 MiB");
    let source = source.trim_start_matches('\u{feff}');
    let rows = match extension.to_ascii_lowercase().as_str() {
        "json" => serde_json::from_str::<Vec<Value>>(source).context("JSON 数据必须是对象数组")?,
        "csv" => {
            let mut reader = csv::ReaderBuilder::new()
                .trim(csv::Trim::Headers)
                .from_reader(source.as_bytes());
            let headers = reader.headers()?.clone();
            ensure!(!headers.is_empty(), "CSV 缺少表头");
            let mut names = std::collections::HashSet::new();
            ensure!(
                headers
                    .iter()
                    .all(|name| !name.is_empty() && names.insert(name)),
                "CSV 表头为空或重复"
            );
            let mut rows = vec![];
            for row in reader.records() {
                ensure!(rows.len() < 10000, "数据行数不能超过 10000");
                let row = row?;
                rows.push(
                    headers
                        .iter()
                        .zip(row.iter())
                        .map(|(key, value)| (key.to_owned(), json!(value)))
                        .collect::<serde_json::Map<_, _>>()
                        .into(),
                );
            }
            rows
        }
        _ => anyhow::bail!("请选择 .csv 或 .json 数据文件"),
    };
    ensure!(!rows.is_empty(), "数据文件没有记录");
    validate(&rows)?;
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_quoted_csv_bom_unicode_and_typed_json_without_coercing_strings() {
        let rows = parse(
            "\u{feff}id,name,note\r\n001,中文,\"a,b\nsecond\"\r\n",
            "csv",
        )
        .unwrap();
        assert_eq!(rows[0]["id"], "001");
        assert_eq!(rows[0]["note"], "a,b\nsecond");
        assert_eq!(
            parse("[{\"n\":2,\"active\":true}]", "json").unwrap()[0]["n"],
            2
        );
        for source in ["a,a\n1,2\n", "a,b\n1\n", "a,b\n"] {
            assert!(parse(source, "csv").is_err());
        }
        assert!(parse("[1]", "json").is_err());
        assert!(parse("[]", "json").is_err());
    }
}
