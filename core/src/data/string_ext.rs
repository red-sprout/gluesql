use {
    crate::result::Result,
    regex::{Regex, RegexBuilder},
    serde::Serialize,
    thiserror::Error,
};

const REGEX_CACHE_CAPACITY: usize = 32;
const MAX_CACHED_REGEX_SOURCE_BYTES: usize = 4 * 1024;

#[derive(PartialEq, Eq)]
struct RegexCacheKey {
    source: String,
    case_insensitive: bool,
}

struct RegexCacheEntry {
    key: RegexCacheKey,
    regex: Regex,
}

pub(crate) struct RegexCache {
    entries: Vec<RegexCacheEntry>,
}

impl RegexCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    fn regex(
        &mut self,
        source: String,
        case_insensitive: bool,
        error: impl FnOnce(regex::Error) -> StringExtError,
    ) -> Result<Regex> {
        let key = RegexCacheKey {
            source,
            case_insensitive,
        };

        if key.source.len() <= MAX_CACHED_REGEX_SOURCE_BYTES
            && let Some(index) = self.entries.iter().position(|entry| entry.key == key)
        {
            let entry = self.entries.remove(index);
            self.entries.insert(0, entry);
            return Ok(self.entries[0].regex.clone());
        }

        let regex = RegexBuilder::new(&key.source)
            .case_insensitive(key.case_insensitive)
            .build()
            .map_err(error)?;

        if key.source.len() <= MAX_CACHED_REGEX_SOURCE_BYTES {
            if self.entries.len() == REGEX_CACHE_CAPACITY {
                self.entries.pop();
            }
            self.entries.insert(
                0,
                RegexCacheEntry {
                    key,
                    regex: regex.clone(),
                },
            );
        }

        Ok(regex)
    }
}

pub(crate) fn like_with_cache(
    value: &str,
    pattern: &str,
    case_sensitive: bool,
    cache: &mut RegexCache,
) -> Result<bool> {
    let (value, pattern) = if case_sensitive {
        (value.to_owned(), pattern.to_owned())
    } else {
        (value.to_lowercase(), pattern.to_lowercase())
    };
    let source = format!(
        "^{}$",
        regex::escape(&pattern).replace('%', ".*").replace('_', ".")
    );

    match_with_cache(&value, &source, false, cache, |_| {
        StringExtError::UnreachablePatternParsing
    })
}

pub(crate) fn regex_with_cache(
    value: &str,
    pattern: &str,
    case_sensitive: bool,
    cache: &mut RegexCache,
) -> Result<bool> {
    match_with_cache(value, pattern, !case_sensitive, cache, |error| {
        StringExtError::InvalidRegexPattern {
            pattern: pattern.to_owned(),
            error: error.to_string(),
        }
    })
}

fn match_with_cache(
    value: &str,
    source: &str,
    case_insensitive: bool,
    cache: &mut RegexCache,
    error: impl FnOnce(regex::Error) -> StringExtError,
) -> Result<bool> {
    cache
        .regex(source.to_owned(), case_insensitive, error)
        .map(|regex| regex.is_match(value))
}

#[derive(Error, Serialize, Debug, PartialEq, Eq)]
pub enum StringExtError {
    #[error("unreachable literal unary operation")]
    UnreachablePatternParsing,
    #[error("invalid regular expression {pattern:?}: {error}")]
    InvalidRegexPattern { pattern: String, error: String },
}

pub trait StringExt {
    fn like(&self, pattern: &str, case_sensitive: bool) -> Result<bool>;
    fn regex(&self, pattern: &str, case_sensitive: bool) -> Result<bool>;
}

impl StringExt for str {
    fn like(&self, pattern: &str, case_sensitive: bool) -> Result<bool> {
        let (match_string, match_pattern) = if case_sensitive {
            (self.to_owned(), pattern.to_owned())
        } else {
            let lowercase_string = self.to_lowercase();
            let lowercase_pattern = pattern.to_lowercase();

            (lowercase_string, lowercase_pattern)
        };

        match_with_regex(
            match_string.as_str(),
            &format!(
                "^{}$",
                regex::escape(match_pattern.as_str())
                    .replace('%', ".*")
                    .replace('_', ".")
            ),
            false,
            |_| StringExtError::UnreachablePatternParsing,
        )
    }

    fn regex(&self, pattern: &str, case_sensitive: bool) -> Result<bool> {
        match_with_regex(self, pattern, !case_sensitive, |error| {
            StringExtError::InvalidRegexPattern {
                pattern: pattern.to_owned(),
                error: error.to_string(),
            }
        })
    }
}

fn match_with_regex(
    value: &str,
    source: &str,
    case_insensitive: bool,
    error: impl FnOnce(regex::Error) -> StringExtError,
) -> Result<bool> {
    Ok(RegexBuilder::new(source)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(error)?
        .is_match(value))
}

#[cfg(test)]
mod tests {
    use {
        super::{
            REGEX_CACHE_CAPACITY, RegexCache, StringExt, StringExtError, like_with_cache,
            regex_with_cache,
        },
        crate::result::Error,
    };

    #[test]
    fn regex_cache_reuses_entries_and_separates_case_options() {
        let mut cache = RegexCache::new();

        assert_eq!(
            regex_with_cache("Hello", "^hello$", true, &mut cache),
            Ok(false)
        );
        assert_eq!(
            regex_with_cache("Hello", "^hello$", false, &mut cache),
            Ok(true)
        );
        assert_eq!(cache.len(), 2);
        assert_eq!(
            regex_with_cache("Hello", "^hello$", false, &mut cache),
            Ok(true)
        );
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn regex_cache_does_not_store_invalid_or_long_patterns() {
        let mut cache = RegexCache::new();

        assert!(regex_with_cache("Hello", "[", true, &mut cache).is_err());
        assert_eq!(cache.len(), 0);

        let long_pattern = "a".repeat(4097);
        assert_eq!(
            regex_with_cache("Hello", &long_pattern, true, &mut cache),
            Ok(false)
        );
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn like_cache_normalizes_pattern_before_matching() {
        let mut cache = RegexCache::new();

        assert_eq!(like_with_cache("Hello", "h%", false, &mut cache), Ok(true));
        assert_eq!(
            like_with_cache("Hello", "h_llo", false, &mut cache),
            Ok(true)
        );
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn like_and_regex_keep_distinct_cache_keys() {
        let mut cache = RegexCache::new();

        like_with_cache("abc", "a%", true, &mut cache).unwrap();
        regex_with_cache("abc", "a%", true, &mut cache).unwrap();

        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn regex_cache_evicts_when_full() {
        let mut cache = RegexCache::new();

        for index in 0..=REGEX_CACHE_CAPACITY {
            let pattern = format!("^{index}$");
            regex_with_cache("0", &pattern, true, &mut cache).unwrap();
        }

        assert_eq!(cache.len(), REGEX_CACHE_CAPACITY);
    }

    #[test]
    fn regex() {
        assert_eq!("Hello".regex("ell", true), Ok(true));
        assert_eq!("Hello".regex("^hello$", true), Ok(false));
        assert_eq!("Hello".regex("^hello$", false), Ok(true));
        assert!(matches!(
            "Hello".regex("[", true),
            Err(Error::StringExt(StringExtError::InvalidRegexPattern { pattern, .. })) if pattern == "["
        ));
        assert!(matches!(
            "Hello".regex("(?i)[", true),
            Err(Error::StringExt(StringExtError::InvalidRegexPattern { pattern, .. })) if pattern == "(?i)["
        ));
        assert!(matches!(
            "Hello".regex("[", false),
            Err(Error::StringExt(StringExtError::InvalidRegexPattern { pattern, .. })) if pattern == "["
        ));

        // the case-insensitive flag must not leak into the reported parse error
        assert!(matches!(
            "Hello".regex("[", false),
            Err(Error::StringExt(StringExtError::InvalidRegexPattern { error, .. }))
                if !error.contains("(?i)")
        ));
    }
}
