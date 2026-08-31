use regex::Regex;
use std::sync::LazyLock;

/// Strip characters that could be used for prompt injection in generated skill content:
/// null bytes, C1 controls (U+0080–U+009F), and Plane-14 tag characters (U+E0000–U+E01EF).
pub fn sanitize_skill_content(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            c != '\0'
                && !((c as u32) >= 0x80 && (c as u32) <= 0x9F)
                && !('\u{E0000}'..='\u{E01EF}').contains(&c)
        })
        .collect()
}

static MASK_BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)Bearer\s+[^\s"']+"#).unwrap());
static MASK_SK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"sk-[a-zA-Z0-9\-_]{8,}").unwrap());
static MASK_KV: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(password|passwd|token|api_key|apikey|secret|private_key)[=:]\s*\S+").unwrap()
});
/// Absolute file paths — Unix (`/home/u/p/x.rs`), Windows (`C:\Users\me\y`),
/// and tilde-home (`~/repo/z`). Error snippets reach an external LLM via
/// skill synthesis and may land in generated SKILL.md, so paths are masked.
static MASK_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:(?:/[A-Za-z][\w./-]*)|(?:[A-Za-z]:\\[^\s"']+)|(?:~[/\w.\\-]+))"#).unwrap()
});

/// Truncate to at most `max` bytes without splitting a UTF-8 character.
///
/// Slicing `&s[..max]` panics when byte `max` lands inside a multi-byte
/// character, which any non-ASCII command hits (a Korean path, an emoji in a
/// commit message). Backs up to the nearest character boundary instead.
pub fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Mask common secret patterns in a string.
/// Covers: Bearer tokens, sk-* API keys, password/token/apikey/secret/private_key
/// values, and absolute file paths (Unix/Windows/tilde-home).
pub fn mask_secrets(s: &str) -> String {
    let s = MASK_BEARER.replace_all(s, "Bearer <REDACTED>");
    let s = MASK_SK.replace_all(&s, "sk-<REDACTED>");
    let s = MASK_KV.replace_all(&s, "$1=<REDACTED>");
    let s = MASK_PATH.replace_all(&s, "<PATH>");
    s.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_bearer_token() {
        assert_eq!(
            mask_secrets("Authorization: Bearer abc123def456"),
            "Authorization: Bearer <REDACTED>"
        );
    }

    #[test]
    fn mask_sk_key() {
        assert_eq!(
            mask_secrets("key=sk-proj-abc123def456ghi789"),
            "key=sk-<REDACTED>"
        );
    }

    #[test]
    fn mask_password() {
        assert_eq!(mask_secrets("password=hunter2"), "password=<REDACTED>");
    }

    #[test]
    fn mask_private_key() {
        assert_eq!(
            mask_secrets("private_key=-----BEGIN"),
            "private_key=<REDACTED>"
        );
    }

    #[test]
    fn mask_apikey() {
        assert_eq!(
            mask_secrets("apikey: my-secret-key-123"),
            "apikey=<REDACTED>"
        );
    }

    #[test]
    fn no_mask_plain_text() {
        assert_eq!(
            mask_secrets("Build the auth module"),
            "Build the auth module"
        );
    }

    #[test]
    fn mask_unix_path() {
        assert_eq!(
            mask_secrets("error at /home/u/proj/src/main.rs:42"),
            "error at <PATH>:42"
        );
    }

    #[test]
    fn mask_windows_path() {
        assert_eq!(
            mask_secrets("failed in C:\\Users\\me\\proj\\x.cs"),
            "failed in <PATH>"
        );
    }

    #[test]
    fn mask_tilde_home() {
        assert_eq!(
            mask_secrets("open ~/repo/z for editing"),
            "open <PATH> for editing"
        );
    }

    #[test]
    fn truncate_bytes_shorter_than_max_is_unchanged() {
        assert_eq!(truncate_bytes("abc", 500), "abc");
    }

    #[test]
    fn truncate_bytes_ascii_cuts_exactly() {
        let s = "a".repeat(600);
        assert_eq!(truncate_bytes(&s, 500).len(), 500);
    }

    /// The panic this guards: "가" is 3 bytes, so byte 500 of a run of them
    /// lands mid-character and `&s[..500]` aborts the hook process.
    #[test]
    fn truncate_bytes_does_not_split_multibyte_char() {
        let s = "가".repeat(300);
        let out = truncate_bytes(&s, 500);
        assert!(out.len() <= 500);
        assert_eq!(out.len() % 3, 0, "must cut on a char boundary");
        assert!(s.starts_with(out));
    }

    #[test]
    fn truncate_bytes_max_zero_is_empty() {
        assert_eq!(truncate_bytes("가나다", 0), "");
    }

    /// A cut landing before any complete character yields empty, not a panic.
    #[test]
    fn truncate_bytes_below_first_char_width() {
        assert_eq!(truncate_bytes("가나다", 2), "");
    }
}
