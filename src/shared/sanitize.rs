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

/// Mask credential patterns but leave file paths intact.
///
/// Covers Bearer tokens, `sk-*` API keys, and
/// password/token/apikey/secret/private_key assignments.
///
/// Use this for values that stay on the machine and whose paths carry meaning —
/// an observation's `action` drives file-level pattern detection, and replacing
/// every path with `<PATH>` would make every edit look like the same file.
pub fn mask_secrets_keep_paths(s: &str) -> String {
    let s = MASK_BEARER.replace_all(s, "Bearer <REDACTED>");
    let s = MASK_SK.replace_all(&s, "sk-<REDACTED>");
    let s = MASK_KV.replace_all(&s, "$1=<REDACTED>");
    s.into_owned()
}

/// Mask common secret patterns in a string.
/// Covers: Bearer tokens, sk-* API keys, password/token/apikey/secret/private_key
/// values, and absolute file paths (Unix/Windows/tilde-home).
pub fn mask_secrets(s: &str) -> String {
    MASK_PATH
        .replace_all(&mask_secrets_keep_paths(s), "<PATH>")
        .into_owned()
}

/// Truncate to at most `max_bytes`, never splitting a UTF-8 character.
///
/// `&s[..n]` panics when `n` lands inside a multi-byte character, which a byte
/// cap over arbitrary command output eventually will.
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
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

    // ── mask_secrets_keep_paths ─────────────────────

    #[test]
    fn keep_paths_still_masks_credentials() {
        assert_eq!(
            mask_secrets_keep_paths("curl -H 'Authorization: Bearer abc123def456' https://x"),
            "curl -H 'Authorization: Bearer <REDACTED>' https://x"
        );
        assert_eq!(
            mask_secrets_keep_paths("export API_KEY=supersecretvalue"),
            "export API_KEY=<REDACTED>"
        );
    }

    #[test]
    fn keep_paths_leaves_file_paths_readable() {
        // Observation actions key file-level pattern detection; `<PATH>` would
        // make every edit look like the same file.
        let cmd = "cargo test --manifest-path /home/u/proj/Cargo.toml";
        assert_eq!(mask_secrets_keep_paths(cmd), cmd);
    }

    // ── truncate_utf8 ───────────────────────────────

    #[test]
    fn truncate_shorter_than_cap_is_unchanged() {
        assert_eq!(truncate_utf8("abc", 10), "abc");
    }

    #[test]
    fn truncate_never_splits_a_character() {
        // "日" is three bytes, so byte 3 lands inside it — a naive &s[..3]
        // would panic. Every cap must yield a prefix on a boundary.
        let s = "a日本語";
        for cap in 0..=s.len() + 2 {
            let out = truncate_utf8(s, cap);
            assert!(s.starts_with(out), "cap {cap} produced {out:?}");
            assert!(out.len() <= cap, "cap {cap} produced {out:?}");
        }
        assert_eq!(truncate_utf8(s, 3), "a");
    }

    #[test]
    fn truncate_at_an_exact_boundary_keeps_the_character() {
        // 'a' + '日' is exactly four bytes.
        assert_eq!(truncate_utf8("a日本語", 4), "a日");
    }

    #[test]
    fn truncate_to_zero_yields_empty() {
        assert_eq!(truncate_utf8("日", 0), "");
        assert_eq!(truncate_utf8("日", 1), "");
    }
}
