//! Generate the configuration reference page (`AsciiDoc`) from the JSON Schema of `AppConfig`
//! and the serialised defaults, so docs and code cannot drift apart.

use std::fmt::Write as _;

use serde_json::Value;

use crate::{AppConfig, ENV_PREFIX};

struct Ctx<'a> {
    defs: &'a serde_json::Map<String, Value>,
}

impl<'a> Ctx<'a> {
    /// Follow `$ref` and unwrap `Option<T>` (`anyOf [T, null]`).
    fn resolve(&self, mut v: &'a Value) -> &'a Value {
        loop {
            if let Some(r) = v.get("$ref").and_then(Value::as_str) {
                let name = r.trim_start_matches("#/$defs/");
                match self.defs.get(name) {
                    Some(d) => v = d,
                    None => return v,
                }
                continue;
            }
            if let Some(any) = v.get("anyOf").and_then(Value::as_array) {
                let non_null: Vec<&Value> = any
                    .iter()
                    .filter(|s| s.get("type").and_then(Value::as_str) != Some("null"))
                    .collect();
                if non_null.len() == 1 {
                    v = non_null[0];
                    continue;
                }
            }
            return v;
        }
    }

    fn type_name(&self, v: &Value) -> String {
        let v = self.resolve(v);
        let consts = |list: &Vec<Value>| -> Vec<String> {
            list.iter()
                .filter_map(|s| {
                    s.get("const")
                        .or_else(|| s.get("enum").and_then(|e| e.get(0)))
                        .and_then(Value::as_str)
                        .map(|c| format!("`{c}`"))
                })
                .collect()
        };
        if let Some(e) = v.get("enum").and_then(Value::as_array) {
            let vals: Vec<String> = e
                .iter()
                .filter_map(Value::as_str)
                .map(|c| format!("`{c}`"))
                .collect();
            return vals.join(" \\| ");
        }
        if let Some(one) = v.get("oneOf").and_then(Value::as_array) {
            let vals = consts(one);
            if !vals.is_empty() {
                return vals.join(" \\| ");
            }
        }
        match v.get("type") {
            Some(Value::String(t)) if t == "array" => {
                let item = v.get("items").map_or("any".into(), |i| self.type_name(i));
                format!("list of {item}")
            }
            Some(Value::String(t)) => t.clone(),
            Some(Value::Array(ts)) => ts
                .iter()
                .filter_map(Value::as_str)
                .filter(|t| *t != "null")
                .collect::<Vec<_>>()
                .join(" or "),
            _ => "any".into(),
        }
    }
}

fn is_table(v: &Value) -> bool {
    v.get("properties").is_some()
}

fn description(v: &Value) -> String {
    v.get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .replace('\n', " ")
        .replace('|', "\\|")
}

fn default_str(v: Option<&toml::Value>) -> String {
    match v {
        None | Some(toml::Value::Table(_)) => "-".into(),
        Some(v) => format!("`{v}`"),
    }
}

fn env_name(path: &[String]) -> String {
    format!("{ENV_PREFIX}{}", path.join("__").to_ascii_uppercase())
}

fn section(
    ctx: &Ctx<'_>,
    out: &mut String,
    path: &[String],
    schema: &Value,
    defaults: Option<&toml::Value>,
) {
    let schema = ctx.resolve(schema);
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    let _ = writeln!(out, "\n== `[{}]`\n", path.join("."));
    let desc = description(schema);
    if !desc.is_empty() {
        let _ = writeln!(out, "{desc}\n");
    }
    let _ = writeln!(
        out,
        "[cols=\"2,3,2,2,4\"]\n|===\n|Key |Environment |Type |Default |Description\n"
    );
    let mut nested = Vec::new();
    for (key, prop) in props {
        let resolved = ctx.resolve(prop);
        let mut p = path.to_vec();
        p.push(key.clone());
        let def = defaults.and_then(|d| d.get(key));
        if is_table(resolved) {
            nested.push((p, prop, def));
            continue;
        }
        if let Some(map_of) = resolved.get("additionalProperties")
            && is_table(ctx.resolve(map_of))
        {
            let mut mp = p.clone();
            mp.push("<id>".into());
            nested.push((mp, map_of, None));
            continue;
        }
        let mut desc = description(prop);
        if desc.is_empty() {
            desc = description(resolved);
        }
        let _ = writeln!(
            out,
            "|`{key}` |`{}` |{} |{} |{desc}",
            env_name(&p),
            ctx.type_name(prop),
            default_str(def),
        );
    }
    let _ = writeln!(out, "|===");
    for (p, s, d) in nested {
        section(ctx, out, &p, s, d);
    }
}

/// The configuration reference page.
///
/// # Panics
/// If the defaults cannot be serialised (a bug).
#[must_use]
pub fn reference_adoc() -> String {
    let schema = schemars::schema_for!(AppConfig);
    let root = schema.as_value();
    let empty = serde_json::Map::new();
    let ctx = Ctx {
        defs: root
            .get("$defs")
            .and_then(Value::as_object)
            .unwrap_or(&empty),
    };
    let defaults = toml::Value::try_from(AppConfig::default()).expect("defaults serialise");
    let mut out = String::from(
        "= Configuration reference\n\
         // Generated from the config structs by `UPDATE_DOCS=1 cargo test -p config`. Do not edit.\n\n\
         The server reads an optional TOML file (`--config <path>`, example: `config/app.example.toml`)\n\
         and then environment variables. Every key can be set as `APP__<SECTION>__<KEY>`\n\
         (nested tables add further `__` parts). Secrets (API keys, the Meilisearch master key,\n\
         OIDC client secret, S3 secret key) should only be set through the environment;\n\
         they are never written back or logged.\n",
    );
    let props = root
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, prop) in &props {
        let resolved = ctx.resolve(prop);
        if let Some(map_of) = resolved.get("additionalProperties")
            && is_table(ctx.resolve(map_of))
        {
            section(&ctx, &mut out, &[key.clone(), "<id>".into()], map_of, None);
        } else {
            section(
                &ctx,
                &mut out,
                std::slice::from_ref(key),
                prop,
                defaults.get(key),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn lists_every_section_and_env_name() {
        let doc = super::reference_adoc();
        for s in [
            "== `[server]`",
            "== `[storage]`",
            "== `[storage.s3]`",
            "== `[connectors.<id>]`",
        ] {
            assert!(doc.contains(s), "missing {s}");
        }
        assert!(doc.contains("`APP__STORAGE__MAX_TOTAL_BYTES`"));
        assert!(doc.contains("`\"20GiB\"`"), "defaults are shown");
    }
}
