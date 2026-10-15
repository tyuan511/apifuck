pub fn is_event_stream(headers: &[(String, String)]) -> bool {
    headers.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case("content-type")
            && value
                .split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/event-stream"))
    })
}

/// Show each data line, including the in-progress final line, as it arrives.
pub fn event_data(body: &str) -> Vec<String> {
    body.trim_start_matches('\u{feff}')
        .split(['\r', '\n'])
        .filter_map(|line| {
            line.strip_prefix("data:")
                .map(|data| data.strip_prefix(' ').unwrap_or(data).to_owned())
                .or_else(|| (line == "data").then(String::new))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_data_retains_empty_lines_and_partial_payloads() {
        assert_eq!(
            event_data("\u{feff}:ping\r\ndata: one\r\ndata:  two\n\ndata\ndata: par"),
            ["one", " two", "", "par"]
        );
        assert!(is_event_stream(&[(
            "Content-Type".into(),
            "Text/Event-Stream; charset=utf-8".into()
        )]));
    }
}
