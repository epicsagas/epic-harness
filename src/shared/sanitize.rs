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

/// Mask common secret patterns in a string.
/// Covers: Bearer tokens, sk-* API keys, password/token/apikey/secret/private_key values.
pub fn mask_secrets(s: &str) -> String {
    let s = MASK_BEARER.replace_all(s, "Bearer <REDACTED>");
    let s = MASK_SK.replace_all(&s, "sk-<REDACTED>");
    let s = MASK_KV.replace_all(&s, "$1=<REDACTED>");
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
}
