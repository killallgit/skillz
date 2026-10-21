//! Usage counts from `~/.claude.json`.

use serde_json::Value;

pub(super) fn claude_uses(json: &Value) -> Vec<(String, u64)> {
    let entries = |field: &str| {
        json.get(field)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.clone(), v.get("lastUsedAt")?.as_u64()?, v)))
    };
    let skills = entries("skillUsage")
        .filter(|(k, ..)| !k.contains(':'))
        .map(|(k, ms, _)| (format!("skill:{k}"), ms));
    let plugins = entries("pluginUsage")
        .filter(|(_, _, v)| v.get("usageCount").and_then(Value::as_u64) > Some(0))
        .map(|(k, ms, _)| (format!("plugin:{k}"), ms));
    skills.chain(plugins).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usage_counts_loose_skills_and_used_plugins() {
        let json = json!({
            "skillUsage": {
                "tdd": {"usageCount": 3, "lastUsedAt": 10},
                "caveman:caveman": {"usageCount": 1, "lastUsedAt": 20},
                "broken": {"usageCount": 1}
            },
            "pluginUsage": {
                "caveman@caveman": {"usageCount": 5, "lastUsedAt": 30},
                "idle@mp": {"usageCount": 0, "lastUsedAt": 40}
            }
        });

        let mut uses = claude_uses(&json);
        uses.sort();

        assert_eq!(
            uses,
            [
                ("plugin:caveman@caveman".into(), 30),
                ("skill:tdd".into(), 10)
            ]
        );
    }
}
