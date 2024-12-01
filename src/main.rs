#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    let stdin = io::stdin();
    loop {
        // prompt
        print!("$ ");
        io::stdout().flush().unwrap();

        // Wait for user input
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        println!("{}: command not found", input.trim());
    }
}
