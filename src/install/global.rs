//! Skills about to be installed into a project that the user already has installed globally.

use super::{Ctx, InstallRequest};
use crate::agents::inventory;
use crate::apply::holds;
use crate::model::{Augmentation, Scope};
use crate::profile::Entry;

/// Ask before installing `names` into the project when some are installed globally already.
/// Says which, then asks; `false` when the user would rather not.
pub(super) fn confirm_duplicates(
    ctx: &Ctx,
    req: &InstallRequest,
    (source, names): (&str, &[String]),
) -> anyhow::Result<bool> {
    let global = inventory(ctx.adapters, ctx.env, &[Scope::Global]).items;
    let project = inventory(ctx.adapters, ctx.env, &[Scope::Project]).items;
    let held = only_global(&(source, &req.agents), names, (&global, &project));
    if held.is_empty() {
        return Ok(true);
    }
    eprintln!("already installed globally: {}", held.join(", "));
    ctx.prompter.confirm("Also install into this project?")
}

/// The names that are installed globally but not in the project.
fn only_global(
    (source, agents): &(&str, &[crate::model::Agent]),
    names: &[String],
    (global, project): (&[Augmentation], &[Augmentation]),
) -> Vec<String> {
    names
        .iter()
        .filter(|name| {
            let entry = Entry::Skill {
                source: source.to_string(),
                names: vec![name.to_string()],
                agents: agents.to_vec(),
            };
            holds(global, &entry) && !holds(project, &entry)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Installer, Kind};

    fn skill(name: &str, scope: Scope) -> Augmentation {
        Augmentation {
            hooks: vec![],
            members: Vec::new(),
            id: format!("skill:{name}"),
            name: name.into(),
            kind: Kind::Skill,
            agents: vec![Agent::Claude],
            scope,
            group: "o/r".into(),
            source: Some("o/r".into()),
            installer: Installer::Npx,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn only_the_skills_installed_globally_and_not_in_the_project_are_reported() {
        let global = [skill("tdd", Scope::Global), skill("both", Scope::Global)];
        let project = [skill("both", Scope::Project)];
        let wanted = names(&["tdd", "both", "new"]);

        let held = only_global(&("o/r", &[Agent::Claude]), &wanted, (&global, &project));

        assert_eq!(held, names(&["tdd"]));
    }

    #[test]
    fn a_skill_from_another_source_is_not_a_duplicate() {
        let global = [skill("tdd", Scope::Global)];

        let held = only_global(&("x/y", &[Agent::Claude]), &names(&["tdd"]), (&global, &[]));

        assert!(held.is_empty());
    }
}
