use std::collections::{BTreeSet, HashMap};

use crate::app::install::{NativeSkills, native_skills};
use crate::app::profile::Profile;
use skillz_harness::Spec;
use skillz_harness::action::Action;
use skillz_harness::clone;
use skillz_harness::model::Agent;
use skillz_harness::provision::{self, Unsupported};

use super::Applier;
use super::standing::compare;
use super::switch::Switch;

impl Applier<'_> {
    /// What the scope needs to hold `target`: what is missing gets installed and, when `exact`,
    /// what the profile does not want gets removed. Reads source locations and the installed
    /// inventory, and fetches the sources of missing skills.
    pub fn plan(&self, target: &Profile, exact: bool) -> anyhow::Result<Switch> {
        let target = target.clone().in_env(self.env);
        let standing = compare(&target.resolved(), &self.installed()?);
        let (installs, unsupported) = self.installs_for(&standing.missing);
        let extras = if exact { standing.extras } else { vec![] };
        Ok(Switch {
            to: target.name.clone(),
            present: standing.present,
            unsourced: extras
                .iter()
                .filter(|e| !e.sourced)
                .map(|e| e.item.id.clone())
                .collect(),
            removals: extras.into_iter().map(|e| e.item).collect(),
            installs,
            unsupported,
            updates: vec![],
        })
    }

    /// The install steps for each entry, and what some agent cannot take. A skill's agents whose
    /// source marketplace supplies it install the owning plugins; the others use `npx skills`.
    fn installs_for(&self, entries: &[Spec]) -> (Vec<(Spec, Vec<Action>)>, Vec<Unsupported>) {
        let mut native = self.native_skills_by_entry(entries);
        let mut installs = Vec::new();
        let mut unsupported = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let mut steps = Vec::new();
            let through_npx = match native.remove(&index) {
                Some(split) if !split.native.is_empty() => {
                    steps.extend(split.native.into_iter().flat_map(|(_, steps)| steps));
                    (!split.rest.is_empty()).then(|| entry.clone().with_agents(split.rest))
                }
                _ => Some(entry.clone()),
            };
            if let Some(through_npx) = through_npx {
                let planned = provision::plan_entries(
                    self.adapters,
                    std::slice::from_ref(&through_npx),
                    self.scope,
                    self.env,
                );
                unsupported.extend(planned.unsupported);
                steps.extend(planned.steps);
            }
            if !steps.is_empty() {
                installs.push((entry.clone(), steps));
            }
        }
        (installs, unsupported)
    }

    /// How each skill entry divides between agents installing its owning plugins and agents
    /// keeping `npx skills`, by its index in `entries`. Each source is fetched once; an entry whose
    /// source cannot be fetched or read is left out, so all its agents keep `npx skills`.
    fn native_skills_by_entry(&self, entries: &[Spec]) -> HashMap<usize, NativeSkills> {
        let skills: Vec<(usize, &str, &[String], &[Agent])> = entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| match entry {
                Spec::Skill {
                    source,
                    names,
                    agents,
                } => Some((index, source.as_str(), names.as_slice(), agents.as_slice())),
                _ => None,
            })
            .collect();
        let sources: BTreeSet<&str> = skills.iter().map(|&(_, source, ..)| source).collect();
        let mut split = HashMap::new();
        for source in sources {
            let fetched =
                clone::with_source(source, &self.env.project, self.runner, |src, resolved| {
                    let from_source = skills.iter().filter(|&&(_, from, ..)| from == source);
                    Ok(from_source
                        .filter_map(|&(index, _, names, agents)| {
                            let native = native_skills(
                                self.adapters,
                                self.env,
                                self.scope,
                                (src, resolved),
                                names,
                                agents,
                            )
                            .ok()?;
                            Some((index, native))
                        })
                        .collect::<Vec<_>>())
                });
            split.extend(fetched.unwrap_or_default());
        }
        split
    }
}
