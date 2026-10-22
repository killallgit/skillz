use crate::model::Agent;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_to_three_agents_are_all_named() {
        let tag = tag(&[Agent::Claude, Agent::Codex, Agent::Cursor]);

        assert_eq!(tag, "claude, codex, cursor");
    }

    #[test]
    fn more_than_three_are_cut_to_two_and_a_count() {
        let many = [Agent::Claude, Agent::Codex, Agent::Cursor, Agent::Claude];

        let tag = tag(&many);

        assert_eq!(tag, "claude, codex +2");
    }
}
