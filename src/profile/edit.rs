use super::same_thing;
use crate::harness::Spec;

/// `entries` followed by those of `added` that name nothing `entries` already name.
pub fn with_added(entries: &[Spec], added: Vec<Spec>) -> Vec<Spec> {
    added.into_iter().fold(entries.to_vec(), |mut all, entry| {
        if !all.iter().any(|have| same_thing(have, &entry)) {
            all.push(entry);
        }
        all
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str) -> Spec {
        Spec::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![],
        }
    }

    #[test]
    fn what_the_profile_has_is_not_added_twice() {
        let entries = [skill("a")];

        let all = with_added(&entries, vec![skill("a"), skill("b"), skill("b")]);

        assert_eq!(all, [skill("a"), skill("b")]);
    }
}
