//! Every value of an entry ends up as a command-line argument, so none may be empty or
//! option-like.

use crate::harness::Spec;
use crate::harness::redact::credential_count;

impl Spec {
    pub fn validate(&self) -> anyhow::Result<()> {
        let args = match self {
            Spec::Skill { source, names, .. } | Spec::Subagent { source, names, .. } => {
                sourced_args(source, names)?
            }
            Spec::Plugin {
                source,
                marketplace,
                name,
                ..
            } => vec![source.as_str(), marketplace, name],
            Spec::Hook { event, command, .. } => hook_args(event, command)?,
            Spec::Mcp { .. } => mcp_args(self)?,
        };
        ensure_plain(&args)
    }
}

impl Spec {
    /// Refuse an entry typed by hand whose command line or url carries a credential: a library
    /// or profile file is plain text and is shared.
    pub fn ensure_no_credentials(&self) -> anyhow::Result<()> {
        let words = match self {
            Spec::Hook { command, .. } => shell_words::split(command)
                .unwrap_or_else(|_| command.split_whitespace().map(str::to_owned).collect()),
            Spec::Mcp {
                command, args, url, ..
            } => {
                let mut words = args.clone();
                words.extend(url.clone());
                if let Some(command) = command {
                    words.extend(shell_words::split(command).unwrap_or_else(|_| {
                        command.split_whitespace().map(str::to_owned).collect()
                    }));
                }
                words
            }
            Spec::Skill { source, .. }
            | Spec::Subagent { source, .. }
            | Spec::Plugin { source, .. } => vec![source.clone()],
        };
        anyhow::ensure!(
            credential_count(&words) == 0,
            "entry carries a credential; keep secrets out of the library"
        );
        Ok(())
    }
}

fn hook_args<'a>(event: &'a str, command: &'a str) -> anyhow::Result<Vec<&'a str>> {
    anyhow::ensure!(
        !event.contains(char::is_whitespace),
        "hook event must be one word"
    );
    Ok(vec![event, command])
}

fn sourced_args<'a>(source: &'a str, names: &'a [String]) -> anyhow::Result<Vec<&'a str>> {
    anyhow::ensure!(!names.is_empty(), "entry names nothing");
    Ok(std::iter::once(source)
        .chain(names.iter().map(String::as_str))
        .collect())
}

/// The arguments of an MCP entry, after checking it has one way to launch and a known transport.
fn mcp_args(entry: &Spec) -> anyhow::Result<Vec<&str>> {
    let Spec::Mcp {
        name,
        command,
        url,
        transport,
        ..
    } = entry
    else {
        return Ok(vec![]);
    };
    anyhow::ensure!(
        command.is_some() != url.is_some(),
        "mcp entry needs exactly one of command or url"
    );
    anyhow::ensure!(
        transport.is_none() || url.is_some(),
        "mcp transport only goes with a url"
    );
    if let Some(t) = transport {
        anyhow::ensure!(
            matches!(t.as_str(), "http" | "sse"),
            "unknown mcp transport; expected http or sse"
        );
    }
    Ok(std::iter::once(name.as_str())
        .chain(command.as_deref())
        .chain(url.as_deref())
        .collect())
}

fn ensure_plain(args: &[&str]) -> anyhow::Result<()> {
    for arg in args {
        anyhow::ensure!(
            !arg.trim().is_empty() && !arg.starts_with('-'),
            "entry contains an empty or option-like argument"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp(command: Option<&str>, url: Option<&str>, transport: Option<&str>) -> Spec {
        Spec::Mcp {
            name: "s".into(),
            command: command.map(String::from),
            args: vec![],
            url: url.map(String::from),
            transport: transport.map(String::from),
            agents: vec![],
        }
    }

    fn skill(source: &str, names: &[&str]) -> Spec {
        Spec::Skill {
            source: source.into(),
            names: names.iter().map(|n| n.to_string()).collect(),
            agents: vec![],
        }
    }

    #[test]
    fn an_entry_naming_nothing_or_an_option_is_rejected() {
        let bad = [
            skill("o/r", &[]),
            skill("-g", &["a"]),
            skill("o/r", &["--all"]),
            skill("", &["a"]),
        ];

        for entry in bad {
            assert!(entry.validate().is_err(), "{entry}");
        }
    }

    #[test]
    fn an_mcp_server_has_exactly_one_launch_and_a_known_transport() {
        let bad = [
            mcp(None, None, None),
            mcp(Some("a"), Some("b"), None),
            mcp(Some("a"), None, Some("http")),
            mcp(None, Some("b"), Some("ws")),
        ];
        let good = [
            mcp(Some("a"), None, None),
            mcp(None, Some("b"), Some("sse")),
        ];

        assert!(bad.iter().all(|e| e.validate().is_err()));
        assert!(good.iter().all(|e| e.validate().is_ok()));
    }

    #[test]
    fn a_hook_or_mcp_line_carrying_a_credential_is_refused() {
        let hook = Spec::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "notify --token abc".into(),
            agents: vec![],
        };
        let mcp = Spec::Mcp {
            name: "s".into(),
            command: Some("npx".into()),
            args: vec!["--api-key=abc".into()],
            url: None,
            transport: None,
            agents: vec![],
        };

        assert!(hook.ensure_no_credentials().is_err());
        assert!(mcp.ensure_no_credentials().is_err());
        assert!(skill("o/r", &["a"]).ensure_no_credentials().is_ok());
    }

    #[test]
    fn a_plugin_needs_all_three_names() {
        let plugin = Spec::Plugin {
            source: "o/r".into(),
            marketplace: "".into(),
            name: "p".into(),
            agents: vec![],
        };

        assert!(plugin.validate().is_err());
    }
    #[test]
    fn credential_rejection_never_echoes_a_hook_token_or_server_url() {
        for spec in [
            "hook Stop notify --token validation-secret",
            "mcp s https://user:validation-secret@host/mcp",
            "mcp s https://host/mcp?key=validation-secret",
        ] {
            let error = Spec::parse_spec(spec).unwrap_err();
            assert!(!format!("{error:#}").contains("validation-secret"));
        }
    }

    #[test]
    fn invalid_fields_never_echo_other_credentials_in_the_entry() {
        let entries = [
            skill("https://user:validation-secret@host/repo", &[]),
            skill("https://user:validation-secret@host/repo", &["--all"]),
            Spec::Hook {
                event: "invalid event".into(),
                command: "notify --token validation-secret".into(),
                matcher: None,
                agents: vec![],
            },
            Spec::Hook {
                event: "--invalid".into(),
                command: "notify --token validation-secret".into(),
                matcher: None,
                agents: vec![],
            },
            mcp(
                Some("srv"),
                Some("https://user:validation-secret@host/mcp"),
                None,
            ),
            mcp(None, Some("https://host/mcp"), Some("validation-secret")),
        ];
        for entry in entries {
            let error = entry.validate().unwrap_err();
            assert!(!format!("{error:#}").contains("validation-secret"));
        }
    }
    #[test]
    fn imported_profiles_refuse_credentials_in_commands_urls_and_repository_sources() {
        for entry in [
            Spec::Hook {
                event: "Stop".into(),
                command: "notify '--token' 'profile-secret'".into(),
                matcher: None,
                agents: vec![],
            },
            mcp(None, Some("https://user:profile-secret@host/mcp"), None),
            skill("https://user:profile-secret@host/repo.git", &["a"]),
            Spec::Subagent {
                source: "https://host/repo.git?token=profile-secret".into(),
                names: vec!["a".into()],
                agents: vec![],
            },
            Spec::Plugin {
                source: "https://user:profile-secret@host/repo.git".into(),
                marketplace: "mp".into(),
                name: "a".into(),
                agents: vec![],
            },
        ] {
            let mut profile = crate::profile::Profile::new("imported");
            profile.entries = vec![entry];
            let error = crate::profile::Profile::parse("imported", &profile.to_text().unwrap())
                .unwrap_err();
            assert!(!format!("{error:#}").contains("profile-secret"));
        }
    }
}
