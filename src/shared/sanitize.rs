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
