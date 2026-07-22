fn declares_schema_field(text: &str, field: &str) -> bool {
    text.contains(&format!("`{field}`")) || text.contains(&format!("\"{field}\""))
}

fn executable_reforge_commands(contents: &str) -> Vec<Vec<String>> {
    let mut executable_fence = false;
    let mut commands = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(language) = trimmed.strip_prefix("```") {
            if executable_fence {
                executable_fence = false;
            } else {
                executable_fence = matches!(
                    language.trim().to_ascii_lowercase().as_str(),
                    "" | "bash" | "console" | "powershell" | "sh" | "shell" | "zsh"
                );
            }
            continue;
        }
        if !executable_fence {
            continue;
        }
        let command = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
        if !command.starts_with("reforge ") {
            continue;
        }
        commands.push(command_tokens(command));
    }
    commands
}

fn command_tokens(command: &str) -> Vec<String> {
    let mut tokenizer = CommandTokenizer::default();
    for character in command.chars() {
        tokenizer.consume(character);
    }
    tokenizer.finish()
}

#[derive(Default)]
struct CommandTokenizer {
    tokens: Vec<String>,
    current: String,
    quote: Option<char>,
    escaped: bool,
}

impl CommandTokenizer {
    fn consume(&mut self, character: char) {
        if self.escaped {
            self.current.push(character);
            self.escaped = false;
            return;
        }
        if character == '\\' && self.quote != Some('\'') {
            self.escaped = true;
            return;
        }
        if matches!(character, '\'' | '"') {
            self.consume_quote(character);
        } else if character.is_whitespace() && self.quote.is_none() {
            self.flush();
        } else {
            self.current.push(character);
        }
    }

    fn consume_quote(&mut self, character: char) {
        match self.quote {
            Some(quote) if quote == character => self.quote = None,
            None => self.quote = Some(character),
            Some(_) => self.current.push(character),
        }
    }

    fn flush(&mut self) {
        if !self.current.is_empty() {
            self.tokens.push(std::mem::take(&mut self.current));
        }
    }

    fn finish(mut self) -> Vec<String> {
        if self.escaped {
            self.current.push('\\');
        }
        self.flush();
        self.tokens
    }
}

fn scan_cli_flags() -> Vec<String> {
    let command = Cli::command();
    let Some(scan) = command.find_subcommand("scan") else {
        return Vec::new();
    };

    scan.get_arguments()
        .filter_map(|argument| argument.get_long())
        .map(|long| format!("--{long}"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn first_existing(root: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn collect_markdown_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_markdown_files(&path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "md") {
            output.push(path);
        }
    }
    Ok(())
}

fn display_path(path: &Path) -> String {
    crate::pathing::display_path(path)
}
