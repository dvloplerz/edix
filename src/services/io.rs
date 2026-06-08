//! I/O module for loading Excel and CSV files with header normalization and merged cell handling

use calamine::{open_workbook, Data, Reader, Xlsx};
use std::collections::HashMap;
use std::path::Path;

use super::types::{CellValue, DataRow, SheetData, ValueType};
use super::config::CsvOptions;

/// Load a sheet from an Excel (.xlsx) file
pub fn load_excel_sheet<P: AsRef<Path>>(
    file_path: P,
    sheet_name: Option<&str>,
    header_row: Option<usize>,
) -> anyhow::Result<SheetData> {
    let mut workbook: Xlsx<_> = open_workbook(&file_path)?;
    let sheets = workbook.sheet_names();

    if sheets.is_empty() {
        anyhow::bail!("workbook has no sheets");
    }

    let sheet_name = sheet_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| sheets[0].clone());

    if !sheets.contains(&sheet_name) {
        anyhow::bail!("sheet '{}' not found. available: {:?}", sheet_name, sheets);
    }

    let range = workbook.worksheet_range(&sheet_name)
        .map_err(|e| anyhow::anyhow!("sheet '{}': {}", sheet_name, e))?;

    let header_row = header_row.unwrap_or(1);
    parse_sheet(range, &sheet_name, header_row)
}

/// Load a sheet from a CSV file
pub fn load_csv_sheet<P: AsRef<Path>>(
    file_path: P,
    csv_options: Option<&CsvOptions>,
    header_row: Option<usize>,
) -> anyhow::Result<SheetData> {
    let delimiter = csv_options
        .as_ref()
        .and_then(|o| o.delimiter.chars().next())
        .unwrap_or(',');

    let has_headers = csv_options
        .as_ref()
        .map(|o| o.has_headers)
        .unwrap_or(true);

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter as u8)
        .has_headers(has_headers)
        .from_path(&file_path)?;

    let sheet_name = file_path.as_ref()
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("csv")
        .to_string();

    // Read all records first
    let mut sheet = SheetData::new(&sheet_name);
    let mut all_records: Vec<csv::StringRecord> = Vec::new();
    let mut raw_headers: Vec<String> = Vec::new();

    for result in reader.records() {
        let record = result?;
        all_records.push(record);
    }

    if all_records.is_empty() {
        return Ok(sheet);
    }

    // Handle headers
    if has_headers {
        // Use first data row as headers
        if let Some(first) = all_records.first() {
            raw_headers = first.iter().map(|f| f.to_string()).collect();
        }
        // Remove first row from data
        all_records.remove(0);
    } else {
        // Generate column letters
        let col_count = all_records[0].len();
        raw_headers = (0..col_count).map(|i| format!("column_{}", i + 1)).collect();
    }

    let header_row = header_row.unwrap_or(if has_headers { 2 } else { 1 });
    sheet.headers = normalize_headers(&raw_headers);

    // Parse data rows
    for (i, record) in all_records.iter().enumerate() {
        let row_num = header_row + i; // 1-based
        let mut data_row = DataRow::new(row_num);

        for (j, field) in record.iter().enumerate() {
            if let Some(header) = sheet.headers.get(j) {
                let value = if field.trim().is_empty() {
                    CellValue::Empty
                } else {
                    // Try number first, then string
                    match field.trim().replace(',', "").parse::<f64>() {
                        Ok(n) => CellValue::Number(n),
                        Err(_) => CellValue::String(field.to_string()),
                    }
                };
                if !matches!(value, CellValue::Empty) {
                    data_row.values.insert(header.clone(), value);
                }
            }
        }

        if !data_row.values.is_empty() {
            sheet.rows.push(data_row);
        }
    }

    // Auto-detect column types
    for col in &sheet.headers {
        let detected = detect_column_type(&sheet.rows, col);
        sheet.column_types.insert(col.clone(), detected);
    }

    Ok(sheet)
}

/// Load either Excel or CSV based on file extension
pub fn load_any<P: AsRef<Path>>(
    file_path: P,
    sheet_name: Option<&str>,
    csv_options: Option<&CsvOptions>,
    header_row: Option<usize>,
) -> anyhow::Result<SheetData> {
    let path = file_path.as_ref();
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" | "tsv" | "txt" => load_csv_sheet(path, csv_options, header_row),
        "xlsx" | "xls" => load_excel_sheet(path, sheet_name, header_row),
        _ => {
            // Try Excel first, fallback to CSV
            if let Ok(result) = load_excel_sheet(path, sheet_name, header_row) {
                Ok(result)
            } else {
                load_csv_sheet(path, csv_options, header_row)
            }
        }
    }
}

/// Normalize headers: trim, lowercase, replace spaces/special chars with underscore
pub fn normalize_headers(headers: &[String]) -> Vec<String> {
    let mut seen: HashMap<String, i32> = HashMap::new();
    let mut result = Vec::new();

    for header in headers {
        let normalized = header.trim()
            .to_lowercase()
            .replace([' ', '-', '.', '(', ')', '[', ']', '/', '\\', '#'], "_")
            .replace(&[':', ';'], "")
            .replace("__", "_")
            .trim_start_matches('_')
            .trim_end_matches('_')
            .to_string();

        let final_name = if normalized.is_empty() {
            format!("column_{}", result.len() + 1)
        } else {
            let count = seen.entry(normalized.clone()).or_insert(0);
            if *count > 0 {
                let new_name = format!("{}_{}", normalized, *count + 1);
                *count += 1;
                new_name
            } else {
                *count += 1;
                normalized
            }
        };

        result.push(final_name);
    }

    result
}

/// Parse calamine range into SheetData, handling merged cells
fn parse_sheet(
    range: calamine::Range<Data>,
    sheet_name: &str,
    header_row: usize,
) -> anyhow::Result<SheetData> {
    let mut sheet = SheetData::new(sheet_name);
    let rows: Vec<_> = range.rows().collect();

    if rows.is_empty() {
        return Ok(sheet);
    }

    // Parse headers from specified row (1-based)
    let header_idx = header_row.saturating_sub(1);
    if header_idx < rows.len() {
        let raw_headers: Vec<String> = rows[header_idx]
            .iter()
            .map(|cell| cell.to_string().trim().to_string())
            .collect();
        sheet.headers = normalize_headers(&raw_headers);
    } else {
        let col_count = rows[0].len();
        sheet.headers = (0..col_count)
            .map(|i| format!("column_{}", i + 1))
            .collect();
    }

    // Parse data rows
    for (i, row) in rows.iter().skip(header_row + 1).enumerate() {
        let row_num = header_row + 2 + i; // 1-based (after header)
        let mut data_row = DataRow::new(row_num);

        for (j, cell) in row.iter().enumerate() {
            if let Some(header) = sheet.headers.get(j) {
                let value = cell_value_from_calamine(cell);
                if !value.is_empty() {
                    data_row.values.insert(header.clone(), value);
                }
            }
        }

        if !data_row.values.is_empty() {
            sheet.rows.push(data_row);
        }
    }

    // Auto-detect column types
    for col in &sheet.headers {
        let detected = detect_column_type(&sheet.rows, col);
        sheet.column_types.insert(col.clone(), detected);
    }

    Ok(sheet)
}

/// Convert calamine cell to our CellValue type
fn cell_value_from_calamine(cell: &Data) -> CellValue {
    match cell {
        Data::String(s) => {
            if s.trim().is_empty() {
                CellValue::Empty
            } else {
                CellValue::String(s.clone())
            }
        }
        Data::Float(f) => CellValue::Number(*f),
        Data::Int(i) => CellValue::Number(*i as f64),
        Data::Bool(b) => CellValue::Boolean(*b),
        Data::DateTime(d) => {
            // Use as_datetime() which returns Option<chrono::NaiveDateTime>
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
        Data::DateTimeIso(dt) => {
            // dt is already a String in ISO format
            CellValue::Date(dt.clone())
        }
        Data::DurationIso(d) => CellValue::String(d.clone()),
        Data::Error(_) => CellValue::Empty,
        Data::Empty => CellValue::Empty,
    }
}

/// Detect data type for a column
pub fn detect_column_type(rows: &[DataRow], column: &str) -> ValueType {
    let mut types_seen: HashMap<String, usize> = HashMap::new();

    for row in rows {
        if let Some(value) = row.get(column) {
            let type_str = match value {
                CellValue::Number(_) => "number",
                CellValue::Date(_) => "date",
                CellValue::Boolean(_) => "boolean",
                CellValue::Empty => continue,
                _ => "string",
            };
            *types_seen.entry(type_str.to_string()).or_insert(0) += 1;
        }
    }

    if types_seen.is_empty() {
        return ValueType::String;
    }

    let most_common = types_seen
        .iter()
        .max_by_key(|&(_, count)| count)
        .map(|(t, _)| t.as_str())
        .unwrap_or("string");

    match most_common {
        "number" => ValueType::Number,
        "date" => ValueType::Date,
        "boolean" => ValueType::Boolean,
        _ => ValueType::String,
    }
}
