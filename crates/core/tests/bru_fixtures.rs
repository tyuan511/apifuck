//! Public fixtures from Bruno commit 23fcb633; MIT notice: vendor/bruno/LICENSE.
use apifuck_core::bru;
use serde_json::Value;
#[test]
fn upstream_request_corpus_survives_bru_yaml_bru_roundtrip() {
    let fixtures = [
        ("request", include_str!("fixtures/bru/request.bru")),
        ("annotations", include_str!("fixtures/bru/annotations.bru")),
        (
            "custom-method",
            include_str!("fixtures/bru/custom-method.bru"),
        ),
        (
            "settings-minimal",
            include_str!("fixtures/bru/settings-minimal.bru"),
        ),
        (
            "settings-all",
            include_str!("fixtures/bru/settings-all-options.bru"),
        ),
        (
            "complex-auth",
            include_str!("fixtures/bru/complex-with-auth.bru"),
        ),
    ];
    for (name, source) in fixtures {
        let yaml =
            bru::convert(source, "request", "import").unwrap_or_else(|e| panic!("{name}: {e:#}"));
        let encoded =
            bru::convert(&yaml, "request", "export").unwrap_or_else(|e| panic!("{name}: {e:#}"));
        let decoded =
            bru::convert(&encoded, "request", "import").unwrap_or_else(|e| panic!("{name}: {e:#}"));
        assert_eq!(
            serde_yaml::from_str::<Value>(&yaml).unwrap(),
            serde_yaml::from_str::<Value>(&decoded).unwrap(),
            "{name}"
        );
    }
}
