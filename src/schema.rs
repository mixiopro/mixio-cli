//! Maps an MCP tool's JSON Schema `inputSchema` to a clap `Command` at
//! runtime, and maps parsed `ArgMatches` back to a JSON `arguments` object.
//! This is the whole "dynamic CLI from MCP schema" trick — no codegen, no
//! per-tool code, works for any tool the server adds tomorrow.

use crate::mcp::McpTool;
use anyhow::{Context, Result};
use clap::{builder::PossibleValuesParser, Arg, ArgAction, Command};
use serde_json::{json, Value};
use std::collections::HashSet;

/// Builds a `Command` for one tool: every schema property becomes a flag.
/// Every value is parsed as a string by clap; `collect_arguments` does the
/// JSON-typed coercion afterward — keeps the builder side uniform instead of
/// fighting clap's generic value_parser typing for a shape we only know at
/// runtime.
pub fn build_tool_command(tool: &McpTool) -> Command {
    let mut cmd = Command::new(tool.name.clone()).about(tool.description.clone());

    let required: HashSet<&str> = tool
        .input_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let Some(props) = tool.input_schema.get("properties").and_then(Value::as_object) else {
        return cmd;
    };

    for (name, prop) in props {
        let mut arg = Arg::new(name.clone()).long(to_kebab(name));
        if let Some(desc) = prop.get("description").and_then(Value::as_str) {
            arg = arg.help(desc.to_string());
        }
        if let Some(values) = prop.get("enum").and_then(Value::as_array) {
            let possible: Vec<String> = values
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if !possible.is_empty() {
                arg = arg.value_parser(PossibleValuesParser::new(possible));
            }
        }
        if is_array(prop) {
            arg = arg.action(ArgAction::Append).num_args(1..);
        }
        // A schema `default` means the server fills it in when omitted — even
        // if the same property is also (quirkily) listed in `required`. Show
        // it in --help and don't force the flag; several tools here declare
        // both, e.g. list_projects' limit/offset (default 50 / 0).
        match prop.get("default") {
            Some(default) => arg = arg.required(false).default_value(default_to_string(default)),
            None => arg = arg.required(required.contains(name.as_str())),
        }
        cmd = cmd.arg(arg);
    }

    // Escape hatch: skip flag mapping entirely and pass a raw JSON object.
    let prop_names: Vec<String> = props.keys().cloned().collect();
    cmd = cmd.arg(
        Arg::new("__json")
            .long("json")
            .help("Raw JSON object for `arguments`, bypassing the flags above")
            .conflicts_with_all(prop_names),
    );

    cmd
}

/// Converts parsed flags back into the JSON `arguments` object `tools/call`
/// expects, coercing each value to the type its schema property declared.
pub fn collect_arguments(tool: &McpTool, matches: &clap::ArgMatches) -> Result<Value> {
    if let Some(raw) = matches.get_one::<String>("__json") {
        return serde_json::from_str(raw).context("--json was not valid JSON");
    }

    let mut out = serde_json::Map::new();
    let Some(props) = tool.input_schema.get("properties").and_then(Value::as_object) else {
        return Ok(Value::Object(out));
    };

    for (name, prop) in props {
        if is_array(prop) {
            let values: Vec<String> = matches
                .get_many::<String>(name)
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            if !values.is_empty() {
                let item_type = prop.get("items").and_then(|i| i.get("type")).and_then(Value::as_str);
                let items: Vec<Value> = values.iter().map(|v| coerce(v, item_type)).collect();
                out.insert(name.clone(), Value::Array(items));
            }
            continue;
        }
        if let Some(raw) = matches.get_one::<String>(name) {
            let ty = prop.get("type").and_then(Value::as_str);
            out.insert(name.clone(), coerce(raw, ty));
        }
    }
    Ok(Value::Object(out))
}

/// `50` -> "50", `"foo"` -> "foo" (unquoted) — a schema default rendered as
/// the raw flag string a user would've typed.
fn default_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn is_array(prop: &Value) -> bool {
    prop.get("type").and_then(Value::as_str) == Some("array")
}

/// Coerces a raw flag string to its schema-declared JSON type. Falls back to
/// leaving it as a string for object/unknown types — the user can always use
/// `--json` for those.
fn coerce(raw: &str, ty: Option<&str>) -> Value {
    match ty {
        Some("integer") => raw.parse::<i64>().map(Value::from).unwrap_or_else(|_| json!(raw)),
        // JSON Schema "number" allows fractions, but whole-number defaults
        // (limit: 50) should round-trip as JSON integers, not "50.0".
        Some("number") => raw
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| raw.parse::<f64>().map(Value::from))
            .unwrap_or_else(|_| json!(raw)),
        Some("boolean") => raw.parse::<bool>().map(Value::from).unwrap_or_else(|_| json!(raw)),
        Some("object") | Some("array") => serde_json::from_str(raw).unwrap_or_else(|_| json!(raw)),
        _ => json!(raw),
    }
}

/// `projectId` -> `project-id`, `list_projects` -> `list-projects`,
/// `already-kebab` unchanged. One-way: callers that need the original back
/// (e.g. resolving a typed `call` subcommand to its real tool name) look it
/// up against the cached tool list rather than assuming this is reversible —
/// a future tool name could legitimately already contain a hyphen. Arg flags
/// never need reversing at all: `ArgMatches` keys on the original `id`,
/// which this only renames for display (`.long()`), never touches.
pub(crate) fn to_kebab(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            out.push('-');
        } else if c.is_uppercase() && i > 0 {
            out.push('-');
            out.extend(c.to_lowercase());
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tool() -> McpTool {
        McpTool {
            name: "studio_list_projects".into(),
            description: "List projects".into(),
            input_schema: json!({
                "type": "object",
                "required": ["status"],
                "properties": {
                    "status": { "type": "string", "enum": ["active", "archived"] },
                    "limit": { "type": "integer", "description": "Page size" },
                    "tags": { "type": "array", "items": { "type": "string" } }
                }
            }),
        }
    }

    #[test]
    fn round_trips_typed_flags_into_json_arguments() {
        let tool = sample_tool();
        let cmd = build_tool_command(&tool);
        let matches = cmd
            .try_get_matches_from([
                "studio_list_projects",
                "--status",
                "active",
                "--limit",
                "10",
                "--tags",
                "a",
                "--tags",
                "b",
            ])
            .expect("valid args should parse");

        let args = collect_arguments(&tool, &matches).unwrap();
        assert_eq!(args["status"], json!("active"));
        assert_eq!(args["limit"], json!(10));
        assert_eq!(args["tags"], json!(["a", "b"]));
    }

    #[test]
    fn missing_required_flag_is_rejected() {
        let cmd = build_tool_command(&sample_tool());
        assert!(cmd.try_get_matches_from(["studio_list_projects"]).is_err());
    }

    /// Regression: list_projects et al. list `limit`/`offset` in `required`
    /// *and* give them a `default` (a zod-to-json-schema quirk) — the default
    /// must win, or the CLI demands flags the server would've filled in.
    #[test]
    fn required_property_with_a_default_is_optional_and_prefilled() {
        let tool = McpTool {
            name: "list_projects".into(),
            description: "".into(),
            input_schema: json!({
                "type": "object",
                "required": ["limit", "offset"],
                "properties": {
                    "limit": { "type": "number", "default": 50 },
                    "offset": { "type": "number", "default": 0 }
                }
            }),
        };
        let cmd = build_tool_command(&tool);
        let matches = cmd.try_get_matches_from(["list_projects"]).expect("defaults should cover required fields");
        let args = collect_arguments(&tool, &matches).unwrap();
        assert_eq!(args["limit"], json!(50));
        assert_eq!(args["offset"], json!(0));
    }

    #[test]
    fn kebab_case_conversion() {
        assert_eq!(to_kebab("projectId"), "project-id");
        assert_eq!(to_kebab("shot_id"), "shot-id");
        assert_eq!(to_kebab("plain"), "plain");
    }
}
