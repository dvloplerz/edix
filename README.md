<p align="center">
  <img src="https://raw.githubusercontent.com/dvloplerz/edix/main/edix-demo-screenshot.png" alt="edix demo" width="600">
</p>

# edix — Excel/CSV Reconciliation Engine

Reconcile two sheets in seconds. Built for accountants, auditors, and anyone who has ever stared at two Excel files wondering why the numbers don't match.

## What It Does

- **Key-based matching**: Match rows by one or more key columns (e.g., `invoice_id`, `transaction_id`)
- **Column-level comparison**: Choose which columns to compare, ignore the rest
- **Fuzzy matching**: Tolerate typos with configurable Levenshtein distance
- **Type coercion**: Compare numbers even if one side stored them as text
- **Multiple outputs**: CSV, Markdown, Excel (with highlights), or all three

## Quick Start

```bash
# Compare two sheets by invoice_id, checking amount and date
cargo run -- compare sales.xlsx accounting.xlsx -k invoice_id -C amount,date -o result.csv

# Fuzzy matching on description, threshold 0.85
cargo run -- compare bank.csv gl.csv -m fuzzy --fuzzy-threshold 0.85 -o matches.csv

# Use a config file
cargo run -- init -o edix.toml  # generate template
cargo run -- compare --config edix.toml
```

## Sample Output

```
================================================================================
                          EXTRACTION & COMPARISON SUMMARY
================================================================================
  Source Path    : sample_comparison.xlsx (Sheet1)
  Compare Path   : sample_comparison.xlsx (Sheet1)
  Output Path    : test
================================================================================
  Comparison Logic:
  Key Columns    : ["id", "name"]
  Compare Columns: ["amount", "status", "date"]
  Match Type     : Exact
================================================================================
  RESULTS SUMMARY
================================================================================
  Total Rows Processed  : 5
  Total Mismatches Found: 3

  [Type]: MissingInCompare
  Row(Src): 1
  Data: {"amount": "2500", "date": "2023-01-02", "id": "1", "name": "Alice", "status": "active"}

  [Type]: ValueMismatch
  Row(Src): 3
  Details:
    Column 'amount': Src '1500' != Cmp '1600'
    Column 'status': Src 'active' != Cmp 'inactive'
  Data: {"amount": "1500", "date": "2023-01-03", "id": "3", "name": "Charlie", "status": "active"}

  [Type]: TypeMismatch
  Row(Src): 4
  Details:
    Column 'amount': Src(Number: 2000) != Cmp(Text: "2000")
================================================================================
  Output Files:
    - All: test_output.*
================================================================================
```

## Config File (TOML)

```toml
name = "Monthly Bank Reconciliation"

[source]
path = "bank.xlsx"
sheet = "Bank"

[[compares]]
path = "gl.xlsx"
sheet = "GL"

[output]
format = "All"  # CSV, Markdown, Excel, or All
path = "reconciliation_jan"
include_summary = true

[matching]
key_columns = ["txn_id"]
compare_columns = ["amount", "date", "description"]
match_type = "Exact"

type_coercion = [
  { column = "amount", target_type = "Number" },
  { column = "date", target_type = "Date" },
]

[fuzzy]
enabled = true
threshold = 0.85
columns = ["description"]
```

## Use Cases

| Industry | Source | Compare | Key | Compare Columns |
|----------|--------|---------|-----|-----------------|
| Accounting | Bank statement | General Ledger | `txn_id` | `amount`, `date` |
| E-commerce | Shopify orders | Payment gateway | `order_id` | `total`, `status` |
| Inventory | Warehouse count | ERP system | `sku` | `qty`, `location` |
| HR | Payroll export | Attendance system | `employee_id` | `hours`, `rate` |

## Install

```bash
git clone https://github.com/dvloplerz/edix.git
cd edix
cargo build --release
# Binary at: target/release/edix
```

## License

MIT
