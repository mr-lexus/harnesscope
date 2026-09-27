use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // OpenAI api key
        Regex::new(r"sk-[a-zA-Z0-9_\-]{20,}").unwrap(),
        // Anthropic api key
        Regex::new(r"sk-ant-[a-zA-Z0-9_\-]{20,}").unwrap(),
        // GitHub personal access tokens
        Regex::new(r"ghp_[a-zA-Z0-9]{30,}").unwrap(),
        Regex::new(r"github_pat_[a-zA-Z0-9_]{30,}").unwrap(),
        Regex::new(r"gho_[a-zA-Z0-9]{30,}").unwrap(),
        Regex::new(r"ghs_[a-zA-Z0-9]{30,}").unwrap(),
        // Bearer tokens
        Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.]{20,}").unwrap(),
        // Generic JSON key-values for secrets
        Regex::new(r#"(?i)"(api_?key|token|access_?token|secret|password|private_?key)"\s*:\s*"([^"]+)""#).unwrap(),
        // Generic CLI arg tokens: --api-key=xyz, --token xyz
        Regex::new(r#"(?i)(--?(?:api[-_]?key|token|secret|password)[=\s])([^\s"']+)"#).unwrap(),
        // URL with user:password
        Regex::new(r"https?://([^:\s]+):([^@\s]+)@").unwrap(),
    ]
});

pub fn redact_secrets(input: &str) -> String {
    let mut result = input.to_string();

    // Redact specific known token prefixes
    for (i, re) in SECRET_PATTERNS.iter().enumerate() {
        if i == 7 {
            // JSON key-values
            result = re.replace_all(&result, |caps: &regex::Captures| {
                format!(r#""{}": "[REDACTED_SECRET]""#, &caps[1])
            }).to_string();
        } else if i == 8 {
            // CLI arg tokens
            result = re.replace_all(&result, |caps: &regex::Captures| {
                format!("{}[REDACTED_SECRET]", &caps[1])
            }).to_string();
        } else if i == 9 {
            // URL credentials
            result = re.replace_all(&result, |caps: &regex::Captures| {
                format!("https://{}:[REDACTED_SECRET]@", &caps[1])
            }).to_string();
        } else {
            result = re.replace_all(&result, "[REDACTED_SECRET]").to_string();
        }
    }

    result
}

pub fn sha256_digest(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_openai_key() {
        let raw = "run --model gpt-4o with sk-1234567890abcdef1234567890 in command";
        let redacted = redact_secrets(raw);
        assert!(!redacted.contains("sk-1234567890abcdef1234567890"));
        assert!(redacted.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn test_redact_github_pat() {
        let raw = "curl -H 'Authorization: token ghp_123456789012345678901234567890123456'";
        let redacted = redact_secrets(raw);
        assert!(!redacted.contains("ghp_123456789012345678901234567890123456"));
        assert!(redacted.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn test_redact_json_secret() {
        let raw = r#"{"apiKey": "super_secret_value_12345", "model": "claude-3-5-sonnet"}"#;
        let redacted = redact_secrets(raw);
        assert!(!redacted.contains("super_secret_value_12345"));
        assert!(redacted.contains(r#""apiKey": "[REDACTED_SECRET]""#));
        assert!(redacted.contains(r#""model": "claude-3-5-sonnet""#));
    }

    #[test]
    fn test_sha256_digest() {
        let hash1 = sha256_digest("test content");
        let hash2 = sha256_digest("test content");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }
}
