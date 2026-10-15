pub const OPERATORS: &[&str] = &[
    "eq",
    "neq",
    "gt",
    "gte",
    "lt",
    "lte",
    "in",
    "notIn",
    "contains",
    "notContains",
    "length",
    "matches",
    "notMatches",
    "startsWith",
    "endsWith",
    "between",
    "isEmpty",
    "isNotEmpty",
    "isNull",
    "isUndefined",
    "isDefined",
    "isTruthy",
    "isFalsy",
    "isJson",
    "isNumber",
    "isString",
    "isBoolean",
    "isArray",
];
use crate::engine::interpolate;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
pub fn assertion_script(assertions: &[Value], env: &Value, vars: &Value) -> Result<String> {
    let mut variables = env.clone();
    for (k, v) in vars.as_object().into_iter().flatten() {
        variables[k] = v.clone();
    }
    let mut code = String::new();
    for assertion in assertions.iter().filter(|a| a["disabled"] != true) {
        let expression = assertion["expression"]
            .as_str()
            .context("断言缺少 expression")?;
        let operator = assertion["operator"].as_str().unwrap_or("eq");
        let raw = interpolate(assertion["value"].as_str().unwrap_or(""), &variables)?;
        let rhs = if raw == "undefined" {
            "undefined".into()
        } else {
            serde_json::from_str::<Value>(&raw)
                .unwrap_or_else(|_| json!(raw.trim_matches('\'')))
                .to_string()
        };
        let lhs = format!("({expression})");
        let expected = format!("expect({lhs})");
        let check = match operator {
            "eq" => format!("{expected}.to.equal({rhs})"),
            "neq" => format!("{expected}.not.to.equal({rhs})"),
            "gt" => format!("{expected}.to.be.greaterThan({rhs})"),
            "gte" => format!("{expected}.to.be.at.least({rhs})"),
            "lt" => format!("{expected}.to.be.lessThan({rhs})"),
            "lte" => format!("{expected}.to.be.at.most({rhs})"),
            "in" => format!("{expected}.to.be.oneOf({rhs})"),
            "notIn" => format!("{expected}.not.to.be.oneOf({rhs})"),
            "contains" => format!("{expected}.to.include({rhs})"),
            "notContains" => format!("{expected}.not.to.include({rhs})"),
            "length" => format!("{expected}.to.have.lengthOf({rhs})"),
            "matches" => format!("{expected}.to.match(new RegExp({rhs}))"),
            "notMatches" => format!("{expected}.not.to.match(new RegExp({rhs}))"),
            "startsWith" => format!("expect({lhs}.startsWith({rhs})).to.equal(true)"),
            "endsWith" => format!("expect({lhs}.endsWith({rhs})).to.equal(true)"),
            "between" => format!("{expected}.to.be.within(...{rhs})"),
            "isEmpty" => format!("{expected}.to.be.empty"),
            "isNotEmpty" => format!("{expected}.not.to.be.empty"),
            "isNull" => format!("{expected}.to.be.null"),
            "isUndefined" => format!("{expected}.to.be.undefined"),
            "isDefined" => format!("{expected}.not.to.be.undefined"),
            "isTruthy" => format!("{expected}.to.be.true"),
            "isFalsy" => format!("{expected}.to.be.false"),
            "isNumber" => format!("{expected}.to.be.a('number')"),
            "isString" => format!("{expected}.to.be.a('string')"),
            "isBoolean" => format!("{expected}.to.be.a('boolean')"),
            "isArray" => format!("{expected}.to.be.an('array')"),
            "isJson" => {
                format!("expect(typeof {lhs} === 'object' && {lhs} !== null).to.equal(true)")
            }
            _ => bail!("不支持的断言操作符：{operator}"),
        };
        code.push_str(&format!(
            "test({},()=>{{ {check}; }});\n",
            json!(format!("{expression} {operator} {raw}"))
        ));
    }
    Ok(code)
}
