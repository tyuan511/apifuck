use super::{RunEntry, RunReport};
use anyhow::Result;
use serde_json::json;
#[derive(Clone, Copy)]
pub enum Format {
    Json,
    Junit,
    Html,
}
impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Junit => "xml",
            Self::Html => "html",
        }
    }
}
fn escape(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            matches!(*c, '\t' | '\n' | '\r') || (*c >= ' ' && *c != '\u{fffe}' && *c != '\u{ffff}')
        })
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&apos;".into(),
            _ => c.to_string(),
        })
        .collect()
}
fn errors(entry: &RunEntry) -> String {
    let mut errors = entry.error.iter().cloned().collect::<Vec<_>>();
    if let Some(response) = &entry.response {
        errors.extend(
            response
                .tests
                .iter()
                .filter(|test| !test.passed)
                .map(|test| {
                    format!(
                        "{}: {}",
                        test.name,
                        test.error.as_deref().unwrap_or("Assertion failed")
                    )
                }),
        );
    }
    errors.join("\n")
}
/// Reports intentionally omit request credentials, data rows, headers, bodies and console logs.
pub fn render(report: &RunReport, format: Format) -> Result<String> {
    match format {
        Format::Json => Ok(serde_json::to_string_pretty(
            &json!({"planned":report.planned,"completed":report.entries.len(),"cancelled":report.cancelled,"passed":report.passed(),"elapsedMs":report.elapsed_ms,"entries":report.entries.iter().map(|entry|json!({"iteration":entry.iteration,"dataRow":entry.data_row,"path":entry.path,"name":entry.name,"passed":entry.passed,"skipped":entry.skipped,"status":entry.response.as_ref().map(|r|r.status),"elapsedMs":entry.elapsed_ms,"error":entry.error,"tests":entry.response.as_ref().map(|r|&r.tests)})).collect::<Vec<_>>()}),
        )?),
        Format::Junit => {
            let failures = report
                .entries
                .iter()
                .filter(|entry| !entry.passed && entry.error.is_none() && !entry.skipped)
                .count();
            let errors_count = report
                .entries
                .iter()
                .filter(|entry| entry.error.is_some() && !entry.skipped)
                .count()
                + usize::from(report.cancelled);
            let skipped = report.entries.iter().filter(|entry| entry.skipped).count();
            let mut xml = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites><testsuite name=\"ApiFuck\" tests=\"{}\" failures=\"{failures}\" errors=\"{errors_count}\" skipped=\"{skipped}\" time=\"{:.3}\">\n",
                report.entries.len() + usize::from(report.cancelled),
                report.elapsed_ms as f64 / 1000.
            );
            for entry in &report.entries {
                xml.push_str(&format!(
                    "<testcase classname=\"{}\" name=\"{} [iteration {}, row {}]\" time=\"{:.3}\">",
                    escape(&entry.path.display().to_string()),
                    escape(&entry.name),
                    entry.iteration,
                    entry
                        .data_row
                        .map(|row| row.to_string())
                        .unwrap_or_else(|| "-".into()),
                    entry.elapsed_ms as f64 / 1000.
                ));
                if entry.skipped {
                    xml.push_str("<skipped message=\"Unsupported protocol in runner\"/>");
                } else if !entry.passed {
                    let tag = if entry.error.is_some() {
                        "error"
                    } else {
                        "failure"
                    };
                    xml.push_str(&format!(
                        "<{tag} message=\"Request failed\">{}</{tag}>",
                        escape(&errors(entry))
                    ));
                }
                xml.push_str("</testcase>\n");
            }
            if report.cancelled {
                xml.push_str("<testcase name=\"Run cancelled\"><error message=\"Run cancelled before normal completion\"/></testcase>\n");
            }
            xml.push_str("</testsuite></testsuites>\n");
            Ok(xml)
        }
        Format::Html => {
            let mut html = format!(
                "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>ApiFuck run report</title><style>body{{font:16px system-ui;margin:2rem;color:#20242b;background:#fff}}table{{border-collapse:collapse;width:100%}}td,th{{text-align:left;border:1px solid #ccc;padding:.6rem;vertical-align:top}}pre{{white-space:pre-wrap;overflow-wrap:anywhere}}.fail{{color:#b42318}}</style><h1>ApiFuck run report</h1><p>{} · {} / {} requests · {} ms</p><table><thead><tr><th>Request</th><th>Iteration / row</th><th>Result</th><th>HTTP</th><th>ms</th><th>Details</th></tr></thead><tbody>",
                if report.cancelled {
                    "Cancelled"
                } else if report.passed() {
                    "Passed"
                } else {
                    "Failed"
                },
                report.entries.len(),
                report.planned,
                report.elapsed_ms
            );
            for entry in &report.entries {
                html.push_str(&format!("<tr><td>{}<br><small>{}</small></td><td>{} / {}</td><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td><pre>{}</pre></td></tr>",escape(&entry.name),escape(&entry.path.display().to_string()),entry.iteration,entry.data_row.map(|row|row.to_string()).unwrap_or_else(||"-".into()),if entry.passed{""}else{"fail"},if entry.skipped{"Skipped"}else if entry.passed{"Passed"}else{"Failed"},entry.response.as_ref().map(|response|response.status.to_string()).unwrap_or_default(),entry.elapsed_ms,escape(&errors(entry))));
            }
            html.push_str("</tbody></table></html>");
            Ok(html)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_a_ci_error_and_html_xml_escape_untrusted_content() {
        let report = RunReport {
            entries: vec![RunEntry {
                iteration: 1,
                data_row: Some(2),
                elapsed_ms: 10,
                path: "x.yml".into(),
                name: "<script>alert(1)</script>".into(),
                passed: false,
                skipped: false,
                response: None,
                error: Some("<&\"\u{1}".into()),
            }],
            elapsed_ms: 10,
            cancelled: true,
            planned: 2,
        };
        let xml = render(&report, Format::Junit).unwrap();
        assert!(xml.contains("errors=\"2\""));
        assert!(xml.contains("&lt;script&gt;"));
        assert!(!xml.contains('\u{1}'));
        let html = render(&report, Format::Html).unwrap();
        assert!(!html.contains("<script>"));
        let json: serde_json::Value =
            serde_json::from_str(&render(&report, Format::Json).unwrap()).unwrap();
        assert_eq!(json["entries"][0]["dataRow"], 2);
        assert!(json["entries"][0].get("body").is_none());
    }
}
