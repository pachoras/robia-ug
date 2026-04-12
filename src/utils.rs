use sha2::{Digest, Sha256};
use std::io::{self};

use rand::distr::{Alphanumeric, SampleString};

/// Hashes a file using SHA-256 and returns the hash as a hexadecimal string.
pub fn hash_string(path: &str) -> io::Result<String> {
    // Source - https://stackoverflow.com/a/69787984
    // Posted by cameron1024, modified by community. See post 'Timeline' for change history
    // Retrieved 2026-03-15, License - CC BY-SA 4.0

    let bytes = std::fs::read(path).unwrap(); // Vec<u8>
    let hash = Sha256::digest(&bytes);

    Ok(format!("{:x}", hash))
}

/// The path to the CSS file. This is used as a constant for generating the cache-busted path.
pub static CSS_PATH: &str = "/static/css/styles.css";

// Generate a unique path for the CSS file using its content hash to enable cache busting
pub fn generate_cache_busted_css_path() -> io::Result<String> {
    let css_hash = hash_string("src/static/css/styles.css").unwrap();
    let static_css_path = format!("{}?v={}", CSS_PATH, css_hash);
    Ok(static_css_path)
}

// Generate a unique path for the JavaScript file using its content hash to enable cache busting
pub fn generate_cache_busted_js_path() -> io::Result<String> {
    let js_hash = hash_string("src/static/js/main.js").unwrap();
    let static_js_path = format!("/static/js/main.js?v={}", js_hash);
    Ok(static_js_path)
}

// Source - https://stackoverflow.com/a/72977937
// Posted by Mari, modified by community. See post 'Timeline' for change history
// Retrieved 2026-04-04, License - CC BY-SA 4.0

/// Generates a random alphanumeric string of the specified length.
pub fn generate_random_string(length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), length)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_string_returns_64_char_hex_for_valid_file() {
        let result = hash_string("Cargo.toml");
        assert!(result.is_ok());
        let hash = result.unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    #[should_panic]
    fn hash_string_panics_for_missing_file() {
        hash_string("nonexistent_file_that_does_not_exist.txt").unwrap();
    }

    #[test]
    fn css_path_constant_value() {
        assert_eq!(CSS_PATH, "/static/css/styles.css");
    }

    #[test]
    fn generate_cache_busted_css_path_has_correct_format() {
        let result = generate_cache_busted_css_path();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.starts_with("/static/css/styles.css?v="));
        // SHA-256 hex digest is always 64 characters
        let version = path.split("?v=").nth(1).unwrap();
        assert_eq!(version.len(), 64);
        assert!(version.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_cache_busted_css_path_is_deterministic() {
        let path1 = generate_cache_busted_css_path().unwrap();
        let path2 = generate_cache_busted_css_path().unwrap();
        assert_eq!(path1, path2);
    }
}
