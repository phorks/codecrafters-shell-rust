#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env,
    fs::{self},
    path::PathBuf,
    str::Chars,
};

struct LineTokenIter<'a> {
    chars: Chars<'a>,
}

impl<'a> LineTokenIter<'a> {
    pub fn new(line: &'a str) -> Self {
        LineTokenIter {
            chars: line.chars(),
        }
    }
}

impl<'a> Iterator for LineTokenIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut token = String::new();
        let mut in_quotes = false;

        while let Some(ch) = self.chars.next() {
            match ch {
                '"' => in_quotes = !in_quotes,
                '\\' => match self.chars.next() {
                    Some(next) => token.push(next),
                    None => panic!("Line ended in a '\\'."),
                },
                ' ' if !in_quotes && token.len() != 0 => break,
                '\n' if !in_quotes && token.len() != 0 => break,
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

enum Command {
    Exit(i32),
    Echo(Vec<String>),
    Type(Vec<String>),
    NotFound,
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
            _ => Command::NotFound,
        })
    }

    pub fn is_builtin(command: &str) -> bool {
        return command == "exit" || command == "echo" || command == "type";
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
                    if Command::is_builtin(name) {
                        println!("{} is a shell builtin", name);
                    } else {
                        match paths.expand(name) {
                            Some(path) => println!("{} is {}", name, path.display()),
                            _ => println!("{}: not found", name),
                        }
                    }
                }
            }
            Command::NotFound => {
                println!("{}: command not found", input.trim());
            }
        }
    }
}
