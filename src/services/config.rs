//! Configuration file support for complex comparison jobs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::types::{MatchType, ValueType};

/// Main configuration for a comparison job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EdixConfig {
    /// Job name (for reporting)
    pub name: String,

    /// Source file configuration
    pub source: FileConfig,

    /// Compare file(s) configuration
    pub compares: Vec<FileConfig>,

    /// Output configuration
    pub output: OutputConfig,

    /// Matching rules
    pub matching: MatchingConfig,

    /// Type coercion rules per column
    pub type_coercion: Vec<ColumnTypeConfig>,

    /// Fuzzy matching settings
    pub fuzzy: FuzzyConfig,
}

impl Default for EdixConfig {
    fn default() -> Self {
        Self {
            name: "edix-comparison".to_string(),
            source: FileConfig::default(),
            compares: vec![],
            output: OutputConfig::default(),
            matching: MatchingConfig::default(),
            type_coercion: vec![],
            fuzzy: FuzzyConfig::default(),
        }
    }
}

impl EdixConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &PathBuf) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: EdixConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file(&self, path: &PathBuf) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Generate a sample config file
    pub fn generate_sample() -> Self {
        Self {
            name: "bank-vs-gl-reconciliation".to_string(),
            source: FileConfig {
                path: PathBuf::from("bank_export.xlsx"),
                sheet: Some("Bank".to_string()),
                csv_options: Some(CsvOptions {
                    delimiter: ','.to_string(),
                    has_headers: true,
                    encoding: "utf-8".to_string(),
                }),
                header_row: Some(1),
            },
            compares: vec![
                FileConfig {
                    path: PathBuf::from("gl_export.xlsx"),
                    sheet: Some("GL".to_string()),
                    csv_options: None,
                    header_row: Some(1),
                },
            ],
            output: OutputConfig {
                format: OutputFormat::Csv,
                path: PathBuf::from("reconciliation_result"),
                include_summary: true,
                highlight_mismatches: true,
                max_rows_per_file: 500_000,
            },
            matching: MatchingConfig {
                key_columns: vec!["transaction_id".to_string()],
                compare_columns: vec![
                    "amount".to_string(),
                    "date".to_string(),
                    "description".to_string(),
                ],
                match_type: MatchType::Exact,
                case_sensitive: false,
            },
            type_coercion: vec![
                ColumnTypeConfig {
                    column: "amount".to_string(),
                    target_type: ValueType::Number,
                },
                ColumnTypeConfig {
                    column: "date".to_string(),
                    target_type: ValueType::Date,
                },
            ],
            fuzzy: FuzzyConfig {
                enabled: false,
                threshold: 0.85,
                columns: vec!["description".to_string()],
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    pub path: PathBuf,
    pub sheet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csv_options: Option<CsvOptions>,
    pub header_row: Option<usize>,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("data.xlsx"),
            sheet: None,
            csv_options: None,
            header_row: Some(1),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvOptions {
    pub delimiter: String,
    pub has_headers: bool,
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub path: PathBuf,
    pub include_summary: bool,
    pub highlight_mismatches: bool,
    pub max_rows_per_file: usize,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Csv,
            path: PathBuf::from("edix_output"),
            include_summary: true,
            highlight_mismatches: true,
            max_rows_per_file: 500_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Csv,
    Excel,
    Markdown,
    Json,
    All, // Generate all formats
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingConfig {
    pub key_columns: Vec<String>,
    pub compare_columns: Vec<String>,
    pub match_type: MatchType,
    pub case_sensitive: bool,
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            key_columns: vec!["id".to_string()],
            compare_columns: vec![],
            match_type: MatchType::Exact,
            case_sensitive: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnTypeConfig {
    pub column: String,
    pub target_type: ValueType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzyConfig {
    pub enabled: bool,
    pub threshold: f64,
    pub columns: Vec<String>,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.85,
            columns: vec![],
        }
    }
}
