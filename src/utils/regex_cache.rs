use regex::{Captures, Regex};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::{Arc, RwLock};

static REGEX_CACHE: LazyLock<RwLock<RegexCache>> = LazyLock::new(|| RwLock::new(RegexCache::default()));

#[derive(Default)]
pub struct RegexCache {
    patterns: HashMap<String, Arc<Regex>>,
}

impl RegexCache {
    fn get(pattern: &str) -> Arc<Regex> {
        let regex = REGEX_CACHE.read().unwrap().patterns.get(pattern).cloned();
        match regex {
            Some(regex) => regex,
            None => {
                let regex = Arc::new(Regex::new(pattern).unwrap());
                REGEX_CACHE.write().unwrap().patterns.insert(pattern.to_string(), regex.clone());
                regex
            }
        }
    }

    pub fn custom_match(pattern: &str, text: &str) -> bool {
        for pattern in pattern.split("||") {
            if let Some(pattern) = pattern.strip_prefix("#") {
                if Self::get(&format!("(?i){pattern}")).is_match(text) {
                    return true;
                }
                continue;
            }

            let mut matched = false;
            for part in pattern.split("&") {
                let (part, negate) = match part.strip_prefix("!=") {
                    Some(p) => (p, true),
                    None => (part, false),
                };
                let pattern = format!("(?i)\\b(?:{part})\\b");
                let matches = Self::get(&pattern).is_match(text);
                if matches == negate {
                    matched = false;
                    break;
                } else {
                    matched = true;
                }
            }

            if matched {
                return true;
            }
        }

        false
    }

    pub fn matches(pattern: &str, text: &str) -> bool {
        Self::get(pattern).is_match(text)
    }

    pub fn captures<'a>(pattern: &str, text: &'a str, index: usize) -> Option<&'a str> {
        Self::get(pattern).captures(text).and_then(|v| v.get(index)).map(|s| s.as_str())
    }

    pub fn find_starts(pattern: &str, text: &str) -> Vec<usize> {
        Self::get(pattern).find_iter(text).map(|m| m.start()).collect()
    }

    pub fn captures_iter<'a>(pattern: &str, text: &'a str) -> Vec<Captures<'a>> {
        Self::get(pattern).captures_iter(text).collect()
    }

    pub fn replace_all<'a>(pattern: &str, text: &'a str, rep: &'a str) -> Cow<'a, str> {
        Self::get(pattern).replace_all(text, rep)
    }
}
