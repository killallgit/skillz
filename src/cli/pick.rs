//! Choosing among the things a query matches: an exact label wins, else the fuzzy matches; one
//! match is taken as it is, several are offered to the user, and with `--yes` they are an error.

use anyhow::bail;

use crate::app::matcher::fuzzy_rank;
use crate::app::prompt::Prompter;

pub(super) struct Choosing<'a> {
    pub prompter: &'a dyn Prompter,
    pub title: &'a str,
    pub yes: bool,
}

pub(super) fn pick<'a, T>(
    choosing: &Choosing,
    query: &str,
    items: &'a [T],
    label: impl Fn(&T) -> String,
) -> anyhow::Result<Vec<&'a T>> {
    let labels: Vec<String> = items.iter().map(&label).collect();
    let found = matching(query, &labels);
    match (found.len(), choosing.yes) {
        (0, _) => bail!("nothing matches {query:?}"),
        (1, _) => Ok(vec![&items[found[0]]]),
        (n, true) => bail!("{n} items match {query:?}; refine the query or run without --yes"),
        _ => {
            let options: Vec<String> = found.iter().map(|&i| labels[i].clone()).collect();
            let chosen = choosing.prompter.multi_select(choosing.title, &options)?;
            Ok(chosen.into_iter().map(|c| &items[found[c]]).collect())
        }
    }
}

fn matching(query: &str, labels: &[String]) -> Vec<usize> {
    let exact: Vec<usize> = (0..labels.len())
        .filter(|&i| labels[i].eq_ignore_ascii_case(query.trim()))
        .collect();
    if exact.is_empty() {
        fuzzy_rank(query, labels)
    } else {
        exact
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::prompt::AssumeYes;

    fn names() -> Vec<String> {
        ["tdd", "tdd-extra", "grill"].map(String::from).to_vec()
    }

    fn choosing(yes: bool) -> Choosing<'static> {
        Choosing {
            prompter: &AssumeYes,
            title: "pick",
            yes,
        }
    }

    #[test]
    fn an_exact_label_beats_longer_labels_that_contain_it() {
        let items = names();

        let picked = pick(&choosing(true), "tdd", &items, Clone::clone).unwrap();

        assert_eq!(picked, [&items[0]]);
    }

    #[test]
    fn several_matches_are_an_error_with_yes_and_offered_without() {
        let items = names();

        let with_yes = pick(&choosing(true), "t", &items, Clone::clone);
        let offered = pick(&choosing(false), "t", &items, Clone::clone).unwrap();

        assert!(with_yes.is_err());
        assert_eq!(offered.len(), 2);
    }

    #[test]
    fn nothing_matching_is_an_error() {
        let items = names();

        assert!(pick(&choosing(false), "zzz", &items, Clone::clone).is_err());
    }
}
