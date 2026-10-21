//! MCP servers as agents configure them. Each adapter reads its own file; this module holds what
//! they share: the launch description, how it is shown on an `Augmentation` and read back for a
//! profile snapshot, and the arguments `<agent> mcp add` takes.

use serde_json::Value;
use toml_edit::{Item, TableLike};

use crate::model::Augmentation;
use crate::model::Kind;
use crate::redact::{credential_count, redact, redact_args};

const COMMAND: &str = "command: ";
const ARG: &str = "arg: ";
const URL: &str = "url: ";
const TRANSPORT: &str = "transport: ";
/// An environment variable or header the server needs. Names only: values are secrets and are
/// never read into skillz.
const SECRET: &str = "needs: ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    Stdio {
        command: String,
        args: Vec<String>,
    },
    Url {
        url: String,
        /// `http` or `sse`.
        transport: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub launch: Launch,
    /// Env vars and headers it is configured with; a profile cannot carry their values.
    pub secrets: Vec<String>,
}

impl Server {
    /// Claude's `{"type": "stdio"|"http"|"sse", "command", "args", "env", "url", "headers"}`.
    pub fn from_json(name: &str, v: &Value) -> Option<Server> {
        let text = |key: &str| v.get(key).and_then(Value::as_str).map(String::from);
        let launch = match (text("command"), text("url")) {
            (Some(command), _) => Launch::Stdio {
                command,
                args: v
                    .get("args")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a.as_str().map(String::from))
                    .collect(),
            },
            (None, Some(url)) => Launch::Url {
                url,
                transport: text("type").unwrap_or_else(|| "http".into()),
            },
            _ => return None,
        };
        let keys = |field: &str| -> Vec<String> {
            v.get(field)
                .and_then(Value::as_object)
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default()
        };
        let mut secrets = keys("env");
        secrets.extend(keys("headers"));
        Some(Server {
            name: name.into(),
            launch,
            secrets,
        })
    }

    /// Codex's `[mcp_servers.<name>]`: `command`, `args`, `env`, `url`, `bearer_token_env_var`,
    /// `http_headers`.
    pub fn from_toml(name: &str, t: &dyn TableLike) -> Option<Server> {
        let text = |key: &str| t.get(key).and_then(Item::as_str).map(String::from);
        let launch = match (text("command"), text("url")) {
            (Some(command), _) => Launch::Stdio {
                command,
                args: t
                    .get("args")
                    .and_then(Item::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a.as_str().map(String::from))
                    .collect(),
            },
            (None, Some(url)) => Launch::Url {
                url,
                transport: "http".into(),
            },
            _ => return None,
        };
        let keys = |field: &str| -> Vec<String> {
            t.get(field)
                .and_then(Item::as_table_like)
                .map(|e| e.iter().map(|(k, _)| k.to_string()).collect())
                .unwrap_or_default()
        };
        let mut secrets = keys("env");
        secrets.extend(keys("http_headers"));
        secrets.extend(text("bearer_token_env_var"));
        Some(Server {
            name: name.into(),
            launch,
            secrets,
        })
    }

    /// How many secrets a profile could not carry: the env vars and headers it needs, plus the
    /// args or url that embed a credential.
    pub fn secret_count(&self) -> usize {
        let embedded = match &self.launch {
            Launch::Stdio { args, .. } => credential_count(args),
            Launch::Url { url, .. } => credential_count(std::slice::from_ref(url)),
        };
        self.secrets.len() + embedded
    }

    /// The `Augmentation::children` lines; `from_item` reads them back.
    pub fn children(&self) -> Vec<String> {
        let mut out = vec![];
        match &self.launch {
            Launch::Stdio { command, args } => {
                out.push(format!("{COMMAND}{command}"));
                out.extend(args.iter().map(|a| format!("{ARG}{a}")));
            }
            Launch::Url { url, transport } => {
                out.push(format!("{URL}{url}"));
                out.push(format!("{TRANSPORT}{transport}"));
            }
        }
        out.extend(self.secrets.iter().map(|s| format!("{SECRET}{s}")));
        out
    }

    /// The same server with credentials in its args or url hidden.
    fn redacted(self) -> Server {
        let launch = match self.launch {
            Launch::Stdio { command, args } => Launch::Stdio {
                command,
                args: redact_args(&args),
            },
            Launch::Url { url, transport } => Launch::Url {
                url: redact(&url),
                transport,
            },
        };
        Server { launch, ..self }
    }

    /// The server an MCP item was discovered from.
    pub fn from_item(item: &Augmentation) -> Option<Server> {
        let lines = |prefix: &str| -> Vec<String> {
            item.children
                .iter()
                .filter_map(|c| c.strip_prefix(prefix).map(String::from))
                .collect()
        };
        let first = |prefix: &str| lines(prefix).into_iter().next();
        let launch = match (first(COMMAND), first(URL)) {
            (Some(command), _) => Launch::Stdio {
                command,
                args: lines(ARG),
            },
            (None, Some(url)) => Launch::Url {
                url,
                transport: first(TRANSPORT).unwrap_or_else(|| "http".into()),
            },
            _ => return None,
        };
        Some(Server {
            name: item.name.clone(),
            launch,
            secrets: lines(SECRET),
        })
    }
}

/// The item's children as safe to show: an MCP server's args and url with credentials hidden.
pub fn shown_children(item: &Augmentation) -> Vec<String> {
    match Server::from_item(item).filter(|_| item.kind == Kind::Mcp) {
        Some(server) => server.redacted().children(),
        None => item.children.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Installer, Kind, Scope};
    use serde_json::json;

    fn item(server: &Server) -> Augmentation {
        Augmentation {
            hooks: Vec::new(),
            id: "mcp:x".into(),
            name: server.name.clone(),
            kind: Kind::Mcp,
            agents: vec![Agent::Claude],
            scope: Scope::Project,
            group: "mcp".into(),
            source: None,
            installer: Installer::ClaudeCli,
            origin: Default::default(),
            children: server.children(),
            removal: vec![],
        }
    }

    #[test]
    fn json_servers_read_launch_and_secret_names_but_never_values() {
        let stdio = Server::from_json(
            "s",
            &json!({"type": "stdio", "command": "npx", "args": ["-y", "pkg"], "env": {"TOKEN": "hunter2"}}),
        )
        .unwrap();
        assert_eq!(
            stdio.launch,
            Launch::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "pkg".into()]
            }
        );
        assert_eq!(stdio.secrets, ["TOKEN"]);
        assert!(!stdio.children().join("\n").contains("hunter2"));

        let http = Server::from_json(
            "h",
            &json!({"type": "sse", "url": "https://x/mcp", "headers": {"Authorization": "Bearer z"}}),
        )
        .unwrap();
        assert_eq!(
            http.launch,
            Launch::Url {
                url: "https://x/mcp".into(),
                transport: "sse".into()
            }
        );
        assert_eq!(http.secrets, ["Authorization"]);
        assert!(Server::from_json("n", &json!({"type": "stdio"})).is_none());
    }

    #[test]
    fn toml_servers_read_the_same_things() {
        let doc: toml_edit::DocumentMut = r#"
[mcp_servers.a]
command = "node"
args = ["x.js"]
[mcp_servers.a.env]
K = "v"
[mcp_servers.b]
url = "https://u"
bearer_token_env_var = "TOK"
[mcp_servers.c]
enabled = true
"#
        .parse()
        .unwrap();
        let table = doc["mcp_servers"].as_table_like().unwrap();
        let get = |n: &str| Server::from_toml(n, table.get(n).unwrap().as_table_like().unwrap());
        let a = get("a").unwrap();
        assert_eq!(
            a.launch,
            Launch::Stdio {
                command: "node".into(),
                args: vec!["x.js".into()]
            }
        );
        assert_eq!(a.secrets, ["K"]);
        assert_eq!(get("b").unwrap().secrets, ["TOK"]);
        assert!(get("c").is_none());
    }

    #[test]
    fn shown_children_hide_credentials_in_args_and_urls() {
        let stdio = Server {
            name: "s".into(),
            launch: Launch::Stdio {
                command: "npx".into(),
                args: vec!["--token".into(), "s3cret".into()],
            },
            secrets: vec![],
        };
        let http = Server {
            name: "h".into(),
            launch: Launch::Url {
                url: "https://u:p4ss@h/mcp?k=s3cret".into(),
                transport: "http".into(),
            },
            secrets: vec![],
        };

        for server in [stdio, http] {
            let shown = shown_children(&item(&server)).join("\n");
            // shown lines are redacted while the stored children keep the originals
            assert!(!shown.contains("s3cret") && !shown.contains("p4ss"));
            assert!(item(&server).children.join("\n").contains("s3cret"));
        }
    }

    #[test]
    fn children_round_trip_through_the_item() {
        for server in [
            Server {
                name: "s".into(),
                launch: Launch::Stdio {
                    command: "npx".into(),
                    args: vec!["-y".into(), "p".into()],
                },
                secrets: vec!["K".into()],
            },
            Server {
                name: "h".into(),
                launch: Launch::Url {
                    url: "https://u".into(),
                    transport: "http".into(),
                },
                secrets: vec![],
            },
        ] {
            assert_eq!(Server::from_item(&item(&server)).unwrap(), server);
        }
        let mut empty = item(&Server {
            name: "e".into(),
            launch: Launch::Url {
                url: "u".into(),
                transport: "http".into(),
            },
            secrets: vec![],
        });
        empty.children.clear();
        assert!(Server::from_item(&empty).is_none());
    }
}
