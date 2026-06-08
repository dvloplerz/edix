//! Output formatters: CSV, Markdown, Excel with highlights

use std::collections::HashSet;
use std::path::Path;

use super::types::*;

/// Write comparison results to CSV
pub fn write_csv<P: AsRef<Path>>(result: &ComparisonResult, path: P) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;

    // Header
    wtr.write_record(&[
        "source_sheet",
        "compare_sheet",
        "key_columns",
        "compare_columns",
        "row_number",
        "key_value",
        "field",
        "source_value",
        "compare_value",
        "mismatch_type",
        "similarity",
    ])?;

    let key_cols = result.key_columns.join("|");
    let cmp_cols = result.compare_columns.join("|");

    for m in &result.mismatches {
        let similarity = m.similarity
            .map(|s| format!("{:.4}", s))
            .unwrap_or_else(|| "".to_string());

        wtr.write_record(&[
            &result.source_name,
            &result.compare_name,
            &key_cols,
            &cmp_cols,
            &m.row_number.to_string(),
            &m.key_value,
            &m.field,
            &m.source_value,
            &m.compare_value,
            &format!("{:?}", m.mismatch_type),
            &similarity,
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// Write comparison results to Markdown summary
pub fn write_markdown<P: AsRef<Path>>(result: &ComparisonResult, path: P) -> anyhow::Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(path)?;

    writeln!(file, "# edix Comparison Report")?;
    writeln!(file)?;
    writeln!(file, "**Source:** {}", result.source_name)?;
    writeln!(file, "**Compare:** {}", result.compare_name)?;
    writeln!(file, "**Key Columns:** {}", result.key_columns.join(", "))?;
    writeln!(file, "**Compare Columns:** {}", result.compare_columns.join(", "))?;
    writeln!(file)?;
    writeln!(file, "## Summary")?;
    writeln!(file)?;
    writeln!(file, "| Metric | Value |")?;
    writeln!(file, "|--------|-------|")?;
    writeln!(file, "| Source Rows | {} |", result.source_row_count)?;
    writeln!(file, "| Compare Rows | {} |", result.compare_row_count)?;
    writeln!(file, "| Matched Rows | {} |", result.matched_rows)?;
    writeln!(file, "| Mismatched Rows | {} |", result.mismatched_rows)?;
    writeln!(file, "| Missing in Compare | {} |", result.missing_in_compare)?;
    writeln!(file, "| Missing in Source | {} |", result.missing_in_source)?;
    writeln!(file, "| Field Mismatches | {} |", result.field_mismatches)?;
    writeln!(file, "| Type Mismatches | {} |", result.type_mismatches)?;
    writeln!(file, "| Total Mismatches | {} |", result.total_mismatches)?;
    writeln!(file, "| Match Rate | {:.2}% |", result.match_rate_percent)?;
    writeln!(file, "| Execution Time | {} ms |", result.execution_time_ms)?;
    writeln!(file)?;

    if !result.mismatches.is_empty() {
        writeln!(file, "## Mismatches")?;
        writeln!(file)?;
        writeln!(file, "| Row | Key | Field | Source | Compare | Type | Similarity |")?;
        writeln!(file, "|-----|-----|-------|--------|---------|------|------------|")?;

        for m in &result.mismatches {
            let similarity = m.similarity
                .map(|s| format!("{:.2}%", s * 100.0))
                .unwrap_or_else(|| "N/A".to_string());

            writeln!(file, "| {} | {} | {} | {} | {} | {:?} | {} |",
                m.row_number,
                m.key_value,
                m.field,
                escape_md(&m.source_value),
                escape_md(&m.compare_value),
                m.mismatch_type,
                similarity
            )?;
        }
    }

    Ok(())
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
     .replace('\n', " ")
     .replace('\r', " ")
}

/// Write comparison results to Excel with highlights
pub fn write_excel<P: AsRef<Path>>(result: &ComparisonResult, path: P) -> anyhow::Result<()> {
    use xlsxwriter::Workbook;
    use xlsxwriter::format::{Format, FormatAlignment, FormatBorder, FormatColor};

    let path_str = path.as_ref().to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
    let workbook = Workbook::new(path_str)?;
    let mut sheet = workbook.add_worksheet(Some("Summary"))?;

    // Styles - Format::new() creates new formats
    let header_fmt = {
        let mut f = Format::new();
        f.set_bold();
        f.set_bg_color(FormatColor::Custom(0x4472C4));
        f.set_font_color(FormatColor::Custom(0xFFFFFF));
        f.set_align(FormatAlignment::Center);
        f.set_border(FormatBorder::Thin);
        f
    };
    let normal_fmt = {
        let mut f = Format::new();
        f.set_border(FormatBorder::Thin);
        f
    };
    let mismatch_fmt = {
        let mut f = Format::new();
        f.set_bg_color(FormatColor::Custom(0xFFC7CE));
        f.set_font_color(FormatColor::Custom(0x9C0006));
        f.set_border(FormatBorder::Thin);
        f
    };
    let missing_fmt = {
        let mut f = Format::new();
        f.set_bg_color(FormatColor::Custom(0xFFEB9C));
        f.set_font_color(FormatColor::Custom(0x9C6500));
        f.set_border(FormatBorder::Thin);
        f
    };
    let type_mismatch_fmt = {
        let mut f = Format::new();
        f.set_bg_color(FormatColor::Custom(0xBDD7EE));
        f.set_font_color(FormatColor::Custom(0x1F4E79));
        f.set_border(FormatBorder::Thin);
        f
    };

    // Summary sheet
    let mut row = 0;
    sheet.write_string(row, 0, "Metric", Some(&header_fmt))?;
    sheet.write_string(row, 1, "Value", Some(&header_fmt))?;

    let summary_data = [
        ("Source", &result.source_name),
        ("Compare", &result.compare_name),
        ("Key Columns", &result.key_columns.join(", ")),
        ("Compare Columns", &result.compare_columns.join(", ")),
        ("Source Rows", &result.source_row_count.to_string()),
        ("Compare Rows", &result.compare_row_count.to_string()),
        ("Matched Rows", &result.matched_rows.to_string()),
        ("Mismatched Rows", &result.mismatched_rows.to_string()),
        ("Missing in Compare", &result.missing_in_compare.to_string()),
        ("Missing in Source", &result.missing_in_source.to_string()),
        ("Field Mismatches", &result.field_mismatches.to_string()),
        ("Type Mismatches", &result.type_mismatches.to_string()),
        ("Total Mismatches", &result.total_mismatches.to_string()),
        ("Match Rate %", &format!("{:.2}", result.match_rate_percent)),
        ("Execution Time (ms)", &result.execution_time_ms.to_string()),
    ];

    for (metric, value) in summary_data {
        row += 1;
        sheet.write_string(row, 0, metric, Some(&normal_fmt))?;
        sheet.write_string(row, 1, value, Some(&normal_fmt))?;
    }

    // Details sheet
    if !result.mismatches.is_empty() {
        let mut detail_sheet = workbook.add_worksheet(Some("Mismatches"))?;
        
        let headers = [
            "Row", "Key Value", "Field", "Source Value", 
            "Compare Value", "Mismatch Type", "Similarity"
        ];
        
        for (col, h) in headers.iter().enumerate() {
            detail_sheet.write_string(0, col as u16, h, Some(&header_fmt))?;
        }

        for (i, m) in result.mismatches.iter().enumerate() {
            let r = (i + 1) as u32;
            let fmt = match m.mismatch_type {
                MismatchType::MissingInCompare | MismatchType::MissingInSource => &missing_fmt,
                MismatchType::TypeMismatch => &type_mismatch_fmt,
                _ => &mismatch_fmt,
            };

            let similarity = m.similarity
                .map(|s| format!("{:.2}%", s * 100.0))
                .unwrap_or_else(|| "N/A".to_string());

            detail_sheet.write_number(r, 0, m.row_number as f64, Some(fmt))?;
            detail_sheet.write_string(r, 1, &m.key_value, Some(fmt))?;
            detail_sheet.write_string(r, 2, &m.field, Some(fmt))?;
            detail_sheet.write_string(r, 3, &m.source_value, Some(fmt))?;
            detail_sheet.write_string(r, 4, &m.compare_value, Some(fmt))?;
            detail_sheet.write_string(r, 5, &format!("{:?}", m.mismatch_type), Some(fmt))?;
            detail_sheet.write_string(r, 6, &similarity, Some(fmt))?;
        }

        // Auto-width columns
        detail_sheet.set_column(0, 0, 8.0, None)?;
        detail_sheet.set_column(1, 1, 25.0, None)?;
        detail_sheet.set_column(2, 2, 20.0, None)?;
        detail_sheet.set_column(3, 4, 30.0, None)?;
        detail_sheet.set_column(5, 5, 20.0, None)?;
        detail_sheet.set_column(6, 6, 12.0, None)?;
    }

    workbook.close()?;
    Ok(())
}

/// Output format selector
pub enum OutputFormat {
    Csv,
    Markdown,
    Excel,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "csv" => Some(Self::Csv),
            "md" | "markdown" => Some(Self::Markdown),
            "xlsx" | "excel" => Some(Self::Excel),
            _ => None,
        }
    }
}

pub fn write_output<P: AsRef<Path>>(
    result: &ComparisonResult,
    path: P,
    format: OutputFormat,
) -> anyhow::Result<()> {
    match format {
        OutputFormat::Csv => write_csv(result, path),
        OutputFormat::Markdown => write_markdown(result, path),
        OutputFormat::Excel => write_excel(result, path),
    }
}