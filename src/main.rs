#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    cell::RefCell,
    env,
    fs::{self, OpenOptions},
    iter::Peekable,
    marker::PhantomData,
    path::PathBuf,
    process,
    str::Chars,
};

use redirection::{Redirection, RedirectionMode, RedirectionSource};
use strum::VariantArray;
use strum_macros::{EnumDiscriminants, VariantArray};

mod redirection;

struct LineTokenIter<'a> {
    chars: Peekable<Chars<'a>>,
    redirection: Option<String>,
}

impl<'a> LineTokenIter<'a> {
    pub fn new(line: &'a str) -> Self {
        LineTokenIter {
            chars: line.chars().peekable(),
            redirection: None,
        }
    }

    fn redirection(&self) -> Option<Redirection> {
        self.redirection
            .as_ref()
            .and_then(|x| Redirection::parse(x))
    }
}

enum QuoteKind {
    Single,
    Double,
    None,
}

impl<'a> Iterator for LineTokenIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut token = String::new();
        let mut quote = QuoteKind::None;

        while let Some(ch) = self.chars.next() {
            match (ch, &quote) {
                ('"', QuoteKind::Double) => quote = QuoteKind::None,
                ('"', QuoteKind::None) => quote = QuoteKind::Double,
                ('\'', QuoteKind::Single) => quote = QuoteKind::None,
                ('\'', QuoteKind::None) => quote = QuoteKind::Single,
                ('\\', QuoteKind::None) => match self.chars.next() {
                    Some(next) => token.push(next),
                    None => panic!("Line ended in a '\\'."),
                },
                ('\\', QuoteKind::Double) => match self.chars.peek() {
                    Some(next) => {
                        if matches!(next, '\\' | '$' | '"' | '\n') {
                            token.push(next.clone());
                            self.chars.next().unwrap();
                        } else {
                            token.push('\\');
                        }
                    }
                    None => panic!("Line ended in a '\\'."),
                },
                (' ' | '\n', QuoteKind::None) if token.len() > 0 => break,
                (' ', _) if token.len() == 0 => continue,
                ('>', QuoteKind::None) => {
                    if token.len() > 0 {
                        if token.chars().all(|x| x.is_ascii_digit()) || token == "&" {
                            token.push('>');
                            self.chars.by_ref().for_each(|x| token.push(x));
                            self.redirection = Some(token);
                            return None;
                        } else {
                            self.redirection =
                                Some(format!(">{}", self.chars.by_ref().collect::<String>()));
                            break;
                        }
                    } else {
                        self.redirection =
                            Some(format!(">{}", self.chars.by_ref().collect::<String>()));
                        break;
                    }
                }
                _ => token.push(ch),
            }
        }

        if token.len() > 0 {
            Some(token)
        } else {
            None
        }
    }
}

#[derive(EnumDiscriminants)]
#[strum_discriminants(derive(VariantArray))]
enum Command {
    Exit(i32),
    Echo(Vec<String>),
    Type(Vec<String>),
    Pwd,
    Cd(Option<PathBuf>),
    NotFound(String, Vec<String>),
}

impl CommandDiscriminants {
    fn builtin_name(&self) -> Option<&'static str> {
        match self {
            CommandDiscriminants::Exit => Some("exit"),
            CommandDiscriminants::Echo => Some("echo"),
            CommandDiscriminants::Type => Some("type"),
            CommandDiscriminants::Pwd => Some("pwd"),
            CommandDiscriminants::Cd => Some("cd"),
            CommandDiscriminants::NotFound => None,
        }
    }

    pub fn is_builtin(command: &str) -> bool {
        return CommandDiscriminants::VARIANTS
            .iter()
            .any(|x| x.builtin_name().map(|x| x == command).unwrap_or(false));
    }
}

struct InputCommand {
    command: Command,
    redirect: Option<Redirection>,
}

impl InputCommand {
    pub fn parse(line: &str) -> anyhow::Result<InputCommand> {
        let mut tokens = LineTokenIter::new(line);

        let name = match tokens.next() {
            Some(token) => token,
            None => anyhow::bail!("Line is empty"),
        };

        let rest = tokens.by_ref().collect::<Vec<_>>();

        let command = match name.as_ref() {
            "exit" => {
                let code = match rest.len() {
                    0 => 127,
                    1 => rest[0].parse()?,
                    _ => anyhow::bail!("Too many arguments (expected 2"),
                };

                Command::Exit(code)
            }
            "echo" => Command::Echo(rest),
            "type" => Command::Type(rest),
            "pwd" => {
                if rest.len() != 0 {
                    anyhow::bail!("pwd: expected 0 arguments; got {}", rest.len());
                }

                Command::Pwd
            }
            "cd" => {
                let path = if rest.len() == 0 {
                    None
                } else if rest.len() == 1 {
                    Some(PathBuf::from(&rest[0]))
                } else {
                    anyhow::bail!("Too many arguments for cd command")
                };

                Command::Cd(path)
            }
            _ => Command::NotFound(name, rest),
        };

        Ok(InputCommand {
            command,
            redirect: tokens.redirection(),
        })
    }

    fn out(&self) -> anyhow::Result<CommandOutput> {
        let redirect = match self.redirect.clone() {
            Some(redirect) => {
                println!("Redirect is not none {}", redirect.target);
                let mut options = OpenOptions::new();
                let mut options = options.create(true);
                options = match redirect.mode {
                    RedirectionMode::Write => options.write(true),
                    RedirectionMode::Append => options.append(true),
                };

                let file = options
                    .open(&redirect.target)
                    .map_err(anyhow::Error::from)?;
                Some((redirect, RefCell::new(file)))
            }
            _ => None,
        };

        Ok(CommandOutput {
            redirect,
            _not_send: Default::default(),
        })
    }
}

struct CommandOutput {
    redirect: Option<(Redirection, RefCell<std::fs::File>)>,
    _not_send: PhantomData<*const ()>, // since `redirect` can be shared between stdout and stderr, we must make this type !Send
}

impl CommandOutput {
    fn writers(
        &self,
    ) -> (
        CommandWriter<std::io::Stdout>,
        CommandWriter<std::io::Stderr>,
    ) {
        (
            CommandWriter {
                handle: std::io::stdout(),
                target: CommandWriterTarget::Stdout,
                output: self,
            },
            CommandWriter {
                handle: std::io::stderr(),
                target: CommandWriterTarget::Stderr,
                output: self,
            },
        )
    }
}

enum CommandWriterTarget {
    Stdout,
    Stderr,
}

impl CommandWriterTarget {
    fn matches_source(&self, src: &RedirectionSource) -> bool {
        match src {
            RedirectionSource::Stdout => matches!(self, CommandWriterTarget::Stdout),
            RedirectionSource::Stderr => matches!(self, CommandWriterTarget::Stderr),
            RedirectionSource::Both => true,
        }
    }
}

struct CommandWriter<'a, S: Write> {
    target: CommandWriterTarget,
    output: &'a CommandOutput,
    handle: S,
}

impl<'a, S: Write> Write for CommandWriter<'a, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let x = &self.output;
        let y = &x.redirect;
        match y {
            Some((ref redirect, ref file)) if self.target.matches_source(&redirect.source) => {
                file.borrow_mut().write(buf)
            }
            _ => self.handle.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &self.output.redirect {
            Some((redirect, ref file)) if self.target.matches_source(&redirect.source) => {
                file.borrow_mut().flush()
            }
            _ => self.handle.flush(),
        }
    }
}

struct EnvPaths {
    paths: Vec<PathBuf>,
}

impl EnvPaths {
    pub fn from_env() -> anyhow::Result<Self> {
        let var = env::var("PATH")?;
        Ok(EnvPaths {
            paths: var.split(':').map(|x| PathBuf::from(x)).collect(),
        })
    }

    pub fn expand(&self, command: &str) -> Option<PathBuf> {
        for path in &self.paths {
            let full_path = path.join(command);
            let Ok(md) = fs::metadata(&full_path) else {
                continue;
            };

            if md.is_file() {
                return Some(full_path);
            }
        }

        None
    }
}

fn main() {
    let paths = EnvPaths::from_env().unwrap();

    let stdin = io::stdin();

    loop {
        // prompt
        print!("$ ");
        io::stdout().flush().unwrap();

        // Wait for user input
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        let command = match InputCommand::parse(&input) {
            Ok(cmd) => cmd,
            Err(err) => {
                println!("{:?}", err);
                continue;
            }
        };

        let Ok(out) = command.out() else {
            eprintln!("Failed to redirect");
            continue;
        };

        let (mut stdout, mut stderr) = out.writers();

        match command.command {
            Command::Exit(code) => {
                std::process::exit(code);
            }
            Command::Echo(vec) => {
                for i in 0..vec.len() {
                    let message = if i != 0 {
                        &format!(" {}", vec[i])
                    } else {
                        &vec[i]
                    };

                    write!(&mut stdout, "{}", message).unwrap();
                    io::stdout().flush().unwrap();
                }

                if vec.len() > 0 {
                    writeln!(&mut stdout, "").unwrap();
                }
            }
            Command::Type(vec) => {
                for name in &vec {
                    if CommandDiscriminants::is_builtin(name) {
                        writeln!(&mut stdout, "{} is a shell builtin", name).unwrap();
                    } else {
                        match paths.expand(name) {
                            Some(path) => {
                                writeln!(&mut stdout, "{} is {}", name, path.display()).unwrap()
                            }
                            _ => writeln!(&mut stderr, "{}: not found", name).unwrap(),
                        }
                    }
                }
            }
            Command::Pwd => match env::current_dir() {
                Ok(dir) => writeln!(&mut stdout, "{}", dir.display()).unwrap(),
                Err(err) => writeln!(&mut stderr, "pwd: {}", err).unwrap(),
            },
            Command::Cd(path) => {
                let Some(mut path) = path else { continue };

                if path.to_str() == Some("~") {
                    match env::var("HOME") {
                        Ok(home_dir) => path = PathBuf::from(home_dir),
                        _ => {
                            writeln!(&mut stderr, "cd: ~: home dir is not available").unwrap();
                            continue;
                        }
                    };
                }

                if !path.exists() {
                    writeln!(
                        &mut stderr,
                        "cd: {}: No such file or directory",
                        path.display()
                    )
                    .unwrap();
                    continue;
                }

                env::set_current_dir(path).unwrap();
            }
            Command::NotFound(cmd, args) => match paths.expand(&cmd) {
                Some(path) => {
                    let Ok(output) = process::Command::new(&path).args(args).output() else {
                        writeln!(&mut stderr, "{}: Failed to execute command", path.display())
                            .unwrap();
                        continue;
                    };

                    stdout.write(&output.stdout).unwrap();
                    stderr.write(&output.stderr).unwrap();
                }
                _ => {
                    writeln!(&mut stderr, "{}: command not found", input.trim()).unwrap();
                }
            },
        }
    }
}
