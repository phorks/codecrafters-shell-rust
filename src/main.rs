#[allow(unused_imports)]
use std::io::{self, Write};
use std::str::Chars;

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
            _ => Command::NotFound,
        })
    }
}

fn main() {
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
                println!("Exiting with code {}", code);
                std::process::exit(code);
            }
            Command::NotFound => {
                println!("{}: command not found", input.trim());
            }
        }
    }
}
