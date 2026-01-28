# Bug List - rustxl Spreadsheet Application

This document tracks potential bugs and issues found during code audit.

## Critical Bugs

### 1. Circular Reference Detection Missing
**Location:** `src/formula.rs` - `evaluate_formula()`, `evaluate_cell()`
**Severity:** High
**Description:** No detection or prevention of circular references in formulas. A cell referencing itself (e.g., A1 contains `=A1+1`) will cause infinite recursion and stack overflow.
**Example:**
- Cell A1: `=A1+1` → Infinite recursion
- Cell A1: `=B1`, Cell B1: `=A1` → Circular dependency
**Impact:** Application crash, data loss
**Fix Required:** Add visited set tracking during formula evaluation to detect cycles

### 2. Potential Panic from Unwrap() Calls
**Location:** Multiple files
**Severity:** High
**Description:** Multiple `unwrap()` calls that could panic if assumptions fail:
- `src/formula.rs:564-565` - `arg.chars().next().unwrap()` and `arg.chars().last().unwrap()` - called after checking `arg.len() >= 2`, but could still fail with certain Unicode edge cases
- `src/formula.rs:1800, 1803` - `c.to_uppercase().next().unwrap()` - could fail if character has no uppercase equivalent
- `src/input.rs:376, 391` - `c.to_digit(10).unwrap()` - called after checking `is_ascii_digit()`, should be safe but defensive programming needed
- `src/update.rs:109` - `platform_asset.unwrap()` - could panic if platform detection fails
**Impact:** Application crash
**Fix Required:** Replace with proper error handling or use `expect()` with descriptive messages

### 3. Formula Evaluation Recursion Depth Limit Missing
**Location:** `src/formula.rs` - `evaluate_formula()`
**Severity:** Medium-High
**Description:** Deeply nested formulas or long dependency chains could cause stack overflow. No recursion depth limit enforced.
**Example:** `=IF(A1>0, IF(A2>0, IF(A3>0, ...)))` with 1000+ nested IFs
**Impact:** Stack overflow, crash
**Fix Required:** Add recursion depth counter and limit (e.g., max 1000 levels)

### 4. Integer Overflow in Column Name Calculation
**Location:** `src/spreadsheet.rs:437-448` - `col_name()`
**Severity:** Medium
**Description:** Column name calculation could overflow for very large column indices. Uses `c % 26` which wraps, but the division `c / 26 - 1` could overflow.
**Example:** Column index 2^32 would cause issues
**Impact:** Incorrect column names, potential panic
**Fix Required:** Add bounds checking or use checked arithmetic

### 5. Cell Reference Parsing Edge Cases
**Location:** `src/spreadsheet.rs:154-197` - `parse_cell_reference()`, `src/formula.rs:634-658` - `parse_cell_ref()`
**Severity:** Medium
**Description:** 
- Invalid references like "A0", "A-1", "AA0" return `None` but should be validated more strictly
- References beyond spreadsheet bounds are checked in `execute_command()` but not consistently elsewhere
- Column "Z" (25) works, but "AA" (26) calculation might have edge cases
**Impact:** Silent failures, incorrect behavior
**Fix Required:** Add comprehensive validation and consistent bounds checking

## Medium Priority Bugs

### 6. Clipboard Paste Overflow
**Location:** `src/spreadsheet.rs:333-428` - `paste()`, `paste_internal()`, `paste_text()`
**Severity:** Medium
**Description:** Pasting large amounts of data could cause memory issues or integer overflow when expanding `num_rows`/`num_cols`. No limits enforced.
**Example:** Pasting 1 million rows could exhaust memory
**Impact:** Memory exhaustion, potential crash
**Fix Required:** Add reasonable limits (e.g., max 1M rows/cols) or streaming approach

### 7. File Loading Memory Issues
**Location:** `src/spreadsheet.rs:1169-1257` - `load_delimited()`, `load_excel()`
**Severity:** Medium
**Description:** Large CSV/Excel files are loaded entirely into memory. No size limits or streaming support.
**Example:** 10GB CSV file would exhaust memory
**Impact:** Memory exhaustion, crash
**Fix Required:** Add file size checks or implement streaming for large files

### 8. SHELL Function Security Risk
**Location:** `src/formula.rs:1291-1413` - `evaluate_shell()`
**Severity:** Medium-High
**Description:** SHELL function executes arbitrary shell commands without any sandboxing or validation. Malicious formulas could execute dangerous commands.
**Example:** `=SHELL("rm -rf /")` or `=SHELL("cat /etc/passwd")`
**Impact:** Security vulnerability, data loss, system compromise
**Fix Required:** Add command whitelist, sandboxing, or at minimum user warnings

### 9. Update System Security Issues
**Location:** `src/update.rs:146-294` - `download_and_install()`
**Severity:** Medium-High
**Description:** 
- Downloads and executes binaries without signature verification
- No checksum validation
- Replaces executable while running (could fail on Windows)
- No rollback mechanism if update fails
**Impact:** Security vulnerability, potential malware installation
**Fix Required:** Add signature verification, checksum validation, atomic updates, rollback

### 10. Settings File Race Condition
**Location:** `src/settings.rs:64-78` - `save()`
**Severity:** Low-Medium
**Description:** `save()` uses `File::create()` which truncates. If multiple instances run simultaneously, settings could be lost.
**Impact:** Settings loss
**Fix Required:** Use file locking or atomic writes

### 11. Excel File Loading - Multiple Sheets Ignored
**Location:** `src/spreadsheet.rs:1196-1257` - `load_excel()`
**Severity:** Low-Medium
**Description:** Only loads the first worksheet. Other sheets are silently ignored. No user notification.
**Impact:** Data loss, user confusion
**Fix Required:** Warn user or provide option to select sheet

### 12. CSV Escaping Issues
**Location:** `src/save.rs:60-66` - `save_to_file()`
**Severity:** Medium
**Description:** CSV escaping only handles commas, quotes, and newlines. Doesn't handle:
- Embedded tabs in CSV mode (should be escaped)
- Carriage returns
- Complex Unicode edge cases
**Impact:** Corrupted CSV files, data loss
**Fix Required:** Use proper CSV library or improve escaping logic

### 13. Formula Auto-complete Index Out of Bounds
**Location:** `src/spreadsheet.rs:761-784` - `update_formula_suggestions()`
**Severity:** Low-Medium
**Description:** `formula_suggestion_index` is reset if >= suggestions length, but could still cause issues if suggestions change while navigating.
**Impact:** Potential panic or incorrect behavior
**Fix Required:** Add bounds checking before accessing suggestions array

### 14. Row/Column Deletion Cursor Position Bug
**Location:** `src/spreadsheet.rs:931-965` - `delete_selected_rows()`, `delete_selected_columns()`
**Severity:** Medium
**Description:** Cursor position adjustment logic might place cursor out of bounds if deleting all rows/columns or if cursor is at edge.
**Example:** Delete row 0 when cursor is at row 0, then cursor might be at -1 (wrapped to max)
**Impact:** Cursor out of bounds, potential panic
**Fix Required:** Add bounds checking after deletion

### 15. Find Mode Case Sensitivity Inconsistency
**Location:** `src/spreadsheet.rs:237-261` - `update_find_matches()`
**Severity:** Low
**Description:** Find is case-insensitive (uses `to_lowercase()`), but this might not match user expectations. No option for case-sensitive search.
**Impact:** User confusion
**Fix Required:** Add case-sensitive option or document behavior

## Low Priority Bugs / Code Quality Issues

### 16. Arithmetic Evaluation Operator Precedence
**Location:** `src/formula.rs:601-632` - `evaluate_arithmetic()`
**Severity:** Low
**Description:** Processes `+/-` before `*/`, which is correct, but doesn't handle parentheses or exponentiation. Limited functionality.
**Impact:** Limited formula support
**Fix Required:** Add proper expression parser with full operator precedence

### 17. VLOOKUP Range Lookup Logic
**Location:** `src/formula.rs:1119-1210` - `evaluate_vlookup()`
**Severity:** Low-Medium
**Description:** Approximate match logic assumes sorted data and finds last match <= value. This might not match Excel's behavior exactly.
**Impact:** Incorrect results for unsorted data
**Fix Required:** Document behavior or match Excel exactly

### 18. Date/Time Formatting Not Implemented
**Location:** `src/ui.rs:56-63` - `format_cell_by_type()`
**Severity:** Low
**Description:** Date and Time data types are stored but not formatted. Just returns value as-is.
**Impact:** Dates/times not displayed correctly
**Fix Required:** Implement date/time parsing and formatting

### 19. Percentage Formatting Precision
**Location:** `src/ui.rs:49-54` - `format_cell_by_type()`
**Severity:** Low
**Description:** Percentage always formats with 1 decimal place. Should respect cell precision or user preference.
**Impact:** Inconsistent formatting
**Fix Required:** Add precision control

### 20. Empty String vs Empty Cell Inconsistency
**Location:** Multiple locations
**Severity:** Low
**Description:** Empty strings and empty cells are treated differently in some places. `get_cell()` returns `""` for missing cells, but `set_cell()` with `""` removes the cell.
**Impact:** Potential confusion
**Fix Required:** Document and standardize behavior

### 21. Column Width Calculation for Very Long Content
**Location:** `src/ui.rs:419-429` - `render_grid()`
**Severity:** Low
**Description:** Content longer than column width is not truncated or wrapped. Could cause display issues.
**Impact:** UI layout issues
**Fix Required:** Add text truncation or wrapping

### 22. Formula Buffer Overflow Risk
**Location:** `src/spreadsheet.rs:786-818` - `handle_char_input()`
**Severity:** Low
**Description:** `edit_buffer` is a `String` with no size limit. Very long formulas could cause memory issues.
**Impact:** Memory exhaustion (unlikely but possible)
**Fix Required:** Add reasonable length limit (e.g., 10KB)

### 23. Update Checker Network Timeout Too Short
**Location:** `src/update.rs:80` - `check_for_update()`
**Severity:** Low
**Description:** 10 second timeout might be too short for slow networks. Update check silently fails.
**Impact:** Updates not detected on slow networks
**Fix Required:** Increase timeout or make configurable

### 24. Terminal Restoration Not Guaranteed
**Location:** `src/main.rs:143-150` - `main()`
**Severity:** Medium
**Description:** If `run_app()` panics, terminal might not be restored. Terminal state could be left corrupted.
**Impact:** Corrupted terminal state
**Fix Required:** Use `std::panic::set_hook()` or `catch_unwind()` to ensure cleanup

### 25. Settings File Corruption Handling
**Location:** `src/settings.rs:35-58` - `load()`
**Severity:** Low
**Description:** If settings file is corrupted (invalid format), it's silently ignored and defaults used. No user notification.
**Impact:** Settings lost without user knowledge
**Fix Required:** Warn user or attempt to repair

## Edge Cases

### 26. Very Large Spreadsheet Performance
**Location:** Multiple
**Severity:** Medium
**Description:** Spreadsheet with millions of cells could have performance issues. No optimization for sparse data.
**Impact:** Slow performance, UI lag
**Fix Required:** Optimize data structures (already using HashMap, but could improve rendering)

### 27. Unicode Handling in Cell References
**Location:** `src/spreadsheet.rs:154-197` - `parse_cell_reference()`
**Severity:** Low
**Description:** Cell reference parsing uses `is_ascii_alphabetic()` and `is_ascii_digit()`. Non-ASCII characters are rejected, which is correct, but edge cases might exist.
**Impact:** Potential parsing issues
**Fix Required:** Add comprehensive Unicode tests

### 28. Formula Argument Splitting with Nested Functions
**Location:** `src/formula.rs:513-531` - `split_function_args()`
**Severity:** Medium
**Description:** Handles nested parentheses correctly, but might fail with:
- String literals containing commas: `=CONCATENATE("a,b", "c")`
- Nested function calls with complex arguments
**Impact:** Incorrect formula parsing
**Fix Required:** Add tests and handle string literals properly

### 29. SHELL Output Parsing Edge Cases
**Location:** `src/formula.rs:1335-1407` - `evaluate_shell()`
**Severity:** Low
**Description:** Tabular detection heuristic (50% of lines have multiple columns) might fail for:
- Single column data with spaces
- Multi-line single values
- Empty output
**Impact:** Incorrect data placement
**Fix Required:** Improve detection logic or add user control

### 30. Row/Column Insertion Performance
**Location:** `src/spreadsheet.rs:995-1035` - `insert_row_after()`, `insert_column_after()`
**Severity:** Low-Medium
**Description:** Shifts all cells manually in nested loops. For large spreadsheets, this could be slow.
**Impact:** Slow insertion operations
**Fix Required:** Optimize or add progress indication

## Summary

- **Critical:** 5 bugs (circular refs, panics, recursion, overflow, parsing)
- **Medium:** 10 bugs (memory, security, file handling, state management)
- **Low:** 15 bugs (code quality, edge cases, performance)

**Total:** 30 identified bugs/issues

**Priority Fixes:**
1. Circular reference detection (#1)
2. SHELL function security (#8)
3. Update system security (#9)
4. Panic prevention (#2)
5. Terminal restoration (#24)
