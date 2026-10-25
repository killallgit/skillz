//! Skills about to be installed into a project that the user already has installed globally.

use super::{Ctx, InstallRequest};
use crate::apply::holds;
use skillz_harness::Spec;
use skillz_harness::agents::inventory;
use skillz_harness::model::{Installed, Scope};

/// Ask before installing `names` into the project when some are installed globally already.
/// Says which, then asks; `false` when the user would rather not.
pub(super) fn confirm_duplicates(
    ctx: &Ctx,
    req: &InstallRequest,
    (source, names): (&str, &[String]),
) -> anyhow::Result<bool> {
    if req.scope == Scope::Global {
        return Ok(true);
    }
    let global = inventory(ctx.adapters, ctx.env, &[Scope::Global]).complete()?;
    let project = inventory(ctx.adapters, ctx.env, &[Scope::Project]).complete()?;
    let held = only_global(&(source, &req.agents), names, (&global, &project));
    if held.is_empty() {
        return Ok(true);
    }
    ctx.prompter.confirm(&format!(
        "Already installed globally: {}. Also install into this project?",
        held.join(", ")
    ))
}

/// The names that are installed globally but not in the project.
fn only_global(
    (source, agents): &(&str, &[skillz_harness::model::Agent]),
    names: &[String],
    (global, project): (&[Installed], &[Installed]),
) -> Vec<String> {
    names
        .iter()
        .filter(|name| {
            let entry = Spec::Skill {
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
    use skillz_harness::model::{Agent, Installer, Kind};

    fn skill(name: &str, scope: Scope) -> Installed {
        Installed {
            details: Default::default(),
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
    #[test]
    fn global_install_neither_reads_project_inventory_nor_asks_about_it() {
        let temporary = tempfile::tempdir().unwrap();
        let env = skillz_harness::env::Env::new(
            temporary.path().to_path_buf(),
            temporary.path().join("project"),
        );
        let invalid = skillz_harness::agents::claude::dir(&env).join("skills");
        std::fs::create_dir_all(invalid.parent().unwrap()).unwrap();
        std::fs::write(invalid, "not a directory").unwrap();
        let adapters = skillz_harness::agents::registry();
        let prompter = crate::install::test_support::ScriptedPrompter::new(&[], false);
        let request = InstallRequest {
            source: "o/r".into(),
            cwd: env.project.clone(),
            agents: vec![Agent::Claude],
            scope: Scope::Global,
            selection: super::super::SkillSelection::All,
        };
        let context = Ctx {
            adapters: &adapters,
            env: &env,
            runner: &skillz_harness::runner::NoCommands,
            prompter: &prompter,
        };

        assert!(confirm_duplicates(&context, &request, ("o/r", &names(&["tdd"]))).unwrap());
    }
}
