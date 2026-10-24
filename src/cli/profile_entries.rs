//! `profile add` and `profile remove`: change which entries a profile has, as the workspace's
//! build picker does: entries typed or copied from the library, kept or taken out.

use super::pick::{Choosing, pick};
use super::profile_args::{ProfileAddArgs, ProfileRemoveArgs};
use super::session::{Console, Session};
use crate::profile::{Spec, with_added};

pub(super) fn add(
    session: &Session,
    console: &Console,
    args: &ProfileAddArgs,
) -> anyhow::Result<()> {
    let have = session.profiles().load(&args.name)?.entries;
    let mut wanted = Vec::new();
    if let Some(spec) = &args.spec {
        wanted.push(Spec::parse_spec(spec)?);
    }
    wanted.extend(from_library(session, console, args)?);
    let entries = with_added(&have, wanted);
    let added = &entries[have.len()..];
    if args.dry_run {
        return added
            .iter()
            .try_for_each(|entry| console.say(&format!("would add {entry}")));
    }
    console.conclude(Some(session.set_entries(&args.name, &entries)?))
}

fn from_library(
    session: &Session,
    console: &Console,
    args: &ProfileAddArgs,
) -> anyhow::Result<Vec<Spec>> {
    let library = session.library().read()?.items;
    let choosing = Choosing {
        prompter: console,
        title: "Select items to add",
        yes: args.yes,
    };
    let mut copies = Vec::new();
    for query in &args.from_library {
        let picked = pick(&choosing, query, library.items(), |i| i.label.clone())?;
        copies.extend(picked.into_iter().map(|item| item.entry.clone()));
    }
    Ok(copies)
}

pub(super) fn remove(
    session: &Session,
    console: &Console,
    args: &ProfileRemoveArgs,
) -> anyhow::Result<()> {
    let have = session.profiles().load(&args.name)?.entries;
    let choosing = Choosing {
        prompter: console,
        title: "Select entries to remove",
        yes: args.yes,
    };
    let gone = pick(&choosing, &args.query, &have, ToString::to_string)?;
    if args.dry_run {
        return gone
            .iter()
            .try_for_each(|entry| console.say(&format!("would remove {entry}")));
    }
    let kept: Vec<Spec> = have.iter().filter(|e| !gone.contains(e)).cloned().collect();
    console.conclude(Some(session.set_entries(&args.name, &kept)?))
}
