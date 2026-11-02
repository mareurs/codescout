//! Test helper: the path inside a `read_file("…", …)` route, as the file system spells it.
//!
//! A route is text an agent pastes back, so its path is a quoted literal and a backslash is
//! written `\\`. On Unix a path has none, so a test could compare the captured text with the
//! path directly. On Windows it could not: `C:\Users\x` is `C:\\Users\\x` in the route
//! (`32d5b6837da926fc`). A test that parses a route unquotes the capture with this.

/// `captured` (the text between the quotes of a route's path) with its escapes resolved.
/// A capture that is not a valid JSON string body is returned as it came.
pub(crate) fn unquote_route_path(captured: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{captured}\""))
        .unwrap_or_else(|_| captured.to_string())
}

#[cfg(test)]
mod tests {
    use super::unquote_route_path;

    #[test]
    fn a_doubled_backslash_is_one_backslash() {
        assert_eq!(
            unquote_route_path(r"C:\\Users\\RUNNER~1\\x.md"),
            r"C:\Users\RUNNER~1\x.md"
        );
    }

    #[test]
    fn a_path_without_escapes_is_unchanged() {
        assert_eq!(unquote_route_path("/tmp/a/b.md"), "/tmp/a/b.md");
        assert_eq!(unquote_route_path("@file_0123abcd"), "@file_0123abcd");
    }

    #[test]
    fn a_capture_that_is_not_a_json_body_is_returned_as_it_came() {
        assert_eq!(unquote_route_path(r"C:\Users\x"), r"C:\Users\x");
    }
}
