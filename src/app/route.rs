//! Which installer each entry of a batch goes through. Each box the user checks runs its own
//! method: an agent runs its own plugin installer, `npx` runs `npx skills` once for the whole
//! source. A skill is only ever installed by `npx`, so unchecking it leaves skills out rather
//! than doing them a second time next to a plugin.

use skillz_harness::Spec;
use skillz_harness::model::Agent;

/// `npx skills` takes every skill the source has.
const EVERY_SKILL: &str = "*";

/// Whether `npx` starts checked: some skill has no plugin of the same source to install it, so
/// without `npx` nothing would.
pub fn npx_by_default(entries: &[Spec]) -> bool {
    entries.iter().any(|entry| match entry {
        Spec::Skill { source, .. } => !entries.iter().any(|other| plugin_from(other, source)),
        _ => false,
    })
}

fn plugin_from(entry: &Spec, from: &str) -> bool {
    matches!(entry, Spec::Plugin { source, .. } if source == from)
}

/// `entries` as the checked installers would run them. Plugins and the other kinds keep to the
/// checked agents, an entry that names its own agents to those. With `npx`, each source gets
/// one install of all its skills in place of its separate skill entries, for the checked agents
/// (every agent when none is checked).
pub fn routed(entries: Vec<Spec>, (checked, every): (&[Agent], &[Agent]), npx: bool) -> Vec<Spec> {
    let mut sources: Vec<String> = vec![];
    let mut routed: Vec<Spec> = vec![];
    for entry in entries {
        if let Spec::Skill { source, .. } | Spec::Plugin { source, .. } = &entry
            && npx
            && !sources.contains(source)
        {
            sources.push(source.clone());
        }
        if matches!(entry, Spec::Skill { .. }) {
            continue;
        }
        if let Some(entry) = narrowed(entry, checked) {
            routed.push(entry);
        }
    }
    let targets = match checked.is_empty() {
        true => every,
        false => checked,
    };
    routed.extend(sources.into_iter().map(|source| Spec::Skill {
        source,
        names: vec![EVERY_SKILL.into()],
        agents: targets.to_vec(),
    }));
    routed
}

/// `entry` for the `chosen` agents it names, or all of them when it names none; `None` when none
/// of its own are chosen.
fn narrowed(entry: Spec, chosen: &[Agent]) -> Option<Spec> {
    if entry.agents().is_empty() {
        return (!chosen.is_empty()).then(|| entry.with_agents(chosen.to_vec()));
    }
    let own: Vec<Agent> = entry
        .agents()
        .iter()
        .copied()
        .filter(|agent| chosen.contains(agent))
        .collect();
    (!own.is_empty()).then(|| entry.with_agents(own))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOTH: [Agent; 2] = [Agent::Claude, Agent::Codex];

    fn skill(source: &str, name: &str) -> Spec {
        Spec::Skill {
            source: source.into(),
            names: vec![name.into()],
            agents: vec![],
        }
    }

    fn plugin(source: &str, name: &str, agents: Vec<Agent>) -> Spec {
        Spec::Plugin {
            source: source.into(),
            marketplace: "mp".into(),
            name: name.into(),
            agents,
        }
    }

    fn all_skills(source: &str, agents: &[Agent]) -> Spec {
        Spec::Skill {
            source: source.into(),
            names: vec!["*".into()],
            agents: agents.to_vec(),
        }
    }

    #[test]
    fn skills_without_a_plugin_start_with_npx_checked_and_the_rest_without() {
        let loose = [skill("o/r", "a"), skill("o/r", "b")];
        let both = [skill("o/r", "a"), plugin("o/r", "p", vec![])];
        let elsewhere = [skill("o/r", "a"), plugin("x/y", "p", vec![])];

        assert!(npx_by_default(&loose));
        assert!(!npx_by_default(&both));
        assert!(npx_by_default(&elsewhere));
        assert!(!npx_by_default(&[plugin("o/r", "p", vec![])]));
    }

    #[test]
    fn without_npx_a_package_installs_its_plugin_and_not_its_skills_a_second_time() {
        let entries = vec![skill("o/r", "a"), plugin("o/r", "p", vec![])];

        let routed = routed(entries, (&BOTH, &BOTH), false);

        assert_eq!(routed, [plugin("o/r", "p", BOTH.to_vec())]);
    }

    #[test]
    fn with_npx_each_source_installs_all_its_skills_once() {
        let entries = vec![
            skill("o/r", "a"),
            skill("o/r", "b"),
            skill("o/r", "c"),
            skill("x/y", "d"),
        ];

        let routed = routed(entries, (&BOTH, &BOTH), true);

        assert_eq!(routed, [all_skills("o/r", &BOTH), all_skills("x/y", &BOTH)]);
    }

    #[test]
    fn with_everything_checked_each_installer_runs() {
        let entries = vec![skill("o/r", "a"), plugin("o/r", "p", vec![])];

        let routed = routed(entries, (&BOTH, &BOTH), true);

        assert_eq!(
            routed,
            [plugin("o/r", "p", BOTH.to_vec()), all_skills("o/r", &BOTH)]
        );
    }

    #[test]
    fn an_unchecked_agent_gets_no_plugin_and_no_skills() {
        let entries = vec![skill("o/r", "a"), plugin("o/r", "p", vec![])];

        let routed = routed(entries, (&[Agent::Codex], &BOTH), true);

        assert_eq!(
            routed,
            [
                plugin("o/r", "p", vec![Agent::Codex]),
                all_skills("o/r", &[Agent::Codex])
            ]
        );
    }

    #[test]
    fn a_plugin_that_names_other_agents_than_the_checked_ones_is_dropped() {
        let entries = vec![plugin("o/r", "p", vec![Agent::Claude])];

        let routed = routed(entries, (&[Agent::Codex], &BOTH), false);

        assert!(routed.is_empty());
    }

    #[test]
    fn npx_with_no_agent_checked_reaches_every_agent_and_nothing_else_runs() {
        let entries = vec![skill("o/r", "a"), plugin("o/r", "p", vec![])];

        let routed = routed(entries, (&[], &BOTH), true);

        assert_eq!(routed, [all_skills("o/r", &BOTH)]);
    }

    #[test]
    fn with_nothing_checked_nothing_is_installed() {
        let entries = vec![skill("o/r", "a"), plugin("o/r", "p", vec![])];

        assert!(routed(entries, (&[], &BOTH), false).is_empty());
    }
}
