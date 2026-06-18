// Lightweight router facade for NyxEdit.
// Provides RouteClass, ExecutionMode, RouterDecision and a builder that
// includes simple privacy detection. This is intentionally conservative
// and avoids adding new crate dependencies so it compiles in-place.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteClass {
    Local,
    Cheap,
    Balanced,
    Premium,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionMode {
    Single,
    Chain,
    Dag,
}

#[derive(Debug, Clone)]
pub struct RouterDecision {
    pub class: RouteClass,
    pub mode: ExecutionMode,
    pub is_sensitive: bool,
    pub reason: String,
}

impl RouterDecision {
    /// Build a RouterDecision from high-level inputs.
    ///
    /// - `intent` is a short intent name, e.g. "ExplainSimple", "CodeWrite", etc.
    /// - `token_estimate` is an approximate token usage for selecting execution mode.
    /// - `attachments` is an optional list of attachment file names to check for sensitive filenames.
    /// - `message_content` is the user message body (scanned for secrets patterns).
    /// - `workspace_privacy_mode` can be "strict", "standard", or "disabled".
    pub fn build(
        intent: &str,
        token_estimate: usize,
        attachments: Option<&[String]>,
        message_content: &str,
        workspace_privacy_mode: &str,
    ) -> Self {
        // Determine execution mode by token thresholds
        let mode = if token_estimate > 500_000 {
            ExecutionMode::Dag
        } else if token_estimate >= 50_000 {
            ExecutionMode::Chain
        } else {
            ExecutionMode::Single
        };

        // Map intent to RouteClass heuristically
        let class = match intent {
            "ExplainSimple" => RouteClass::Cheap,
            "ScanOnly" | "SymbolLookup" => RouteClass::Local,
            "CodeWrite" | "TestGenerate" => RouteClass::Balanced,
            "CodeReview" | "DebugLogic" => RouteClass::Premium,
            "RefactorFull" | "ArchDesign" => {
                // Large refactors lean premium and may use Chain/DAG
                RouteClass::Premium
            }
            _ => RouteClass::Balanced,
        };

        // Privacy detection
        let mut is_sensitive = false;
        let mut reasons: Vec<String> = Vec::new();

        // 1) Attachment name patterns
        if let Some(files) = attachments {
            for f in files {
                if Self::sensitive_filename(f) {
                    is_sensitive = true;
                    reasons.push(format!("sensitive filename: {}", f));
                }
            }
        }

        // 2) Message content scan for common key patterns
        if Self::contains_sensitive_content(message_content) {
            is_sensitive = true;
            reasons.push("sensitive content detected in message".into());
        }

        // 3) Workspace privacy mode overrides
        match workspace_privacy_mode {
            "strict" => {
                if !is_sensitive {
                    // strict mode prefers local by default for safety
                    reasons.push("workspace privacy: strict".into());
                }
                is_sensitive = true;
            }
            "disabled" => {
                // do nothing
            }
            _ => {
                // standard: no-op
            }
        }

        // If sensitive, force Local class
        let final_class = if is_sensitive { RouteClass::Local } else { class };

        let reason = if reasons.is_empty() {
            format!("mapped intent '{}' to class {:?}", intent, final_class)
        } else {
            format!("{}; mapped intent '{}' to class {:?}", reasons.join("; "), intent, final_class)
        };

        RouterDecision {
            class: final_class,
            mode,
            is_sensitive,
            reason,
        }
    }

    fn sensitive_filename(name: &str) -> bool {
        let lower = name.to_lowercase();
        // common sensitive patterns
        let patterns = [
            ".env",
            ".pem",
            ".key",
            ".p12",
            ".pfx",
            "id_rsa",
            "id_ed25519",
            "secret",
            "credential",
            "token",
            "password",
        ];
        for p in &patterns {
            if lower.ends_with(p) || lower.contains(p) || lower == *p {
                return true;
            }
        }
        false
    }

    fn contains_sensitive_content(s: &str) -> bool {
        let lower = s.to_lowercase();
        let patterns = ["sk-", "bearer ", "ghp_", "akia", "AIzaSy", "password", "api_key", "private_key", "secret"];
        for p in &patterns {
            if lower.contains(&p.to_lowercase()) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_intent_mapping() {
        let d = RouterDecision::build("ExplainSimple", 1000, None, "hello", "disabled");
        assert_eq!(d.class, RouteClass::Cheap);
        assert_eq!(d.mode, ExecutionMode::Single);
    }

    #[test]
    fn test_sensitive_filename_triggers_local() {
        let files = vec![".env".to_string(), "notes.txt".to_string()];
        let d = RouterDecision::build("CodeWrite", 1000, Some(&files), "no-secrets", "disabled");
        assert!(d.is_sensitive);
        assert_eq!(d.class, RouteClass::Local);
    }

    #[test]
    fn test_token_thresholds() {
        let d = RouterDecision::build("RefactorFull", 600_000, None, "", "disabled");
        assert_eq!(d.mode, ExecutionMode::Dag);
    }

    #[test]
    fn test_workspace_privacy_strict_forces_local() {
        let d = RouterDecision::build("ExplainSimple", 1000, None, "hello world", "strict");
        assert!(d.is_sensitive);
        assert_eq!(d.class, RouteClass::Local);
        assert!(d.reason.contains("workspace privacy: strict"));
    }

    #[test]
    fn test_message_content_sensitive_triggers_local() {
        let d = RouterDecision::build("CodeWrite", 1000, None, "here is my key sk-ABC123", "disabled");
        assert!(d.is_sensitive);
        assert_eq!(d.class, RouteClass::Local);
        assert!(d.reason.contains("sensitive content detected"));
    }
}
