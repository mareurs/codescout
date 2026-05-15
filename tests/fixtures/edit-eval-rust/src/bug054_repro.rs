pub trait Tool {
    fn format_compact(&self, n: u32) -> Option<String>;
}

pub struct ReadMarkdown;

impl Tool for ReadMarkdown {
    fn format_compact(&self, n: u32) -> Option<String> {
        if n > 0 {
            return Some(String::from("content"));
        }
        Some(String::from("map"))
    }
}
