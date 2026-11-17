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

/// Quoted-value alternatives shared by the key/value and CLI-flag masks. A quote
/// may be backslash-escaped (JSON nested inside a JSON string, `password=\"a b\"`)
/// and an unterminated quote runs to the end of the line.
const QUOTED_VALUE: &str = r#"\\+"[^"\r\n]*?\\+"|\\+"[^\r\n]*|"(?:\\.|[^"\\\r\n])*"|"[^\r\n]*|'(?:\\.|[^'\\\r\n])*'|'[^\r\n]*"#;

// The key may be quoted or escape-quoted (`"token":"a b"`, `\"token\":\"a b\"`),
// carry a prefix (`AWS_SECRET_ACCESS_KEY`, `x-api-key`) or suffix, and the value
// may be a quoted string with spaces. `max_token: 5` is over-redacted on purpose:
// a word boundary would also let `access_token` through.
static MASK_KV: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        &r#"(?i)(?:\\*["'])?((?:password|passwd|token|api[_-]?key|secret|private[_-]?key)(?:[_-][a-z0-9]+)*)(?:\\*["'])?\s*[=:]\s*(?:QUOTED|\S+)"#
            .replace("QUOTED", QUOTED_VALUE),
    )
    .unwrap()
});
/// `--password VALUE` / `--api-key VALUE` (the `=` form is covered by `MASK_KV`).
/// A following flag is not taken for the value.
static MASK_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        &r#"(?i)(--[\w-]*(?:password|passwd|token|secret|api-?key|private-?key)\s+)(?:QUOTED|[^\s-]\S*)"#
            .replace("QUOTED", QUOTED_VALUE),
    )
    .unwrap()
});
/// `Authorization: Basic|token|Digest <value>` (`Bearer` is `MASK_BEARER`).
static MASK_AUTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(authorization(?:\\*["'])?\s*[:=]\s*(?:\\*["'])?(?:basic|token|digest))\s+[^\s"',\\]+"#,
    )
    .unwrap()
});
static MASK_COOKIE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b((?:set-)?cookie(?:\\*["'])?\s*:\s*(?:\\*["'])?)[^\r\n"'\\]+"#).unwrap()
});
/// Password in URL userinfo: `scheme://user:PASS@host`.
static MASK_URL_USERINFO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)([a-z][a-z0-9+.-]*://[^\s:/@"'\\]+:)[^\s/@"'\\]+@"#).unwrap()
});
/// Well-known credential formats: GitHub, Slack, AWS access key id, JWT.
static MASK_TOKEN_FORMATS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[abposr]-[A-Za-z0-9-]{10,}|(?:AKIA|ASIA)[0-9A-Z]{16}|eyJ[A-Za-z0-9_-]{5,}\.eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*)",
    )
    .unwrap()
});
/// PEM private key block; an unterminated (truncated) block runs to the end.
static MASK_PEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?(?:-----END [A-Z ]*PRIVATE KEY-----|\z)")
        .unwrap()
});

/// Every credential mask except the path mask.
fn mask_credentials(s: &str) -> String {
    let s = MASK_PEM.replace_all(s, "<REDACTED>");
    let s = MASK_BEARER.replace_all(&s, "Bearer <REDACTED>");
    let s = MASK_AUTH.replace_all(&s, "$1 <REDACTED>");
    let s = MASK_SK.replace_all(&s, "sk-<REDACTED>");
    let s = MASK_TOKEN_FORMATS.replace_all(&s, "<REDACTED>");
    let s = MASK_URL_USERINFO.replace_all(&s, "${1}<REDACTED>@");
    let s = MASK_COOKIE.replace_all(&s, "${1}<REDACTED>");
    let s = MASK_FLAG.replace_all(&s, "${1}<REDACTED>");
    MASK_KV.replace_all(&s, "$1=<REDACTED>").into_owned()
}

/// Redact secrets in a JSON value by structure: the value of any object key
/// matching the `MASK_KV` key set is replaced, and a string that itself parses
/// as a JSON object or array is redacted recursively and re-serialized. Text
/// regexes cannot see through the escaping of a JSON string nested in JSON.
pub fn mask_json_secrets(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    static SECRET_KEY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)password|passwd|token|api[_-]?key|secret|private[_-]?key").unwrap()
    });
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| {
                    let x = if SECRET_KEY.is_match(k) {
                        Value::String("<REDACTED>".into())
                    } else {
                        mask_json_secrets(x)
                    };
                    (k.clone(), x)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(mask_json_secrets).collect()),
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(inner @ (Value::Object(_) | Value::Array(_))) => {
                Value::String(mask_json_secrets(&inner).to_string())
            }
            _ => v.clone(),
        },
        _ => v.clone(),
    }
}

/// Absolute file paths — Unix (`/home/u/p/x.rs`), Windows (`C:\Users\me\y`),
/// and tilde-home (`~/repo/z`). Error snippets reach an external LLM via
/// skill synthesis and may land in generated SKILL.md, so paths are masked.
static MASK_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:(?:/[A-Za-z][\w./-]*)|(?:[A-Za-z]:\\[^\s"']+)|(?:~[/\w.\\-]+))"#).unwrap()
});

/// Sanitize a path that is itself an observation's `action`.
///
/// [`mask_secrets`] collapses every absolute path to one `<PATH>` token. That
/// is right for an error snippet, but destroys the action when the action *is*
/// a path: every file becomes indistinguishable, so same-file streak detection
/// (`long_debug_loop`), per-file weak-spot stats and the observation dedup key
/// all collapse onto a single value.
///
/// A rooted path (leading `/` or `\`, a drive prefix such as `C:`, or a UNC
/// share) under the project is made relative, which drops the part that
/// actually leaks (home directory, username) while keeping files distinct,
/// and then gets the secret masks but *not* the path mask, which would
/// re-collapse `src/a.rs` and `src/b.rs` onto `src<PATH>`. A rooted path
/// outside the project becomes the literal `<PATH>`. A path that is not
/// rooted (a Codex `apply_patch` names its files that way, `~/x` too) has no
/// home directory or username in it and only gets the secret masks.
pub fn mask_path_action(path: &str) -> String {
    let trimmed = path.trim();
    let tilde_user = trimmed
        .strip_prefix('~')
        .is_some_and(|r| !r.is_empty() && !r.starts_with('/'));
    if tilde_user {
        return "<PATH>".to_string();
    }
    if !is_rooted(trimmed) {
        return mask_credentials(path);
    }
    // A `..` can climb out of the project even when the prefix matches.
    if std::path::Path::new(trimmed)
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return "<PATH>".to_string();
    }
    match std::path::Path::new(trimmed).strip_prefix(crate::shared::paths::cwd()) {
        Ok(r) => mask_credentials(&r.to_string_lossy()),
        // `mask_secrets` stops at whitespace and misses UNC paths.
        Err(_) => "<PATH>".to_string(),
    }
}

/// True for a path that names a root, identically on every OS: a leading `/`
/// or `\` (POSIX, UNC, root-relative), a drive prefix (`C:\x`, `C:/x`,
/// `C:foo`), or whatever the host's `Path` parser calls a root or prefix.
fn is_rooted(path: &str) -> bool {
    use std::path::{Component, Path};
    let b = path.as_bytes();
    matches!(b.first(), Some(b'/' | b'\\'))
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
        || matches!(
            Path::new(path).components().next(),
            Some(Component::RootDir | Component::Prefix(_))
        )
}

/// Mask common secret patterns in a string.
/// Covers: Bearer tokens, sk-* API keys, password/token/apikey/secret/private_key
/// values, and absolute file paths (Unix/Windows/tilde-home).
pub fn mask_secrets(s: &str) -> String {
    MASK_PATH
        .replace_all(&mask_credentials(s), "<PATH>")
        .into_owned()
}

/// Most bytes `mask_snippet` scans. Tool output can run to many megabytes; a
/// secret that starts past this point cannot reach a snippet of a few hundred
/// bytes, so scanning the rest only costs hook latency.
const SNIPPET_SCAN_LIMIT: usize = 64 * 1024;

/// Mask, then truncate to `max` bytes, scanning at most `SNIPPET_SCAN_LIMIT`
/// bytes of `s`. Masking before the final cut keeps a secret split by the limit
/// from slipping past the patterns.
pub fn mask_snippet(s: &str, max: usize) -> String {
    use crate::shared::helpers::truncate_str;
    truncate_str(&mask_secrets(truncate_str(s, SNIPPET_SCAN_LIMIT)), max).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_snippet_bounds_scan_and_output() {
        let big = format!(
            "token=sk-abcdefghijklmnopqrstuvwx {}",
            "x".repeat(8 * 1024 * 1024)
        );
        let out = mask_snippet(&big, 500);
        assert!(out.len() <= 500);
        assert!(!out.contains("sk-abcdefghijklmnopqrstuvwx"), "{out}");
    }

    /// Two files in the project must stay distinguishable after sanitizing.
    /// Masking an absolute path alone turns every one of them into `<PATH>`,
    /// which silently breaks same-file streak detection and the dedup key.
    #[test]
    fn project_paths_stay_distinct_after_masking() {
        let root = crate::shared::paths::cwd();
        let a = root.join("src/a.rs");
        let b = root.join("src/b.rs");

        let sa = mask_path_action(&a.to_string_lossy());
        let sb = mask_path_action(&b.to_string_lossy());

        assert_eq!(sa, "src/a.rs");
        assert_eq!(sb, "src/b.rs");
        assert_ne!(sa, sb);
    }

    /// A path outside the project keeps the mask: nothing about the user's
    /// home directory should survive into stored observations.
    #[test]
    fn outside_paths_are_still_masked() {
        // The masked value is deliberately not interpolated into the failure
        // message: printing the output of a masking function trips CodeQL's
        // cleartext-logging rule, and the assertion is diagnostic without it.
        let masked = mask_path_action("/home/someone/secret/x.rs");
        assert!(!masked.contains("someone"), "home dir survived masking");
        assert!(masked.contains("<PATH>"), "outside path was not masked");
    }

    #[test]
    fn rooted_outside_paths_become_path_token() {
        for p in [
            "/home/u/a b/x.rs",
            r"\\server\share\x",
            r"\Users\u\x.rs",
            r"C:\Users\u\x.rs",
            r"C:foo\bar",
        ] {
            assert_eq!(mask_path_action(p), "<PATH>");
        }
    }

    #[test]
    fn non_rooted_paths_are_unchanged() {
        for p in ["src/a.rs", "./src/a.rs", "../x/y.rs"] {
            assert_eq!(mask_path_action(p), p);
        }
    }

    #[test]
    fn cwd_join_becomes_relative() {
        let p = crate::shared::paths::cwd().join("src/a.rs");
        assert_eq!(mask_path_action(&p.to_string_lossy()), "src/a.rs");
    }

    #[test]
    fn outside_cwd_absolute_paths_are_exactly_path_token() {
        for p in [
            "/home/u/proj/x.rs",
            "/home/u/my proj/x.rs",
            r"\\server\share\dir\x.rs",
            r"\\server\share name\x.rs",
            r"D:\work dir\x.rs",
            "C:/Users/u/x.rs",
            " /home/u/x.rs",
            "/home/u/x.rs\n",
            "~user/x.rs",
            "~user",
            "/home/u/../../etc/x.rs",
        ] {
            assert_eq!(mask_path_action(p), "<PATH>", "{p:?}");
        }
    }

    #[test]
    fn escaping_path_under_cwd_is_path_token() {
        let p = format!(
            "{}/../../home/u/x.rs",
            crate::shared::paths::cwd().display()
        );
        assert_eq!(mask_path_action(&p), "<PATH>");
    }

    #[test]
    fn plain_tilde_and_relative_paths_stay_readable() {
        assert_eq!(mask_path_action("~/x.rs"), "~/x.rs");
        assert_eq!(mask_path_action("src/a.rs"), "src/a.rs");
    }

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
    fn mask_json_quoted_key_and_value() {
        assert_eq!(
            mask_secrets(r#"{"token":"abc def","n":1}"#),
            "{token=<REDACTED>,\"n\":1}"
        );
        assert_eq!(mask_secrets(r#"{"api_key": "x"}"#), "{api_key=<REDACTED>}");
        assert_eq!(
            mask_secrets(r#"{"password":"a \"b\" c"}"#),
            "{password=<REDACTED>}"
        );
    }

    #[test]
    fn mask_json_value_redacts_nested_and_embedded_json() {
        let v = serde_json::json!({
            "n": 1,
            "auth": {"api_key": "SYNTHETICA SYNTHETICB"},
            "list": [{"password": "SYNTHETICC SYNTHETICD"}],
            "payload": r#"{"token":"SYNTHETICE SYNTHETICF","ok":1}"#,
        });
        let out = mask_json_secrets(&v).to_string();
        assert!(!out.contains("SYNTHETIC"));
        assert!(out.contains("\"n\":1"));
        assert!(out.contains("ok"));
    }

    #[test]
    fn mask_escaped_nested_json_in_plain_text() {
        let nested = r#"{"payload":"{\"token\":\"SYN A B\"}"}"#;
        let out = mask_secrets(nested);
        assert!(!out.contains("SYN") && !out.contains(" B"), "{out}");
        assert!(out.contains("<REDACTED>"));
        let prompt = r#"{"prompt":"password=\"SYN A B\""}"#;
        let out = mask_secrets(prompt);
        assert!(!out.contains("SYN") && !out.contains(" B"), "{out}");
        // Twice-escaped (JSON inside JSON inside JSON).
        let deep = r#"{"a":"{\"b\":\"{\\\"token\\\":\\\"SYN A B\\\"}\"}"}"#;
        let out = mask_secrets(deep);
        assert!(!out.contains("SYN") && !out.contains(" B"), "{out}");
    }

    #[test]
    fn mask_unterminated_quoted_value_to_end_of_line() {
        let out = mask_secrets("{\"token\":\"SYN A B\nnext line stays");
        assert!(!out.contains("SYN") && !out.contains(" B"), "{out}");
        assert!(out.contains("next line stays"));
        let out = mask_secrets("password='SYN A B");
        assert!(!out.contains("SYN") && !out.contains(" B"), "{out}");
    }

    #[test]
    fn mask_authorization_schemes() {
        for h in [
            "Authorization: Basic c3ludGhldGljOnB3",
            "Authorization: Bearer SYNTHETICVALUE",
            "Authorization: token SYNTHETICVALUE",
            r#"{"Authorization":"Basic SYNTHETICVALUE"}"#,
        ] {
            let out = mask_secrets(h);
            assert!(
                !out.contains("c3ludGhldGljOnB3") && !out.contains("SYNTHETICVALUE"),
                "{out}"
            );
            assert!(out.contains("<REDACTED>"));
        }
    }

    #[test]
    fn mask_hyphenated_and_prefixed_keys() {
        for h in [
            "x-api-key: SYNTHETICVALUE",
            "api-key=SYNTHETICVALUE",
            "GET https://example.test/v1?api-key=SYNTHETICVALUE",
            "AWS_SECRET_ACCESS_KEY=SYNTHETICVALUE",
            "GITHUB_TOKEN=SYNTHETICVALUE",
            "client_secret: SYNTHETICVALUE",
        ] {
            let out = mask_secrets(h);
            assert!(!out.contains("SYNTHETICVALUE"), "{out}");
            assert!(out.contains("<REDACTED>"));
        }
    }

    #[test]
    fn mask_url_userinfo_password() {
        let url = "git clone https://synthuser:SYNTHETICPASS@host.test/repo.git";
        assert!(!mask_secrets(url).contains("SYNTHETICPASS"));
        // The path mask would also hide the user name; check the credential mask alone.
        assert_eq!(
            mask_credentials(url),
            "git clone https://synthuser:<REDACTED>@host.test/repo.git"
        );
    }

    #[test]
    fn mask_cookie_header() {
        let out = mask_secrets("Cookie: sid=SYNTHETICVALUE; theme=dark\nHost: x");
        assert!(
            !out.contains("SYNTHETICVALUE") && !out.contains("theme"),
            "{out}"
        );
        assert!(out.contains("Host: x"));
        let out = mask_secrets("Set-Cookie: sid=SYNTHETICVALUE; HttpOnly");
        assert!(!out.contains("SYNTHETICVALUE"), "{out}");
    }

    #[test]
    fn mask_cli_flag_values() {
        for h in [
            "deploy --password SYNTHETICVALUE --force",
            "deploy --token SYNTHETICVALUE",
            "deploy --password \"SYN A B\" --force",
            "deploy --api-key SYNTHETICVALUE",
        ] {
            let out = mask_secrets(h);
            assert!(
                !out.contains("SYNTHETICVALUE") && !out.contains("SYN A") && !out.contains(" B"),
                "{out}"
            );
        }
        let out = mask_secrets("deploy --password --force");
        assert!(out.contains("--force"));
    }

    #[test]
    fn mask_well_known_token_formats() {
        for t in [
            "ghp_SYNTHETIC0123456789ABCDEFGHIJKLMN",
            "gho_SYNTHETIC0123456789ABCDEFGHIJKLMN",
            "github_pat_SYNTHETIC0123456789ABCDEFGHIJKLMN",
            concat!("xo", "xb-123456789012-SYNTHETICVALUE"),
            concat!("xo", "xp-123456789012-SYNTHETICVALUE"),
            "AKIAABCDEFGHIJKLMNOP",
            "eyJhbGciOiJub25lIn0.eyJzdWIiOiJzeW50aGV0aWMifQ.c2lnbmF0dXJl",
        ] {
            let out = mask_secrets(&format!("saw {t} in output"));
            assert_eq!(out, "saw <REDACTED> in output");
        }
    }

    #[test]
    fn mask_pem_private_key_block() {
        let pem = "before\n-----BEGIN RSA PRIVATE KEY-----\nSYNTHETICLINE1\nSYNTHETICLINE2\n-----END RSA PRIVATE KEY-----\nafter";
        assert_eq!(mask_secrets(pem), "before\n<REDACTED>\nafter");
        let cut = "-----BEGIN PRIVATE KEY-----\nSYNTHETICLINE1\nSYNTHETICLI";
        assert_eq!(mask_secrets(cut), "<REDACTED>");
    }

    #[test]
    fn ordinary_words_are_not_masked() {
        for t in [
            "the token budget was exceeded",
            "cookie banner shown to the user",
            "tokenization of secrets is hard",
            "ssh://git@host.test/repo.git",
            "http://host.test:8080/path",
            "deploy --token-file path.txt",
            "max_tokens: 5",
            "see eyJ docs and ghp_short",
        ] {
            assert_eq!(mask_credentials(t), t);
        }
    }

    #[test]
    fn mask_single_quoted_value_with_spaces() {
        assert_eq!(
            mask_secrets("secret='two words' ok"),
            "secret=<REDACTED> ok"
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
}
