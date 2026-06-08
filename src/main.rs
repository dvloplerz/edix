use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};

mod services;
use services::config::{EdixConfig, CsvOptions, OutputConfig, MatchingConfig, ColumnTypeConfig, FuzzyConfig, FileConfig};
use services::diff::compare_sheets;
use services::formatter::{write_output, OutputFormat as FmtOutputFormat};
use services::io::{load_excel_sheet, load_csv_sheet};
use services::types::{SheetData, MatchType, MismatchType};

#[derive(Parser, Debug)]
#[command(name = "edix", version, about = "Excel/CSV comparison tool with key-based joins, type coercion, and fuzzy matching", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Dry run - show what would be done without writing output
    #[arg(long, global = true)]
    dry_run: bool,

    /// Show progress bars
    #[arg(short, long, global = true)]
    progress: bool,

    /// Config file path (TOML)
    #[arg(short, long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compare two sheets/files
    Compare {
        /// Source file path (Excel or CSV)
        #[arg(value_name = "SOURCE")]
        source: PathBuf,

        /// Compare file path (Excel or CSV)
        #[arg(value_name = "COMPARE")]
        compare: PathBuf,

        /// Source sheet name (for Excel)
        #[arg(short = 'S', long, value_name = "NAME")]
        source_sheet: Option<String>,

        /// Compare sheet name (for Excel)
        #[arg(short = 's', long, value_name = "NAME")]
        compare_sheet: Option<String>,

        /// Key column(s) for joining (comma-separated for multi-column keys)
        #[arg(short = 'k', long, value_name = "COLUMNS", value_delimiter = ',')]
        key: Vec<String>,

        /// Columns to compare (comma-separated, default: all non-key columns)
        #[arg(short = 'C', long, value_name = "COLUMNS", value_delimiter = ',')]
        compare_columns: Vec<String>,

        /// Matching mode
        #[arg(short = 'm', long, value_enum, default_value = "exact")]
        match_type: MatchMode,

        /// Fuzzy similarity threshold (0.0-1.0)
        #[arg(long, value_name = "THRESHOLD")]
        fuzzy_threshold: Option<f64>,

        /// Output file path
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "csv")]
        format: OutputFormatCli,

        /// CSV delimiter for source file
        #[arg(long, default_value = ",")]
        source_delimiter: char,

        /// CSV delimiter for compare file
        #[arg(long, default_value = ",")]
        compare_delimiter: char,

        /// Source file has header row [default: true]
        #[arg(long, default_value_t = true, num_args = 0..=1, require_equals = true)]
        source_has_header: bool,

        /// Compare file has header row [default: true]
        #[arg(long, default_value_t = true, num_args = 0..=1, require_equals = true)]
        compare_has_header: bool,

        /// Quick audit mode: only check row counts, columns, and data types
        #[arg(long)]
        quick_audit: bool,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Create a default config file
    Init {
        /// Output config file path
        #[arg(short, long, value_name = "FILE", default_value = "edix.toml")]
        output: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current config
    Show {
        /// Config file path
        #[arg(short, long, value_name = "FILE")]
        file: Option<PathBuf>,
    },
    /// Validate config file
    Validate {
        /// Config file path
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum MatchMode {
    Exact,
    CaseInsensitive,
    Fuzzy,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormatCli {
    Csv,
    Markdown,
    Excel,
}

impl From<OutputFormatCli> for FmtOutputFormat {
    fn from(v: OutputFormatCli) -> Self {
        match v {
            OutputFormatCli::Csv => FmtOutputFormat::Csv,
            OutputFormatCli::Markdown => FmtOutputFormat::Markdown,
            OutputFormatCli::Excel => FmtOutputFormat::Excel,
        }
    }
}

impl From<MatchMode> for MatchType {
    fn from(v: MatchMode) -> Self {
        match v {
            MatchMode::Exact => MatchType::Exact,
            MatchMode::CaseInsensitive => MatchType::CaseInsensitive,
            MatchMode::Fuzzy => MatchType::Fuzzy { threshold: 0.8 },
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config file if provided
    let config = if let Some(config_path) = &cli.config {
        EdixConfig::from_file(config_path)?
    } else {
        EdixConfig::default()
    };

    match cli.command {
        Commands::Compare {
            source,
            compare,
            source_sheet,
            compare_sheet,
            key,
            compare_columns,
            match_type,
            fuzzy_threshold,
            output,
            format,
            source_delimiter,
            compare_delimiter,
            source_has_header,
            compare_has_header,
            quick_audit,
        } => {
            run_compare(
                source,
                compare,
                source_sheet,
                compare_sheet,
                key,
                compare_columns,
                match_type,
                fuzzy_threshold,
                output,
                format,
                source_delimiter,
                compare_delimiter,
                source_has_header,
                compare_has_header,
                quick_audit,
                &config,
                cli.verbose,
                cli.dry_run,
                cli.progress,
            )?;
        }
        Commands::Config { action } => {
            run_config(action, cli.config.as_deref())?;
        }
        Commands::Init { output } => {
            let default_config = EdixConfig::default();
            default_config.to_file(&output)?;
            println!("Created default config at {}", output.display());
        }
    }

    Ok(())
}

fn run_compare(
    source_path: PathBuf,
    compare_path: PathBuf,
    source_sheet: Option<String>,
    compare_sheet: Option<String>,
    key_columns: Vec<String>,
    compare_columns: Vec<String>,
    match_mode: MatchMode,
    fuzzy_threshold: Option<f64>,
    output_path: Option<PathBuf>,
    output_format: OutputFormatCli,
    source_delimiter: char,
    compare_delimiter: char,
    source_has_header: bool,
    compare_has_header: bool,
    quick_audit: bool,
    config: &EdixConfig,
    verbose: bool,
    dry_run: bool,
    _progress: bool,
) -> anyhow::Result<()> {
    // Determine file types and load sheets
    let source_sheet_data = load_sheet(&source_path, source_sheet, source_delimiter, source_has_header, verbose)?;
    let compare_sheet_data = load_sheet(&compare_path, compare_sheet, compare_delimiter, compare_has_header, verbose)?;

    if verbose {
        println!("Source: {} ({} rows, {} cols)", source_sheet_data.name, source_sheet_data.rows.len(), source_sheet_data.headers.len());
        println!("Compare: {} ({} rows, {} cols)", compare_sheet_data.name, compare_sheet_data.rows.len(), compare_sheet_data.headers.len());
    }

    // QUICK AUDIT MODE: skip full comparison, just do structural check
    if quick_audit {
        return run_quick_audit(&source_sheet_data, &compare_sheet_data, output_path, output_format, dry_run, verbose);
    }

    // Determine key columns
    let key_columns = if key_columns.is_empty() {
        // Use config or auto-detect first column
        if !config.matching.key_columns.is_empty() {
            config.matching.key_columns.clone()
        } else if !source_sheet_data.headers.is_empty() {
            vec![source_sheet_data.headers[0].clone()]
        } else {
            anyhow::bail!("No key columns specified and cannot auto-detect");
        }
    } else {
        key_columns
    };

    // Validate key columns exist
    for col in &key_columns {
        if !source_sheet_data.headers.contains(col) {
            anyhow::bail!("Key column '{}' not found in source", col);
        }
        if !compare_sheet_data.headers.contains(col) {
            anyhow::bail!("Key column '{}' not found in compare", col);
        }
    }

    // Determine compare columns
    let compare_columns = if compare_columns.is_empty() {
        // All non-key columns from source
        source_sheet_data.headers.iter()
            .filter(|h| !key_columns.contains(h))
            .cloned()
            .collect()
    } else {
        compare_columns
    };

    // Determine match type
    let mut match_type: MatchType = match_mode.into();
    if let MatchMode::Fuzzy = match_mode {
        if let Some(threshold) = fuzzy_threshold {
            match_type = MatchType::Fuzzy { threshold };
        } else if config.fuzzy.enabled {
            match_type = MatchType::Fuzzy { threshold: config.fuzzy.threshold };
        }
    }

    if verbose {
        println!("Key columns: {:?}", key_columns);
        println!("Compare columns: {:?}", compare_columns);
        println!("Match type: {:?}", match_type);
    }

    if dry_run {
        println!("Dry run - would compare {} source rows with {} compare rows", source_sheet_data.rows.len(), compare_sheet_data.rows.len());
        return Ok(());
    }

    // Run comparison
    let result = compare_sheets(&source_sheet_data, &compare_sheet_data, &key_columns, &compare_columns, match_type);

    // Print summary
    println!("\nComparison Complete");
    println!("{}", "─".repeat(50));
    println!("Source:      {}", result.source_name);
    println!("Compare:     {}", result.compare_name);
    println!("Key columns: {}", key_columns.join(", "));
    println!("Source rows: {}", result.source_row_count);
    println!("Compare rows: {}", result.compare_row_count);
    println!("Matched:     {}", result.matched_rows);
    println!("Mismatched:  {}", result.mismatched_rows);
    println!("Missing in compare: {}", result.missing_in_compare);
    println!("Missing in source:  {}", result.missing_in_source);
    println!("Match rate:  {:.2}%", result.match_rate_percent);
    println!("Time:        {} ms", result.execution_time_ms);

    // Write output
    if let Some(output_path) = output_path {
        let format: FmtOutputFormat = output_format.into();
        if verbose {
            println!("Writing {} output to {}", format_to_str(&format), output_path.display());
        }
        write_output(&result, &output_path, format)?;
        println!("✓ Output written to {}", output_path.display());
    }

    Ok(())
}

fn format_to_str(f: &FmtOutputFormat) -> &'static str {
    match f {
        FmtOutputFormat::Csv => "CSV",
        FmtOutputFormat::Markdown => "Markdown",
        FmtOutputFormat::Excel => "Excel",
    }
}

/// Quick Audit: structural comparison without full row-by-row check.
/// Returns in <1 second even for million-row files.
fn run_quick_audit(
    source: &SheetData,
    compare: &SheetData,
    output_path: Option<PathBuf>,
    output_format: OutputFormatCli,
    dry_run: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    use std::cmp::Ordering;
    use services::types::ValueType;

    // Column overlap analysis
    let source_cols: std::collections::HashSet<_> = source.headers.iter().collect();
    let compare_cols: std::collections::HashSet<_> = compare.headers.iter().collect();
    let common_cols: Vec<_> = source.headers.iter().filter(|h| compare_cols.contains(h)).collect();
    let source_only: Vec<_> = source.headers.iter().filter(|h| !compare_cols.contains(h)).collect();
    let compare_only: Vec<_> = compare.headers.iter().filter(|h| !source_cols.contains(h)).collect();

    // Type mismatch analysis at sample level
    let mut type_mismatches = Vec::new();
    for col in &common_cols {
        let s_type = source.column_types.get(*col).unwrap_or(&ValueType::Auto);
        let c_type = compare.column_types.get(*col).unwrap_or(&ValueType::Auto);
        if s_type != c_type && !(matches!(s_type, ValueType::String) && matches!(c_type, ValueType::String)) {
            type_mismatches.push((*col, s_type.clone(), c_type.clone()));
        }
    }

    // Row count comparison
    let row_count_diff = if source.rows.len() > compare.rows.len() {
        let diff = source.rows.len() - compare.rows.len();
        format!("Source has {} more rows", diff)
    } else if compare.rows.len() > source.rows.len() {
        let diff = compare.rows.len() - source.rows.len();
        format!("Compare has {} more rows", diff)
    } else {
        "Row counts match".to_string()
    };

    // Audit score: percentage of structural compatibility
    let col_overlap = common_cols.len() as f64 / source.headers.len().max(compare.headers.len()) as f64 * 100.0;
    let row_ratio = source.rows.len() as f64 / compare.rows.len().max(1) as f64;
    let row_similarity = if row_ratio > 1.0 { 1.0 / row_ratio } else { row_ratio } * 100.0;
    let audit_score = (col_overlap * 0.5) + (row_similarity * 0.5);

    // Console output
    println!("
╔══════════════════════════════════════════════════════╗
║           QUICK AUDIT REPORT                         ║
╠══════════════════════════════════════════════════════╣");
    println!("  Source:    {} ({} rows, {} cols)", source.name, source.rows.len(), source.headers.len());
    println!("  Compare:   {} ({} rows, {} cols)", compare.name, compare.rows.len(), compare.headers.len());
    println!("  ─────────────────────────────────────────────────  ");
    println!("  Row Status:        {}", row_count_diff);
    println!("  Column Overlap:    {} / {} ({}%)", common_cols.len(), source.headers.len(), col_overlap.round());
    println!("  Audit Score:       {:.1}%", audit_score);
    println!("  ─────────────────────────────────────────────────  ");
    println!("  Common Columns:    {}", common_cols.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    if !source_only.is_empty() {
        println!("  ⚠ Source Only:     {}", source_only.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }
    if !compare_only.is_empty() {
        println!("  ⚠ Compare Only:    {}", compare_only.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }
    if !type_mismatches.is_empty() {
        println!("  ─────────────────────────────────────────────────  ");
        println!("  Type Mismatches:");
        for (col, s_type, c_type) in &type_mismatches {
            println!("    ⚠ Column '{}': Source = {:?}, Compare = {:?}", col, s_type, c_type);
        }
    }
    println!("╚══════════════════════════════════════════════════════╝");

    if dry_run {
        println!("(Dry run - no output file written)");
        return Ok(());
    }

    // Write output if requested
    if let Some(path) = output_path {
        let format: FmtOutputFormat = output_format.into();
        let content = format!(
            "### Quick Audit Report\n\n| Metric | Source | Compare |\n|--------|--------|---------|\n| File | {} | {} |\n| Rows | {} | {} |\n| Columns | {} | {} |\n| Column Overlap | {} / {} ({:.0}%) | |\n| Row Status | {} | |\n| Audit Score | {:.1}% | |\n\n",
            source.name, compare.name,
            source.rows.len(), compare.rows.len(),
            source.headers.len(), compare.headers.len(),
            common_cols.len(), source.headers.len(), col_overlap,
            row_count_diff, audit_score
        );

        std::fs::write(&path, content)?;
        println!("✓ Quick audit summary written to {}", path.display());
    }

    Ok(())
}

fn load_sheet(
    path: &PathBuf,
    sheet_name: Option<String>,
    delimiter: char,
    has_header: bool,
    verbose: bool,
) -> anyhow::Result<SheetData> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    
    match ext.as_str() {
        "xlsx" | "xls" | "xlsm" => {
            let sheet = sheet_name.unwrap_or_else(|| {
                if verbose {
                    println!("No sheet specified, using first sheet");
                }
                String::new()
            });
            load_excel_sheet(path, if sheet.is_empty() { None } else { Some(&sheet) }, None)
        }
        "csv" | "txt" => {
            let csv_opts = CsvOptions {
                delimiter: delimiter.to_string(),
                has_headers: has_header,
                encoding: "utf-8".to_string(),
            };
            load_csv_sheet(path, Some(&csv_opts), None)
        }
        _ => {
            // Try Excel first, then CSV
            if verbose {
                println!("Unknown extension, trying Excel...");
            }
            match load_excel_sheet(path, sheet_name.as_deref(), None) {
                Ok(s) => Ok(s),
                Err(_) => {
                    if verbose {
                        println!("Excel failed, trying CSV...");
                    }
                    let csv_opts = CsvOptions {
                        delimiter: delimiter.to_string(),
                        has_headers: has_header,
                        encoding: "utf-8".to_string(),
                    };
                    load_csv_sheet(path, Some(&csv_opts), None)
                }
            }
        }
    }
}

fn run_config(action: ConfigAction, config_path: Option<&Path>) -> anyhow::Result<()> {
    match action {
        ConfigAction::Show { file } => {
            let path = file.unwrap_or_else(|| PathBuf::from("edix.toml"));
            let config = EdixConfig::from_file(&path)?;
            println!("{}", toml::to_string_pretty(&config)?);
        }
        ConfigAction::Validate { file } => {
            let _ = EdixConfig::from_file(&file)?;
            println!("Config file '{}' is valid", file.display());
        }
    }
    Ok(())
}