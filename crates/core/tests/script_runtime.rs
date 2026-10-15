use apifuck_core::scripts::{ScriptReport, run};
use serde_json::{Value, json};
use std::time::Instant;

fn execute(code: &str) -> anyhow::Result<ScriptReport> {
    let mut report = ScriptReport::default();
    run(
        code,
        &mut json!({"method":"GET","url":"http://localhost/"}),
        Some(&mut json!({"status":200,"headers":{},"body":"{\"ok\":true}","time":1})),
        &mut json!({}),
        &mut json!({}),
        &mut json!({}),
        &mut report,
    )?;
    Ok(report)
}

#[test]
#[ignore = "manual latency measurement; no machine-dependent timing assertions"]
fn hook_latency() {
    for (name, code) in [
        ("simple", "api.vars.set('ok', true);"),
        (
            "assertions",
            "test('status',()=>expect(api.response.status).to.equal(200));test('body',()=>expect(api.response.json().ok).to.equal(true));",
        ),
        (
            "json_schema",
            "test('schema',()=>expect(api.response.json()).to.have.jsonSchema({type:'object',required:['ok'],properties:{ok:{type:'boolean'}}}));",
        ),
    ] {
        execute(code).unwrap();
        let mut samples = Vec::new();
        for _ in 0..50 {
            let start = Instant::now();
            let report = execute(code).unwrap();
            samples.push(start.elapsed().as_secs_f64() * 1000.);
            assert!(
                report.tests.iter().all(|test| test.passed),
                "{:?}",
                report.tests
            );
        }
        samples.sort_by(f64::total_cmp);
        eprintln!(
            "{name}: median={:.3} ms, p95={:.3} ms",
            samples[25], samples[47]
        );
    }
}

#[test]
fn hook_globals_and_library_mutations_do_not_leak() {
    execute("globalThis.leaked = 1; Object.prototype.leaked = 2; expect.custom = 3; api.vars.set('local', 4);").unwrap();
    let report = execute("test('isolated',()=>{expect(globalThis.leaked).to.equal(undefined);expect(({}).leaked).to.equal(undefined);expect(expect.custom).to.equal(undefined);expect(api.vars.get('local')).to.equal(undefined);});").unwrap();
    assert_eq!(report.tests.len(), 1);
    assert!(report.tests[0].passed, "{:?}", report.tests);
}

#[test]
fn failed_hook_does_not_poison_the_next_context() {
    assert!(execute("throw new Error('expected failure');").is_err());
    assert!(execute("while (true) {}").is_err());
    let report = execute("test('fresh',()=>expect(api.response.status).to.equal(200));").unwrap();
    assert!(report.tests[0].passed);
}

#[test]
fn concurrent_hooks_keep_their_own_variables_and_libraries() {
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4).map(|id| scope.spawn(move || {
            let code = format!("globalThis.marker={id};api.vars.set('id', {id});test('own marker',()=>expect(globalThis.marker).to.equal({id}));");
            let report = execute(&code).unwrap();
            assert_eq!(report.variables["id"], Value::from(id));
            assert!(report.tests[0].passed);
        })).collect();
        for worker in workers {
            worker.join().unwrap();
        }
    });
}
