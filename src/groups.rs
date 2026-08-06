//! Derives `mixio <noun> <verb>` groups from tool names — no hardcoded tool
//! names, just a small stable vocabulary of CRUD-ish verbs. MCP's tools/list
//! gives name/description/schema and nothing else, so any noun/verb split is
//! a client-side guess; two guardrails keep that guess honest instead of
//! silently wrong:
//!
//! - A (noun, verb) key claimed by more than one tool aliases neither.
//!   Picking one would shadow the other — e.g. `elements_get` (a scoped
//!   read) and `get_element` (plain CRUD) both reduce to element/get but are
//!   different tools.
//! - A noun with only one verb isn't a group, it's a rename — often a bad
//!   one for compound names (`batch_submit_studio_jobs` strips `batch` as
//!   "the verb" and leaves a noun that isn't one). Require >= 2 verbs.
//!
//! Recomputed from the live tool cache on every `tools refresh`, so a
//! server-side rename either lands in a new correct group or drops out
//! safely — never silently stale, unlike a hardcoded name table.

use crate::mcp::McpTool;
use crate::schema::to_kebab;
use std::collections::BTreeMap;

// ponytail: vocabulary observed in the current tool set, not a general
// English CRUD-verb list. Extend when a new tool introduces a verb this
// doesn't recognize — it'll just stay flat under `call` until then.
const VERBS: &[&str] = &[
    "list", "create", "get", "update", "delete", "query", "tag", "bulk", "submit", "cancel",
    "register", "upload", "revise", "link", "upsert", "batch", "search", "describe",
];

/// noun -> verb -> raw tool name
pub type Groups = BTreeMap<String, BTreeMap<String, String>>;

pub fn derive(tools: &[McpTool]) -> Groups {
    let mut candidates: BTreeMap<(String, String), Vec<&str>> = BTreeMap::new();
    for tool in tools {
        if let Some((noun, verb)) = split(&tool.name) {
            candidates.entry((singular(&noun), verb)).or_default().push(&tool.name);
        }
    }

    let mut groups = Groups::new();
    for ((noun, verb), members) in candidates {
        if let [only] = members.as_slice() {
            // Kebab here, not in `candidates` — collision matching above is
            // on the raw split, display naming is a separate concern.
            groups.entry(to_kebab(&noun)).or_default().insert(to_kebab(&verb), only.to_string());
        }
        // len() > 1: collision — aliased to neither, `call <raw_name>` still works.
    }
    groups.retain(|_, verbs| verbs.len() >= 2);
    groups
}

/// Strips a leading or trailing known verb; `None` if neither end matches
/// (e.g. `ping`).
fn split(name: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() < 2 {
        return None;
    }
    if VERBS.contains(&parts[0]) {
        return Some((parts[1..].join("_"), parts[0].to_string()));
    }
    if VERBS.contains(&parts[parts.len() - 1]) {
        return Some((parts[..parts.len() - 1].join("_"), parts[parts.len() - 1].to_string()));
    }
    None
}

fn singular(noun: &str) -> String {
    noun.strip_suffix('s').unwrap_or(noun).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> McpTool {
        McpTool { name: name.into(), description: String::new(), input_schema: serde_json::json!({}) }
    }

    #[test]
    fn groups_verbs_sharing_a_noun() {
        let tools = [tool("create_project"), tool("get_project"), tool("list_projects")];
        let groups = derive(&tools);
        assert_eq!(groups["project"]["create"], "create_project");
        assert_eq!(groups["project"]["get"], "get_project");
        assert_eq!(groups["project"]["list"], "list_projects");
    }

    #[test]
    fn collision_is_excluded_from_both_sides() {
        // elements_get vs get_element: real collision from the live schema.
        let tools = [
            tool("elements_get"),
            tool("get_element"),
            tool("create_element"),
            tool("update_element"),
        ];
        let groups = derive(&tools);
        assert!(!groups["element"].contains_key("get"), "colliding verb must not be aliased");
        assert_eq!(groups["element"]["create"], "create_element");
        assert_eq!(groups["element"]["update"], "update_element");
    }

    #[test]
    fn single_verb_noun_does_not_form_a_group() {
        let tools = [tool("batch_submit_studio_jobs"), tool("ping")];
        assert!(derive(&tools).is_empty());
    }
}
