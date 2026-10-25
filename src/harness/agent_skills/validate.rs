use super::Frontmatter;

const NAME_MAX: usize = 64;
const DESCRIPTION_MAX: usize = 1024;
const COMPATIBILITY_MAX: usize = 500;

/// A way a skill breaks the Agent Skills specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Violation {
    MissingName,
    NameTooLong,
    /// Anything but lowercase letters, digits and hyphens.
    NameCharacters,
    /// A leading, trailing or doubled hyphen.
    NameHyphens,
    /// The name differs from the directory holding the skill.
    NameIsNotTheDirectory,
    MissingDescription,
    DescriptionTooLong,
    CompatibilityTooLong,
}

/// Every way `front` breaks the spec, for a skill in a directory called `directory`.
pub fn validate(front: &Frontmatter, directory: &str) -> Vec<Violation> {
    let mut found = Vec::new();
    match front.name.as_deref() {
        None => found.push(Violation::MissingName),
        Some(name) => {
            if name.chars().count() > NAME_MAX {
                found.push(Violation::NameTooLong);
            }
            if !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                found.push(Violation::NameCharacters);
            }
            if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
                found.push(Violation::NameHyphens);
            }
            if name != directory {
                found.push(Violation::NameIsNotTheDirectory);
            }
        }
    }
    match front.description.as_deref() {
        None => found.push(Violation::MissingDescription),
        Some(d) if d.chars().count() > DESCRIPTION_MAX => found.push(Violation::DescriptionTooLong),
        Some(_) => {}
    }
    if front
        .compatibility
        .as_deref()
        .is_some_and(|c| c.chars().count() > COMPATIBILITY_MAX)
    {
        found.push(Violation::CompatibilityTooLong);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;

    fn violations(front: &str, directory: &str) -> Vec<Violation> {
        validate(&parse(&format!("---\n{front}\n---\n")).unwrap(), directory)
    }

    #[test]
    fn a_valid_skill_has_no_violations() {
        assert_eq!(
            violations("name: pdf-tools\ndescription: d", "pdf-tools"),
            []
        );
    }

    #[test]
    fn each_rule_is_reported_on_its_own() {
        assert_eq!(violations("description: d", "x"), [Violation::MissingName]);
        assert_eq!(
            violations("name: Pdf\ndescription: d", "Pdf"),
            [Violation::NameCharacters]
        );
        assert_eq!(
            violations("name: -a--b-\ndescription: d", "-a--b-"),
            [Violation::NameHyphens]
        );
        assert_eq!(
            violations("name: a\ndescription: d", "b"),
            [Violation::NameIsNotTheDirectory]
        );
        assert_eq!(violations("name: a", "a"), [Violation::MissingDescription]);
    }

    #[test]
    fn length_limits_apply_to_name_description_and_compatibility() {
        let name = "a".repeat(65);
        let long = "d".repeat(1025);
        let compat = "c".repeat(501);

        assert_eq!(
            violations(&format!("name: {name}\ndescription: d"), &name),
            [Violation::NameTooLong]
        );
        assert_eq!(
            violations(&format!("name: a\ndescription: {long}"), "a"),
            [Violation::DescriptionTooLong]
        );
        assert_eq!(
            violations(
                &format!("name: a\ndescription: d\ncompatibility: {compat}"),
                "a"
            ),
            [Violation::CompatibilityTooLong]
        );
    }
}
