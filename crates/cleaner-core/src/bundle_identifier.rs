pub fn is_safe_bundle_identifier(identifier: &str) -> bool {
    if identifier.is_empty() || identifier.starts_with('.') || identifier.ends_with('.') {
        return false;
    }

    identifier.split('.').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    })
}

pub fn is_apple_bundle_identifier(identifier: &str) -> bool {
    identifier == "com.apple" || identifier.starts_with("com.apple.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_bundle_identifier_rejects_path_escape_shapes() {
        for identifier in [
            "",
            "..",
            "/",
            "com/example/app",
            "com..example",
            ".com.example",
            "com.example.",
        ] {
            assert!(!is_safe_bundle_identifier(identifier), "{identifier}");
        }

        assert!(is_safe_bundle_identifier("com.example.app"));
        assert!(is_safe_bundle_identifier("com.example-app_2"));
    }

    #[test]
    fn apple_namespace_requires_exact_component_boundary() {
        assert!(is_apple_bundle_identifier("com.apple"));
        assert!(is_apple_bundle_identifier("com.apple.Safari"));
        assert!(!is_apple_bundle_identifier("com.appleish.fake"));
    }
}
