use super::Reason;
use crate::app::profile::SpecChecks;
use skillz_harness::Spec;
use skillz_harness::mcp::Launch;
use skillz_harness::model::{Details, Installed, Installer, Kind, split_plugin_id};

/// The validated entries this installed copy can reproduce, with its agents filled in.
/// Marketplaces belong to their plugins and produce no entry of their own.
pub fn entries(item: &Installed) -> Result<Vec<Spec>, Reason> {
    let mut agents = item.agents.clone();
    agents.sort();
    agents.dedup();
    let entries = match item.kind {
        Kind::Marketplace => Vec::new(),
        Kind::Skill => {
            let source = match item.installer {
                Installer::Npx => item.source.as_ref(),
                Installer::Local => item.origin.link.as_ref(),
                _ => None,
            }
            .ok_or(Reason::NoSource)?;
            let name = match &item.details {
                Details::Skill { install_name, .. } => install_name,
                _ => &item.name,
            };
            vec![Spec::Skill {
                source: source.clone(),
                names: vec![name.clone()],
                agents,
            }]
        }
        Kind::Plugin => {
            let source = item.source.as_ref().ok_or(Reason::NoSource)?;
            let (name, marketplace) = split_plugin_id(&item.name);
            vec![Spec::Plugin {
                source: source.clone(),
                marketplace: marketplace.into(),
                name: name.into(),
                agents,
            }]
        }
        Kind::Subagent => {
            let source = item
                .source
                .as_ref()
                .or(item.origin.link.as_ref())
                .ok_or(Reason::NoSource)?;
            let Details::Subagent { file } = &item.details else {
                return Err(Reason::Unreadable);
            };
            let name = file
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or(Reason::Unreadable)?;
            vec![Spec::Subagent {
                source: source.clone(),
                names: vec![name.into()],
                agents,
            }]
        }
        Kind::Hook => hook_entries(item)?,
        Kind::Mcp => {
            let Details::Mcp { server, .. } = &item.details else {
                return Err(Reason::Unreadable);
            };
            let secrets = server.secret_count();
            if secrets > 0 {
                return Err(Reason::Secrets(secrets));
            }
            vec![server_entry(
                server.name.clone(),
                server.launch.clone(),
                agents,
            )]
        }
    };
    if entries
        .iter()
        .any(|entry| entry.validate().is_err() || entry.ensure_no_credentials().is_err())
    {
        return Err(Reason::Invalid);
    }
    Ok(entries)
}

pub(super) fn server_entry(
    name: String,
    launch: Launch,
    agents: Vec<skillz_harness::model::Agent>,
) -> Spec {
    match launch {
        Launch::Stdio { command, args } => Spec::Mcp {
            name,
            command: Some(command),
            args,
            url: None,
            transport: None,
            agents,
        },
        Launch::Url { url, transport } => Spec::Mcp {
            name,
            command: None,
            args: vec![],
            url: Some(url),
            transport: (transport != "http").then_some(transport),
            agents,
        },
    }
}

/// One entry per handler, for the agents whose own copy holds it: a merged hook row can carry
/// each agent's own command line.
fn hook_entries(item: &Installed) -> Result<Vec<Spec>, Reason> {
    let mut handlers: Vec<Spec> = Vec::new();
    for part in item.parts() {
        for hook in &part.hooks {
            let held = handlers.iter_mut().find(|spec| {
                matches!(spec, Spec::Hook { event, matcher, command, .. }
                    if *event == hook.event && *matcher == hook.matcher && *command == hook.command)
            });
            match held {
                Some(Spec::Hook { agents, .. }) => agents.extend(part.agents.iter().copied()),
                _ => handlers.push(Spec::Hook {
                    event: hook.event.clone(),
                    matcher: hook.matcher.clone(),
                    command: hook.command.clone(),
                    agents: part.agents.clone(),
                }),
            }
        }
    }
    match handlers.is_empty() {
        true => Err(Reason::Unreadable),
        false => Ok(handlers),
    }
}
