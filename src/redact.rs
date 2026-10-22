//! Spotting and hiding credentials in command lines and urls: userinfo, query strings,
//! `--token VALUE` / `--api-key=VALUE` / `NAME_SECRET=VALUE` pairs and `Bearer` tokens.

const MASK: &str = "***";
const SECRET_WORDS: [&str; 6] = ["key", "token", "secret", "password", "passwd", "credential"];

fn is_secret_name(name: &str) -> bool {
    let name = name.trim_start_matches('-').to_ascii_lowercase();
    name == "authorization" || SECRET_WORDS.iter().any(|w| name.contains(w))
}

/// One argument with any credential it carries hidden.
pub fn redact(arg: &str) -> String {
    if let Some(at) = bearer_end(arg) {
        return format!("{}{MASK}", &arg[..at]);
    }
    if arg.contains("://") {
        return redact_url(arg);
    }
    match arg.split_once('=') {
        Some((name, _)) if is_secret_name(name) => format!("{name}={MASK}"),
        _ => arg.to_string(),
    }
}

/// Where the token starts in a `Bearer <token>` value.
fn bearer_end(arg: &str) -> Option<usize> {
    let marker = "bearer ";
    arg.to_ascii_lowercase()
        .find(marker)
        .map(|at| at + marker.len())
}

fn redact_url(url: &str) -> String {
    let (base, has_query) = match url.split_once('?') {
        Some((base, _)) => (base, true),
        None => (url, false),
    };
    let base = redact_userinfo(base);
    if has_query {
        format!("{base}?{MASK}")
    } else {
        base
    }
}

fn redact_userinfo(url: &str) -> String {
    match (url.find("://"), url.find('@')) {
        (Some(scheme), Some(at)) if at > scheme && !url[scheme + 3..at].contains('/') => {
            format!("{}://{MASK}{}", &url[..scheme], &url[at..])
        }
        _ => url.to_string(),
    }
}

/// Every argument with credentials hidden, including the value after a bare `--token` flag.
pub fn redact_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut hide_next = false;
    for arg in args {
        if std::mem::take(&mut hide_next) {
            out.push(MASK.to_string());
            continue;
        }
        hide_next = takes_secret_value(arg);
        out.push(redact(arg));
    }
    out
}

fn takes_secret_value(arg: &str) -> bool {
    arg.eq_ignore_ascii_case("bearer")
        || (arg.starts_with('-') && !arg.contains('=') && is_secret_name(arg))
}

/// The arguments as one line, credentials hidden.
pub fn redact_line(args: &[String]) -> String {
    redact_args(args).join(" ")
}

/// Free text with credential-bearing arguments hidden, including quoted flag values.
pub fn redact_text(text: &str) -> String {
    text.lines()
        .map(|line| {
            let words = shell_words::split(line)
                .unwrap_or_else(|_| line.split_whitespace().map(str::to_owned).collect());
            redact_line(&words)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many of `args` carry a credential.
pub fn credential_count(args: &[String]) -> usize {
    redact_args(args)
        .iter()
        .zip(args)
        .filter(|(shown, arg)| shown != arg)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn url_userinfo_and_query_are_hidden_but_plain_urls_and_scp_paths_are_not() {
        assert_eq!(
            redact("https://u:tok@github.com/o/r.git"),
            "https://***@github.com/o/r.git"
        );
        assert_eq!(redact("https://x/mcp?api_key=abc"), "https://x/mcp?***");
        assert_eq!(redact("https://github.com/a@b"), "https://github.com/a@b");
        assert_eq!(redact("git@github.com:o/r.git"), "git@github.com:o/r.git");
    }

    #[test]
    fn secret_flags_hide_their_value_in_either_form() {
        let shown = redact_args(&args(&["--api-key", "abc", "--token=def", "--port", "80"]));

        assert_eq!(shown, ["--api-key", "***", "--token=***", "--port", "80"]);
    }

    #[test]
    fn bearer_tokens_and_secret_assignments_are_hidden() {
        let shown = redact_args(&args(&[
            "Authorization: Bearer abc",
            "Bearer",
            "xyz",
            "DB_PASSWORD=p",
        ]));

        assert_eq!(
            shown,
            [
                "Authorization: Bearer ***",
                "Bearer",
                "***",
                "DB_PASSWORD=***"
            ]
        );
    }

    #[test]
    fn credential_count_counts_carriers_only() {
        assert_eq!(credential_count(&args(&["-y", "pkg", "--token", "t"])), 1);
        assert_eq!(credential_count(&args(&["-y", "pkg"])), 0);
    }
    #[test]
    fn diagnostic_text_hides_bare_flags_and_quoted_secret_values() {
        for text in [
            "notify --token text-secret",
            "notify '--token' 'text-secret with spaces'",
            "failed --api-key text-secret: could not connect",
            "notify --token text-secret 'unterminated",
        ] {
            assert!(!redact_text(text).contains("text-secret"));
        }
    }
}
