//! Every value of an entry ends up as a command-line argument, so none may be empty or
//! option-like.

use super::Entry;

impl Entry {
    pub fn validate(&self) -> anyhow::Result<()> {
        let args = match self {
            Entry::Skill { source, names, .. } | Entry::Subagent { source, names, .. } => {
                sourced_args(source, names)?
            }
            Entry::Plugin {
                source,
                marketplace,
                name,
                ..
            } => vec![source.as_str(), marketplace, name],
            Entry::Hook { event, command, .. } => hook_args(event, command)?,
            Entry::Mcp { .. } => mcp_args(self)?,
        };
        ensure_plain(&args, self)
    }
}

fn hook_args<'a>(event: &'a str, command: &'a str) -> anyhow::Result<Vec<&'a str>> {
    anyhow::ensure!(
        !event.contains(char::is_whitespace),
        "invalid hook event {event:?}"
    );
    Ok(vec![event, command])
}

fn sourced_args<'a>(source: &'a str, names: &'a [String]) -> anyhow::Result<Vec<&'a str>> {
    anyhow::ensure!(!names.is_empty(), "entry for {source} names nothing");
    Ok(std::iter::once(source)
        .chain(names.iter().map(String::as_str))
        .collect())
}

/// The arguments of an MCP entry, after checking it has one way to launch and a known transport.
fn mcp_args(entry: &Entry) -> anyhow::Result<Vec<&str>> {
    let Entry::Mcp {
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
        "mcp entry {name} needs exactly one of command or url"
    );
    anyhow::ensure!(
        transport.is_none() || url.is_some(),
        "mcp entry {name}: transport only goes with a url"
    );
    if let Some(t) = transport {
        anyhow::ensure!(
            matches!(t.as_str(), "http" | "sse"),
            "unknown transport {t:?}"
        );
    }
    Ok(std::iter::once(name.as_str())
        .chain(command.as_deref())
        .chain(url.as_deref())
        .collect())
}

fn ensure_plain(args: &[&str], entry: &Entry) -> anyhow::Result<()> {
    for arg in args {
        anyhow::ensure!(
            !arg.trim().is_empty() && !arg.starts_with('-'),
            "invalid value {arg:?} in {entry}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp(command: Option<&str>, url: Option<&str>, transport: Option<&str>) -> Entry {
        Entry::Mcp {
            name: "s".into(),
            command: command.map(String::from),
            args: vec![],
            url: url.map(String::from),
            transport: transport.map(String::from),
            agents: vec![],
        }
    }

    fn skill(source: &str, names: &[&str]) -> Entry {
        Entry::Skill {
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
    fn a_plugin_needs_all_three_names() {
        let plugin = Entry::Plugin {
            source: "o/r".into(),
            marketplace: "".into(),
            name: "p".into(),
            agents: vec![],
        };

        assert!(plugin.validate().is_err());
    }
}
