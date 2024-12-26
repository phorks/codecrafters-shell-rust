use peeking_take_while::PeekableExt;

#[derive(Clone)]
pub enum RedirectionMode {
    Write,
    Append,
}

#[derive(Clone)]
pub enum RedirectionSource {
    Stdout,
    Stderr,
    Both,
}

#[derive(Clone)]
pub struct Redirection {
    pub source: RedirectionSource,
    pub mode: RedirectionMode,
    pub target: String,
}

impl Redirection {
    pub fn parse(value: &str) -> Option<Redirection> {
        if value.len() == 0 {
            return None;
        }

        let mut chars = value.chars().peekable();
        let mut source = RedirectionSource::Stdout;

        if *chars.peek().unwrap() == '&' {
            source = RedirectionSource::Both;
            chars.next().unwrap();
        } else {
            let n_str = chars
                .by_ref()
                .peeking_take_while(|x| x.is_ascii_digit())
                .collect::<String>();

            if n_str.len() > 0 {
                let n = n_str.parse::<u32>().unwrap();
                if n == 0 || n == 1 {
                    // do nothing
                } else if n == 2 {
                    source = RedirectionSource::Stderr;
                } else {
                    return None;
                }
            }
        }

        let n_lt = chars.by_ref().take_while(|x| *x == '>').count();

        let mode = match n_lt {
            1 => RedirectionMode::Write,
            2 => RedirectionMode::Append,
            _ => return None,
        };

        Some(Redirection {
            source,
            mode,
            target: chars
                .skip_while(|x| x.is_whitespace())
                .take_while(|x| !x.is_whitespace())
                .collect(),
        })
    }
}
