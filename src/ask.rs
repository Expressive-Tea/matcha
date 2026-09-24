//! Questions for a person at a terminal, and defaults for everyone else.
//!
//! Off a TTY (CI, a pipe, a script) nothing is asked: each question takes its
//! default, and one without a default is an error naming the flag to pass.
//! Blocking on a read that nobody will ever answer is the one thing this must
//! never do.

use std::io::{self, BufRead, IsTerminal, Write};

pub struct Ask<R, W> {
    interactive: bool,
    input: R,
    output: W,
}

impl Ask<io::StdinLock<'static>, io::Stderr> {
    /// Prompts go to stderr, so stdout stays the report of what was written.
    pub fn stdio() -> Self {
        Ask::new(io::stdin().is_terminal(), io::stdin().lock(), io::stderr())
    }
}

impl<R: BufRead, W: Write> Ask<R, W> {
    pub fn new(interactive: bool, input: R, output: W) -> Self {
        Ask {
            interactive,
            input,
            output,
        }
    }

    /// A line for the person answering; silent off a TTY.
    pub fn say(&mut self, text: &str) -> io::Result<()> {
        if self.interactive {
            writeln!(self.output, "{text}")?;
        }
        Ok(())
    }

    fn line(&mut self, prompt: &str) -> io::Result<String> {
        write!(self.output, "{prompt} ")?;
        self.output.flush()?;
        let mut answer = String::new();
        if self.input.read_line(&mut answer)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "no answer (end of input)",
            ));
        }
        Ok(answer.trim().to_string())
    }

    pub fn yes_no(&mut self, question: &str, default: bool) -> io::Result<bool> {
        if !self.interactive {
            return Ok(default);
        }
        let hint = if default { "(Y/n)" } else { "(y/N)" };
        let answer = self.line(&format!("{question} {hint}"))?.to_lowercase();
        Ok(match answer.as_str() {
            "" => default,
            "y" | "yes" => true,
            _ => false,
        })
    }

    /// One of `choices`, each `(key, value)`: the answer may be the key or the
    /// value, and Enter takes `default`. Anything else is asked again. Quietly
    /// taking the default would turn a typed `npm` into a JSR-only package.
    pub fn choice(
        &mut self,
        question: &str,
        choices: &[(&str, &'static str)],
        default: &'static str,
        flag: &str,
    ) -> io::Result<&'static str> {
        if !self.interactive {
            return Ok(default);
        }
        let keys: Vec<&str> = choices.iter().map(|(k, _)| *k).collect();
        loop {
            let answer = self.line(question)?.to_lowercase();
            if answer.is_empty() {
                return Ok(default);
            }
            if let Some((_, v)) = choices.iter().find(|(k, v)| answer == *k || answer == *v) {
                return Ok(v);
            }
            writeln!(self.output, "answer {} (or pass {flag})", keys.join(" or "))?;
        }
    }

    /// `flag` names what to pass instead, for the error off a TTY.
    pub fn text(
        &mut self,
        question: &str,
        default: Option<&str>,
        flag: &str,
    ) -> io::Result<String> {
        if !self.interactive {
            return default.map(str::to_string).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("no terminal to ask on; pass {flag}"),
                )
            });
        }
        let prompt = match default {
            Some(d) => format!("{question} ({d})"),
            None => question.to_string(),
        };
        loop {
            let answer = self.line(&prompt)?;
            if !answer.is_empty() {
                return Ok(answer);
            }
            if let Some(d) = default {
                return Ok(d.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn scripted(answers: &str) -> Ask<Cursor<Vec<u8>>, Vec<u8>> {
        Ask::new(true, Cursor::new(answers.as_bytes().to_vec()), Vec::new())
    }

    fn piped() -> Ask<Cursor<Vec<u8>>, Vec<u8>> {
        Ask::new(false, Cursor::new(Vec::new()), Vec::new())
    }

    #[test]
    fn yes_no_takes_the_default_on_enter_and_reads_y() {
        assert!(!scripted("\n").yes_no("Package?", false).unwrap());
        assert!(scripted("y\n").yes_no("Package?", false).unwrap());
        assert!(scripted("YES\n").yes_no("Package?", false).unwrap());
        assert!(!scripted("nope\n").yes_no("Package?", false).unwrap());
    }

    #[test]
    fn text_takes_the_default_on_enter() {
        assert_eq!(
            scripted("\n")
                .text("Folder?", Some("plugins"), "--folder")
                .unwrap(),
            "plugins"
        );
        assert_eq!(
            scripted("lib\n")
                .text("Folder?", Some("plugins"), "--folder")
                .unwrap(),
            "lib"
        );
    }

    #[test]
    fn text_without_a_default_asks_again_until_answered() {
        assert_eq!(
            scripted("\n\nalgo\n").text("Name?", None, "NAME").unwrap(),
            "algo"
        );
    }

    #[test]
    fn text_errors_on_eof() {
        let err = scripted("").text("Name?", None, "NAME").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn choice_asks_again_on_an_answer_that_is_not_offered() {
        let mut ask = scripted("npm\n3\n2\n");
        let picked = ask
            .choice(
                "Publish to:",
                &[("1", "jsr"), ("2", "both")],
                "jsr",
                "--registry",
            )
            .unwrap();
        assert_eq!(picked, "both");
        let shown = String::from_utf8(ask.output.clone()).unwrap();
        assert!(shown.contains("answer 1 or 2"), "{shown}");
    }

    #[test]
    fn choice_takes_the_default_on_enter_and_off_a_tty() {
        let choices = [("1", "jsr"), ("2", "both")];
        assert_eq!(
            scripted("\n")
                .choice("P?", &choices, "jsr", "--registry")
                .unwrap(),
            "jsr"
        );
        assert_eq!(
            scripted("both\n")
                .choice("P?", &choices, "jsr", "--registry")
                .unwrap(),
            "both"
        );
        assert_eq!(
            piped().choice("P?", &choices, "jsr", "--registry").unwrap(),
            "jsr"
        );
    }

    #[test]
    fn off_a_tty_nothing_is_asked() {
        let mut ask = piped();
        assert!(!ask.yes_no("Package?", false).unwrap());
        assert_eq!(
            ask.text("Folder?", Some("plugins"), "--folder").unwrap(),
            "plugins"
        );
        let err = ask.text("Name?", None, "the NAME argument").unwrap_err();
        assert!(err.to_string().contains("the NAME argument"), "{err}");
        assert!(
            ask.output.is_empty(),
            "nothing is printed when nobody is there to answer"
        );
    }
}
