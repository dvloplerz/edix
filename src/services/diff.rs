//! Core comparison engine for edix
//!
//! Supports:
//! - Key-based joins (single or multi-column keys)
//! - Type coercion and mismatch detection
//! - Fuzzy matching with similarity scores
//! - Multi-column comparison
//! - Missing row detection (in source or compare)

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::fuzzy;
use super::types::*;

/// Compare two sheets and return detailed results
pub fn compare_sheets(
    source: &SheetData,
    compare: &SheetData,
    key_columns: &[String],
    compare_columns: &[String],
    match_type: MatchType,
) -> ComparisonResult {
    let start = Instant::now();

    // Validate key columns exist in both sheets
    for col in key_columns {
        if !source.headers.contains(col) {
            eprintln!("Warning: key column '{}' not found in source", col);
        }
        if !compare.headers.contains(col) {
            eprintln!("Warning: key column '{}' not found in compare", col);
        }
    }

    // Build key maps
    let source_map = build_key_map(source, key_columns);
    let compare_map = build_key_map(compare, key_columns);

    let source_keys: HashSet<String> = source_map.keys().cloned().collect();
    let compare_keys: HashSet<String> = compare_map.keys().cloned().collect();

    let matched_keys: Vec<String> = source_keys.intersection(&compare_keys).cloned().collect();
    let missing_in_compare: Vec<String> = source_keys.difference(&compare_keys).cloned().collect();
    let missing_in_source: Vec<String> = compare_keys.difference(&source_keys).cloned().collect();

    let mut mismatches = Vec::new();
    let mut field_mismatch_count = 0;
    let mut type_mismatch_count = 0;

    // Compare matched rows field by field
    for key in &matched_keys {
        let (source_row, compare_row) = match (source_map.get(key), compare_map.get(key)) {
            (Some(s), Some(c)) => (s, c),
            _ => continue,
        };

        for col in compare_columns {
            let source_val = source_row.get(col);
            let compare_val = compare_row.get(col);

            // Type mismatch detection (before coercion)
            let tm = detect_type_mismatch(source_val, compare_val);
            if tm {
                type_mismatch_count += 1;
                mismatches.push(Mismatch {
                    row_number: source_row.row_number,
                    key_value: key.clone(),
                    field: col.clone(),
                    source_value: format_value(source_val),
                    compare_value: format_value(compare_val),
                    mismatch_type: MismatchType::TypeMismatch,
                    similarity: None,
                });
                continue;
            }

            // Coerce values before comparison for smarter matching
            let source_type = source.column_types.get(col).cloned();
            let compare_type = compare.column_types.get(col).cloned();
            let source_coerced = maybe_coerce(source_val, &source_type);
            let compare_coerced = maybe_coerce(compare_val, &compare_type);

            // Value comparison (uses coerced values if available)
            let is_match = match_values(&source_coerced, &compare_coerced, &match_type);

            if !is_match {
                field_mismatch_count += 1;
                let (mismatch_type, similarity) = match &match_type {
                    MatchType::Fuzzy { threshold } => {
                        let sim = fuzzy::similarity(
                            &format_value(source_val),
                            &format_value(compare_val),
                        );
                        if sim < *threshold {
                            (MismatchType::ValueMismatch, Some(sim))
                        } else {
                            (MismatchType::FuzzyMatch { distance: 1.0 - sim }, Some(sim))
                        }
                    }
                    _ => (MismatchType::ValueMismatch, None),
                };

                mismatches.push(Mismatch {
                    row_number: source_row.row_number,
                    key_value: key.clone(),
                    field: col.clone(),
                    source_value: format_value(source_val),
                    compare_value: format_value(compare_val),
                    mismatch_type,
                    similarity,
                });
            }
        }
    }

    // Add missing row mismatches
    for key in &missing_in_compare {
        if let Some(row) = source_map.get(key) {
            mismatches.push(Mismatch {
                row_number: row.row_number,
                key_value: key.clone(),
                field: "-".to_string(),
                source_value: "present in source".to_string(),
                compare_value: "MISSING".to_string(),
                mismatch_type: MismatchType::MissingInCompare,
                similarity: None,
            });
        }
    }

    for key in &missing_in_source {
        mismatches.push(Mismatch {
            row_number: 0,
            key_value: key.clone(),
            field: "-".to_string(),
            source_value: "MISSING".to_string(),
            compare_value: "present in compare".to_string(),
            mismatch_type: MismatchType::MissingInSource,
            similarity: None,
        });
    }

    let total_mismatches = mismatches.len();
    let source_row_count = source.rows.len();
    let compare_row_count = compare.rows.len();
    let matched_count = matched_keys.len();
    let missing_compare = missing_in_compare.len();
    let missing_source = missing_in_source.len();

    let match_rate = if source_row_count > 0 {
        ((matched_count as f64) / (source_row_count as f64)) * 100.0
    } else {
        0.0
    };

    let execution_time = start.elapsed().as_millis() as u64;

    ComparisonResult {
        source_name: source.name.clone(),
        compare_name: compare.name.clone(),
        key_columns: key_columns.to_vec(),
        compare_columns: compare_columns.to_vec(),
        source_row_count,
        compare_row_count,
        matched_rows: matched_count,
        mismatched_rows: total_mismatches,
        missing_in_compare: missing_compare,
        missing_in_source: missing_source,
        field_mismatches: field_mismatch_count,
        type_mismatches: type_mismatch_count,
        total_mismatches,
        mismatches,
        execution_time_ms: execution_time,
        match_rate_percent: match_rate,
    }
}

/// Build a key map from a SheetData
fn build_key_map<'a>(sheet: &'a SheetData, key_columns: &'a [String]) -> HashMap<String, &'a DataRow> {
    let mut map = HashMap::new();
    for row in &sheet.rows {
        let key = key_columns
            .iter()
            .map(|col| row.get_normalized(col))
            .collect::<Vec<_>>()
            .join("|");
        if !key.trim().is_empty() {
            map.insert(key, row);
        }
    }
    map
}

// --- Private helpers ---

/// Attempt to coerce a CellValue to match the expected column type.
/// Returns the coerced value if possible, or the original if coercion fails or type is unknown.
fn maybe_coerce(value: Option<&CellValue>, target: &Option<ValueType>) -> Option<CellValue> {
    match (value, target.as_ref()) {
        (Some(v), Some(t)) => v.coerce_to(t).or_else(|| Some(v.clone())),
        _ => value.cloned(),
    }
}

fn match_values(source: &Option<CellValue>, compare: &Option<CellValue>, match_type: &MatchType) -> bool {
    match (source, compare) {
        (None, None) => true,  // Both missing = match
        (Some(_), None) | (None, Some(_)) => false,  // One missing = mismatch
        (Some(s), Some(c)) => {
            match match_type {
                MatchType::Exact => s.to_normalized() == c.to_normalized(),
                MatchType::CaseInsensitive => {
                    s.to_normalized().eq_ignore_ascii_case(&c.to_normalized())
                }
                MatchType::Fuzzy { threshold } => {
                    let sim = fuzzy::similarity(&s.to_normalized(), &c.to_normalized());
                    sim >= *threshold
                }
            }
        }
    }
}

fn detect_type_mismatch(source: Option<&CellValue>, compare: Option<&CellValue>) -> bool {
    match (source, compare) {
        // Number <-> String cross (common in Excel CSV exports)
        (Some(CellValue::Number(_)), Some(CellValue::String(_))) => true,
        (Some(CellValue::String(_)), Some(CellValue::Number(_))) => true,

        // Date <-> String cross
        (Some(CellValue::Date(_)), Some(CellValue::String(_))) => true,
        (Some(CellValue::String(_)), Some(CellValue::Date(_))) => true,

        // Date <-> Number cross (Excel serial dates vs ISO strings)
        (Some(CellValue::Date(_)), Some(CellValue::Number(_))) => true,
        (Some(CellValue::Number(_)), Some(CellValue::Date(_))) => true,

        // Boolean <-> String cross
        (Some(CellValue::Boolean(_)), Some(CellValue::String(_))) => true,
        (Some(CellValue::String(_)), Some(CellValue::Boolean(_))) => true,

        // Boolean <-> Number cross (Excel TRUE/FALSE vs 1/0)
        (Some(CellValue::Boolean(_)), Some(CellValue::Number(_))) => true,
        (Some(CellValue::Number(_)), Some(CellValue::Boolean(_))) => true,

        // Boolean <-> Date (unlikely but catches weird data)
        (Some(CellValue::Boolean(_)), Some(CellValue::Date(_))) => true,
        (Some(CellValue::Date(_)), Some(CellValue::Boolean(_))) => true,

        _ => false,
    }
}

fn format_value(value: Option<&CellValue>) -> String {
    match value {
        Some(v) => v.to_normalized(),
        None => "<missing>".to_string(),
    }
}
