use calamine::{open_workbook, Reader, Xlsx, XlsxError};
use std::collections::HashSet;
use std::path::PathBuf;
use thiserror::Error;

/// Error types for Excel operations
#[derive(Debug, Error, Clone)]
pub enum ExcelDiffError {
    #[error("Failed to open workbook: {0}")]
    WorkbookOpenError(String),
    
    #[error("Sheet '{0}' not found")]
    SheetNotFoundError(String),
    
    #[error("Invalid data in sheet: {0}")]
    DataError(String),
    
    #[error("IO error: {0}")]
    IoError(String),
}

/// Result type for Excel operations
pub type ExcelDiffResult<T> = Result<T, ExcelDiffError>;

/// Compare two sheets in an Excel workbook and return mismatches
pub async fn compare_sheets(
    file_path: PathBuf,
    source_sheet: String,
    gl_sheet: String,
    column_index: usize,
    source_limit: usize,
    gl_limit: usize,
) -> ExcelDiffResult<(Vec<(usize, String)>, usize, usize)> {
    // Run the CPU-intensive Excel reading in a blocking thread
    tokio::task::spawn_blocking(move || {
        compare_sheets_blocking(
            file_path,
            source_sheet,
            gl_sheet,
            column_index,
            source_limit,
            gl_limit,
        )
    }).await.map_err(|e: tokio::task::JoinError| ExcelDiffError::DataError(format!("Task failed: {}", e)))?
}

/// Blocking version of sheet comparison (runs on background thread)
fn compare_sheets_blocking(
    file_path: PathBuf,
    source_sheet: String,
    gl_sheet: String,
    column_index: usize,
    source_limit: usize,
    gl_limit: usize,
) -> ExcelDiffResult<(Vec<(usize, String)>, usize, usize)> {
    // Open workbook
    let mut workbook: Xlsx<_> = open_workbook(&file_path)
        .map_err(|e: XlsxError| ExcelDiffError::WorkbookOpenError(e.to_string()))?;
    
    // Get source sheet data
    let source_range = workbook.worksheet_range(&source_sheet)
        .map_err(|e: XlsxError| ExcelDiffError::SheetNotFoundError(format!("{}: {}", source_sheet, e)))?;
    
    // Get GL sheet data  
    let gl_range = workbook.worksheet_range(&gl_sheet)
        .map_err(|e: XlsxError| ExcelDiffError::SheetNotFoundError(format!("{}: {}", gl_sheet, e)))?;
    
    // Extract and clean data from source sheet
    let mut source_data = Vec::new();
    for (i, row) in source_range.rows().skip(1).enumerate().take(source_limit) {
        if let Some(cell) = row.get(column_index) {
            let s = cell.to_string();
            // Clean: split on '-' or ' ' and take the last part
            let cleaned = s.splitn(2, |c| c == '-' || c == ' ')
                .last()
                .unwrap_or(&s)
                .to_string();
            if !cleaned.is_empty() {
                source_data.push((i + 2, cleaned)); // +2 for 1-based row numbering + header skip
            }
        }
    }
    
    // Extract and clean data from GL sheet into HashSet for fast lookup
    let mut gl_set = HashSet::new();
    for row in gl_range.rows().skip(1).take(gl_limit) {
        if let Some(cell) = row.get(column_index) {
            let s = cell.to_string();
            let cleaned = s.splitn(2, |c| c == '-' || c == ' ')
                .last()
                .unwrap_or(&s)
                .to_string();
            if !cleaned.is_empty() {
                gl_set.insert(cleaned);
            }
        }
    }
    
    // Find mismatches (in source but not in GL)
    let mut mismatches = Vec::new();
    for (row_no, item) in source_data {
        if !gl_set.contains(&item) {
            mismatches.push((row_no, item));
        }
    }
    
    let mismatch_count = mismatches.len();
    Ok((mismatches, gl_set.len(), mismatch_count))
}

/// Synchronous version for direct use (if needed)
pub fn compare_sheets_sync(
    file_path: PathBuf,
    source_sheet: String,
    gl_sheet: String,
    column_index: usize,
    source_limit: usize,
    gl_limit: usize,
) -> ExcelDiffResult<(Vec<(usize, String)>, usize, usize)> {
    compare_sheets_blocking(file_path, source_sheet, gl_sheet, column_index, source_limit, gl_limit)
}