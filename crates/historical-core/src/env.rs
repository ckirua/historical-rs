//! Environment helpers.

/// Return the first non-empty trimmed value among `keys`.
pub fn env_first(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_first_set_key() {
        std::env::set_var("HIST_CORE_TEST_A", "first");
        std::env::set_var("HIST_CORE_TEST_B", "second");
        assert_eq!(
            env_first(&["HIST_CORE_TEST_A", "HIST_CORE_TEST_B"]),
            Some("first".into())
        );
        std::env::remove_var("HIST_CORE_TEST_A");
        std::env::remove_var("HIST_CORE_TEST_B");
    }
}
