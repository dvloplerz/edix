//! Fuzzy string matching using Levenshtein distance

use strsim::{levenshtein, normalized_levenshtein};

/// Calculate similarity between two strings (0.0 to 1.0, where 1.0 = identical)
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a_norm = a.trim().to_lowercase();
    let b_norm = b.trim().to_lowercase();
    normalized_levenshtein(&a_norm, &b_norm)
}

/// Calculate Levenshtein distance between two strings
pub fn distance(a: &str, b: &str) -> usize {
    levenshtein(&a.to_lowercase(), &b.to_lowercase())
}

/// Check if two strings are a fuzzy match above a threshold
pub fn is_fuzzy_match(a: &str, b: &str, threshold: f64) -> bool {
    similarity(a, b) >= threshold
}

/// Find the best match in a list of candidates
pub fn find_best_match<'a>(target: &str, candidates: &'a [&str]) -> Option<(usize, &'a str, f64)> {
    let mut best_idx = 0;
    let mut best_score = 0.0;

    for (i, candidate) in candidates.iter().enumerate() {
        let score = similarity(target, candidate);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    if best_score > 0.0 {
        Some((best_idx, candidates[best_idx], best_score))
    } else {
        None
    }
}

/// Batch fuzzy matching for column values
/// Returns positions of potential matches that are above threshold but not exact
pub fn find_fuzzy_mismatches(
    values_a: &[&str],
    values_b: &[&str],
    threshold: f64,
) -> Vec<(usize, usize, f64)> {
    let mut results = Vec::new();

    for (i, a) in values_a.iter().enumerate() {
        for (j, b) in values_b.iter().enumerate() {
            let sim = similarity(a, b);
            if sim < 1.0 && sim >= threshold {
                results.push((i, j, sim));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert_eq!(similarity("hello", "hello"), 1.0);
    }

    #[test]
    fn test_similar_strings() {
        let sim = similarity("hello", "helo");
        assert!(sim > 0.8, "similarity should be > 0.8 for 'hello' vs 'helo'");
    }

    #[test]
    fn test_different_strings() {
        let sim = similarity("hello", "world");
        assert!(sim < 0.5, "similarity should be low for 'hello' vs 'world'");
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(similarity("Hello", "hello"), 1.0);
    }

    #[test]
    fn test_best_match() {
        let candidates = vec!["apple", "banana", "aple"];
        let result = find_best_match("apple", &candidates);
        assert!(result.is_some());
        let (_, matched, score) = result.unwrap();
        assert_eq!(matched, "apple");
        assert!(score > 0.9);
    }
}
