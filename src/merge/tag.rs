use crate::harness::model::Agent;

/// Agents listed by name up to this many; more are abbreviated to the first two and a count.
const SHOWN_IN_FULL: usize = 3;
const SHOWN_WHEN_ABBREVIATED: usize = 2;

/// `claude, codex`, or `claude, codex +2` when there are many.
pub fn tag(agents: &[Agent]) -> String {
    let shown = match agents.len() > SHOWN_IN_FULL {
        true => SHOWN_WHEN_ABBREVIATED,
        false => agents.len(),
    };
    let names: Vec<String> = agents[..shown].iter().map(Agent::to_string).collect();
    match agents.len() - shown {
        0 => names.join(", "),
        more => format!("{} +{more}", names.join(", ")),
    }
}
