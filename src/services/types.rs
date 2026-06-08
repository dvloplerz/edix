//! Core data types for edix

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported cell value types with coercion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum CellValue {
    String(String),
    Number(f64),
    Date(String),      // ISO 8601 format
    Boolean(bool),
    Empty,
    Error(String),
}

impl CellValue {
    /// Convert to a normalized string for comparison
    pub fn to_normalized(&self) -> String {
        match self {
            CellValue::String(s) => s.trim().to_lowercase(),
            CellValue::Number(n) => {
                // Remove trailing zeros for clean comparison
                if n.fract() == 0.0 {
                    format!("{:.0}", n)
                } else {
                    format!("{}", n)
                }
            }
            CellValue::Date(d) => d.clone(),
            CellValue::Boolean(b) => b.to_string().to_lowercase(),
            CellValue::Empty => String::new(),
            CellValue::Error(e) => format!("error:{}", e),
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        matches!(self, CellValue::Empty)
    }

    /// Attempt type coercion to target type
    pub fn coerce_to(&self, target: &ValueType) -> Option<CellValue> {
        match (self, target) {
            // Already correct type
            (CellValue::String(_), ValueType::String) => Some(self.clone()),
            (CellValue::Number(_), ValueType::Number) => Some(self.clone()),
            (CellValue::Date(_), ValueType::Date) => Some(self.clone()),
            (CellValue::Boolean(_), ValueType::Boolean) => Some(self.clone()),

            // Coerce string to number
            (CellValue::String(s), ValueType::Number) => {
                s.trim().replace(',', "").parse::<f64>()
                    .ok()
                    .map(CellValue::Number)
            }

            // Coerce number to string
            (CellValue::Number(n), ValueType::String) => {
                Some(CellValue::String(format!("{}", n)))
            }

            // Coerce date string to date
            (CellValue::String(s), ValueType::Date) => {
                // Try common formats
                let formats = [
                    "%Y-%m-%d",
                    "%d/%m/%Y",
                    "%m/%d/%Y",
                    "%Y/%m/%d",
                    "%d-%m-%Y",
                    "%m-%d-%Y",
                ];
                for fmt in &formats {
                    if let Ok(dt) = chrono::NaiveDate::parse_from_str(s.trim(), fmt) {
                        return Some(CellValue::Date(dt.format("%Y-%m-%d").to_string()));
                    }
                }
                None
            }

            // Coerce date to string
            (CellValue::Date(d), ValueType::String) => {
                Some(CellValue::String(d.clone()))
            }

            _ => None,
        }
    }
}

impl From<calamine::Data> for CellValue {
    fn from(data: calamine::Data) -> Self {
        match data {
            calamine::Data::String(s) => CellValue::String(s),
            calamine::Data::Float(f) => CellValue::Number(f),
            calamine::Data::Int(i) => CellValue::Number(i as f64),
            calamine::Data::Bool(b) => CellValue::Boolean(b),
            calamine::Data::DateTime(d) => {
                // d is ExcelDateTime, convert to NaiveDate using as_datetime()
                if let Some(dt) = d.as_datetime() {
                    CellValue::Date(dt.date().format("%Y-%m-%d").to_string())
                } else {
                    // Fallback: use raw float value
                    let days = d.as_f64();
                    let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
                        .unwrap_or(chrono::NaiveDate::MIN);
                    if let Some(date) = base.checked_add_days(chrono::Days::new(days.floor() as u64)) {
                        CellValue::Date(date.format("%Y-%m-%d").to_string())
                    } else {
                        CellValue::String(format!("{}", days))
                    }
                }
            }
            calamine::Data::DateTimeIso(dt) => {
                // dt is already a String in ISO format
                CellValue::Date(dt)
            }
            calamine::Data::DurationIso(d) => {
                CellValue::String(d)
            }
            calamine::Data::Error(e) => CellValue::Error(format!("{:?}", e)),
            calamine::Data::Empty => CellValue::Empty,
        }
    }
}

/// Target value type for coercion
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ValueType {
    String,
    Number,
    Date,
    Boolean,
    Auto, // Detect from first non-empty cell
}

impl Default for ValueType {
    fn default() -> Self {
        ValueType::Auto
    }
}

/// A single row of data keyed by column name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRow {
    pub row_number: usize,  // 1-based for user display
    pub values: HashMap<String, CellValue>,
}

impl DataRow {
    pub fn new(row_number: usize) -> Self {
        Self {
            row_number,
            values: HashMap::new(),
        }
    }

    pub fn get(&self, column: &str) -> Option<&CellValue> {
        self.values.get(column)
    }

    pub fn get_normalized(&self, column: &str) -> String {
        self.get(column)
            .map(|v| v.to_normalized())
            .unwrap_or_default()
    }
}

/// A sheet loaded from Excel/CSV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetData {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<DataRow>,
    pub column_types: HashMap<String, ValueType>,
}

impl SheetData {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            headers: Vec::new(),
            rows: Vec::new(),
            column_types: HashMap::new(),
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get unique values for a column (useful for key columns)
    pub fn column_values(&self, column: &str) -> Vec<CellValue> {
        self.rows
            .iter()
            .filter_map(|row| row.get(column).cloned())
            .collect()
    }

    /// Build lookup map by key column for fast joins
    pub fn build_key_map(&self, key_columns: &[String]) -> HashMap<String, &DataRow> {
        let mut map = HashMap::new();
        for row in &self.rows {
            let key = key_columns.iter()
                .map(|col| row.get_normalized(col))
                .collect::<Vec<_>>()
                .join("|");
            if !key.trim().is_empty() {
                map.insert(key, row);
            }
        }
        map
    }
}

/// Comparison match type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MatchType {
    Exact,
    CaseInsensitive,
    Fuzzy { threshold: f64 },
}

impl Default for MatchType {
    fn default() -> Self {
        MatchType::Exact
    }
}

/// A single mismatch record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mismatch {
    pub row_number: usize,
    pub key_value: String,
    pub field: String,
    pub source_value: String,
    pub compare_value: String,
    pub mismatch_type: MismatchType,
    pub similarity: Option<f64>, // For fuzzy matches
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MismatchType {
    MissingInCompare,      // Row exists in source but not in compare sheet
    MissingInSource,       // Row exists in compare but not in source
    ValueMismatch,         // Same key, different values
    TypeMismatch,          // Same key, different data types
    FuzzyMatch { distance: f64 }, // Near match with similarity score
}

/// Comparison result summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub source_name: String,
    pub compare_name: String,
    pub key_columns: Vec<String>,
    pub compare_columns: Vec<String>,
    pub source_row_count: usize,
    pub compare_row_count: usize,
    pub matched_rows: usize,
    pub mismatched_rows: usize,
    pub missing_in_compare: usize,
    pub missing_in_source: usize,
    pub field_mismatches: usize,
    pub type_mismatches: usize,
    pub total_mismatches: usize,
    pub mismatches: Vec<Mismatch>,
    pub execution_time_ms: u64,
    pub match_rate_percent: f64,
}
