/// User interaction. Injected so flows are testable without a terminal.
pub trait Prompter {
    /// Let the user pick any subset of `options`; returns chosen indices (ascending).
    /// An empty result means the user cancelled or picked nothing.
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>>;
    fn confirm(&self, question: &str) -> anyhow::Result<bool>;
}

/// Interactive terminal prompter backed by `tui::select` and a y/n line prompt.
pub struct TuiPrompter;

impl Prompter for TuiPrompter {
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        crate::tui::select::run(title, options)
    }

    fn confirm(&self, question: &str) -> anyhow::Result<bool> {
        let _ = question;
        todo!("lane: tui")
    }
}

/// Non-interactive `--yes`: selects every option and confirms everything.
pub struct AssumeYes;

impl Prompter for AssumeYes {
    fn multi_select(&self, _title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok((0..options.len()).collect())
    }

    fn confirm(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}
