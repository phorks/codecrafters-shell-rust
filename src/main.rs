#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env,
    fs::{self},
    iter::Peekable,
    path::PathBuf,
    process,
    str::Chars,
};

use strum::VariantArray;
use strum_macros::{EnumDiscriminants, VariantArray};

struct LineTokenIter<'a> {
    chars: Peekable<Chars<'a>>,
}

impl<'a> LineTokenIter<'a> {
    pub fn new(line: &'a str) -> Self {
        LineTokenIter {
            chars: line.chars().peekable(),
        }
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

impl Command {
    pub fn parse(line: &str) -> anyhow::Result<Command> {
        let mut tokens = LineTokenIter::new(line);

        let name = match tokens.next() {
            Some(token) => token,
            None => anyhow::bail!("Line is empty"),
        };

        Ok(match name.as_ref() {
            "exit" => {
                let rest: Vec<_> = tokens.collect();

                let code = match rest.len() {
                    0 => 127,
                    1 => rest[0].parse()?,
                    _ => anyhow::bail!("Too many arguments (expected 2"),
                };

                Command::Exit(code)
            }
            "echo" => Command::Echo(tokens.collect()),
            "type" => Command::Type(tokens.collect()),
            "pwd" => {
                let rest = tokens.count();

                if rest != 0 {
                    anyhow::bail!("pwd: expected 0 arguments; got {}", rest);
                }

                Command::Pwd
            }
            "cd" => {
                let rest: Vec<_> = tokens.collect();

                let path = if rest.len() == 0 {
                    None
                } else if rest.len() == 1 {
                    Some(PathBuf::from(&rest[0]))
                } else {
                    anyhow::bail!("Too many arguments for cd command")
                };

                Command::Cd(path)
            }
            _ => Command::NotFound(name, tokens.collect()),
        })
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

        let command = match Command::parse(&input) {
            Ok(cmd) => cmd,
            Err(err) => {
                println!("{:?}", err);
                continue;
            }
        };

        match command {
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

                    print!("{}", message);
                    io::stdout().flush().unwrap();
                }

                if vec.len() > 0 {
                    println!("");
                }
            }
            Command::Type(vec) => {
                for name in &vec {
                    if CommandDiscriminants::is_builtin(name) {
                        println!("{} is a shell builtin", name);
                    } else {
                        match paths.expand(name) {
                            Some(path) => println!("{} is {}", name, path.display()),
                            _ => println!("{}: not found", name),
                        }
                    }
                }
            }
            Command::Pwd => match env::current_dir() {
                Ok(dir) => println!("{}", dir.display()),
                Err(err) => println!("pwd: {}", err),
            },
            Command::Cd(path) => {
                let Some(mut path) = path else { continue };

                if path.to_str() == Some("~") {
                    match env::var("HOME") {
                        Ok(home_dir) => path = PathBuf::from(home_dir),
                        _ => {
                            println!("cd: ~: home dir is not available");
                            continue;
                        }
                    };
                }

                if !path.exists() {
                    println!("cd: {}: No such file or directory", path.display());
                    continue;
                }

                env::set_current_dir(path).unwrap();
            }
            Command::NotFound(cmd, args) => match paths.expand(&cmd) {
                Some(path) => {
                    let Ok(output) = process::Command::new(&path).args(args).output() else {
                        println!("{}: Failed to execute command", path.display());
                        continue;
                    };

                    io::stdout().write(&output.stdout).unwrap();
                }
                _ => {
                    println!("{}: command not found", input.trim());
                }
            },
        }
    }
}
