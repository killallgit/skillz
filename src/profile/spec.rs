//! An entry typed by hand in one line.

use super::Spec;

impl Spec {
    /// `skill SOURCE NAME[,NAME...]`, `subagent SOURCE NAME[,NAME...]`,
    /// `plugin SOURCE MARKETPLACE NAME`, `hook EVENT COMMAND...` or `mcp NAME COMMAND|URL [ARGS...]`. It uses the
    /// profile's agents.
    pub fn parse_spec(spec: &str) -> anyhow::Result<Spec> {
        let words: Vec<&str> = spec.split_whitespace().collect();
        let entry = match words.as_slice() {
            ["skill" | "skills", source, names] => Spec::Skill {
                source: source.to_string(),
                names: comma_list(names),
                agents: vec![],
            },
            ["subagent" | "subagents", source, names] => Spec::Subagent {
                source: source.to_string(),
                names: comma_list(names),
                agents: vec![],
            },
            ["plugin", source, marketplace, name] => Spec::Plugin {
                source: source.to_string(),
                marketplace: marketplace.to_string(),
                name: name.to_string(),
                agents: vec![],
            },
            ["hook", event, command @ ..] if !command.is_empty() => Spec::Hook {
                event: event.to_string(),
                matcher: None,
                command: command.join(" "),
                agents: vec![],
            },
            ["mcp", name, target, rest @ ..] => mcp_entry(name, target, rest),
            _ => anyhow::bail!(
                "expected `skill SOURCE NAMES`, `subagent SOURCE NAMES`, `plugin SOURCE MARKETPLACE NAME`, `hook EVENT COMMAND` or `mcp NAME COMMAND|URL [ARGS]`"
            ),
        };
        entry.validate()?;
        entry.ensure_no_credentials()?;
        Ok(entry)
    }
}

fn comma_list(names: &str) -> Vec<String> {
    names
        .split(',')
        .filter(|n| !n.is_empty())
        .map(String::from)
        .collect()
}

fn mcp_entry(name: &str, target: &str, rest: &[&str]) -> Spec {
    let is_url = target.starts_with("http://") || target.starts_with("https://");
    Spec::Mcp {
        name: name.to_string(),
        command: (!is_url).then(|| target.to_string()),
        args: if is_url {
            vec![]
        } else {
            rest.iter().map(|a| a.to_string()).collect()
        },
        url: is_url.then(|| target.to_string()),
        transport: None,
        agents: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_line_names_the_source_and_its_skills() {
        let entry = Spec::parse_spec("skill o/r tdd,grill").unwrap();

        assert_eq!(
            entry,
            Spec::Skill {
                source: "o/r".into(),
                names: vec!["tdd".into(), "grill".into()],
                agents: vec![]
            }
        );
    }

    #[test]
    fn a_plugin_line_is_trimmed() {
        let entry = Spec::parse_spec(" plugin o/mp mp pp ").unwrap();

        assert!(matches!(entry, Spec::Plugin { .. }));
    }

    #[test]
    fn an_mcp_line_with_a_command_keeps_its_args() {
        let Spec::Mcp {
            command, args, url, ..
        } = Spec::parse_spec("mcp s npx -y pkg").unwrap()
        else {
            panic!("not an mcp entry");
        };

        assert_eq!(command.as_deref(), Some("npx"));
        assert_eq!(args, ["-y", "pkg"]);
        assert_eq!(url, None);
    }

    #[test]
    fn an_mcp_line_with_a_url_has_no_command() {
        let Spec::Mcp { command, url, .. } = Spec::parse_spec("mcp h https://x/mcp").unwrap()
        else {
            panic!("not an mcp entry");
        };

        assert_eq!((command, url.as_deref()), (None, Some("https://x/mcp")));
    }

    #[test]
    fn lines_that_name_nothing_usable_are_rejected() {
        let bad = [
            "",
            "skill o/r",
            "skill o/r ,",
            "skill -g a",
            "plugin o/mp mp",
            "hook x",
            "hook Stop run --token abc",
            "mcp s npx --api-key=abc",
        ];

        for line in bad {
            assert!(Spec::parse_spec(line).is_err(), "{line:?}");
        }
    }
}
