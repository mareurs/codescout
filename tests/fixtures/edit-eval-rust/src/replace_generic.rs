use std::fmt::Display;
use std::str::FromStr;

pub fn parse<T>(s: &str) -> Option<T>
where
    T: FromStr + Display + 'static,
{
    let trimmed = s.trim();
    trimmed.parse().ok()
}
