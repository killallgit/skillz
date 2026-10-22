//! `profile list` and `profile status`: how each profile compares with what the project has.

use std::io::Write;

use serde_json::{Value, json};

use super::deps::Deps;
use super::scope::installed;
use crate::apply::{Standing, compare, drift};
use crate::env::Env;
use crate::model::{Augmentation, Scope};
use crate::profile::{Entry, Profile, Store};
use crate::stale::{Disk, Probe, missing_source};

pub(super) fn list(deps: &Deps, env: &Env, json: bool, out: &mut dyn Write) -> anyhow::Result<()> {
    let store = Store::in_env(env);
    let here = installed(deps, env, Scope::Project);
    let readable = store.readable()?;
    let rows: Vec<Value> = store
        .names()?
        .into_iter()
        .map(|name| match readable.iter().find(|p| p.name == name) {
            Some(profile) => row_of(profile, &here),
            None => json!({"name": name, "readable": false}),
        })
        .collect();
    if json {
        return Ok(writeln!(out, "{}", serde_json::to_string_pretty(&rows)?)?);
    }
    rows.iter()
        .try_for_each(|row| writeln!(out, "{}", line_of(row)))?;
    Ok(())
}

fn row_of(profile: &Profile, here: &[Augmentation]) -> Value {
    let drift = drift(profile, here);
    json!({
        "name": profile.name,
        "readable": true,
        "entries": profile.entries.len(),
        "matches": drift.matches(),
        "missing": drift.missing,
        "extra": drift.extra,
    })
}

fn line_of(row: &Value) -> String {
    let name = row["name"].as_str().unwrap_or_default();
    match (row["readable"].as_bool(), row["matches"].as_bool()) {
        (Some(false), _) => format!("{name}  (unreadable)"),
        (_, Some(true)) => format!("{name}  matches"),
        _ => format!("{name}  {} missing, {} extra", row["missing"], row["extra"]),
    }
}

pub(super) fn status(
    deps: &Deps,
    env: &Env,
    (name, json): (Option<&str>, bool),
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let store = Store::in_env(env);
    let profiles = match name {
        Some(name) => vec![store.load_flat(name)?],
        None => store.readable()?,
    };
    let here = installed(deps, env, Scope::Project);
    let rows: Vec<Value> = profiles
        .iter()
        .map(|p| standing_json(&p.name, &compare(&p.resolved(), &here), &Disk))
        .collect();
    if json {
        return Ok(writeln!(out, "{}", serde_json::to_string_pretty(&rows)?)?);
    }
    rows.iter().try_for_each(|row| write_standing(row, out))
}

fn standing_json(name: &str, standing: &Standing, probe: &dyn Probe) -> Value {
    let labels =
        |entries: &[Entry]| -> Vec<String> { entries.iter().map(ToString::to_string).collect() };
    let unavailable: Vec<String> = standing
        .present
        .iter()
        .chain(&standing.missing)
        .filter(|entry| missing_source(entry, probe).is_some())
        .map(ToString::to_string)
        .collect();
    let extra: Vec<Value> = standing
        .extras
        .iter()
        .map(|e| json!({"item": e.item.label(true), "sourced": e.sourced}))
        .collect();
    json!({
        "name": name,
        "matches": standing.missing.is_empty() && extra.is_empty(),
        "present": labels(&standing.present),
        "missing": labels(&standing.missing),
        "extra": extra,
        "unavailable": unavailable,
    })
}

fn write_standing(row: &Value, out: &mut dyn Write) -> anyhow::Result<()> {
    writeln!(out, "{}", line_of_status(row))?;
    for (heading, key) in [("have", "present"), ("lack", "missing")] {
        for entry in row[key].as_array().into_iter().flatten() {
            writeln!(out, "  {heading:<5}{}", entry.as_str().unwrap_or_default())?;
        }
    }
    for entry in row["unavailable"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "  ! source missing: {}",
            entry.as_str().unwrap_or_default()
        )?;
    }
    for extra in row["extra"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "  extra {}",
            extra["item"].as_str().unwrap_or_default()
        )?;
    }
    Ok(())
}

fn line_of_status(row: &Value) -> String {
    let name = row["name"].as_str().unwrap_or_default();
    match row["matches"].as_bool() {
        Some(true) => format!("{name}: matches"),
        _ => format!("{name}:"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Agent;
    use crate::stale::tests::{Existing, skill};

    #[test]
    fn an_entry_whose_local_source_is_gone_is_unavailable_apart_from_missing() {
        let wanted = [skill("/old/skills", "mine"), skill("/new/skills", "kept")]
            .map(|entry| entry.with_agents(vec![Agent::Claude]));
        let standing = compare(&wanted, &[]);

        let row = standing_json("p", &standing, &Existing::of(&["/new/skills"]));

        // Assert both entries are missing from the project but only one has lost its source.
        assert_eq!(row["missing"].as_array().unwrap().len(), 2);
        assert_eq!(row["unavailable"].as_array().unwrap().len(), 1);
    }
}
