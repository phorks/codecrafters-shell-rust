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
        if chars.next().unwrap() == '&' {
            source = RedirectionSource::Both;
        } else {
            let n = chars
                .by_ref()
                .take_while(|x| x.is_ascii_digit())
                .fold(0, |acc, e| (10 * acc) + ((e as u8 - '0' as u8) as u32));
            if n == 0 || n == 1 {
                // do nothing
            } else if n == 2 {
                source = RedirectionSource::Stderr;
            } else {
                return None;
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
