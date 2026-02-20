
/// Helper for allocation-free case-insensitive containment check.
fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    // Fast path for ASCII
    if haystack.is_ascii() && needle_lower.is_ascii() {
        let haystack_bytes = haystack.as_bytes();
        let needle_bytes = needle_lower.as_bytes();
        if needle_bytes.len() > haystack_bytes.len() {
            return false;
        }
        // Naive byte-by-byte scan is fast enough for short strings (UI labels)
        for i in 0..=haystack_bytes.len() - needle_bytes.len() {
            let mut match_found = true;
            for (j, &b) in needle_bytes.iter().enumerate() {
                if haystack_bytes[i + j].to_ascii_lowercase() != b {
                    match_found = false;
                    break;
                }
            }
            if match_found {
                return true;
            }
        }
        return false;
    }
    // Fallback for Unicode (allocates, but correct)
    haystack.to_lowercase().contains(needle_lower)
}
