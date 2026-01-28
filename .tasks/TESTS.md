# Testing Gap Analysis - rustxl Spreadsheet Application

This document identifies areas where additional tests are needed to ensure code quality and prevent regressions.

## Critical Testing Gaps

### 1. Circular Reference Detection Tests
**Location:** `src/formula.rs`
**Priority:** Critical
**Missing Tests:**
- Self-referencing cell: `A1 = A1+1`
- Direct circular reference: `A1 = B1`, `B1 = A1`
- Multi-cell circular reference: `A1 = B1`, `B1 = C1`, `C1 = A1`
- Circular reference in function arguments: `A1 = SUM(B1, A1)`
- Circular reference in nested functions: `A1 = IF(B1>0, A1, 0)`
**Test Cases Needed:** ~10-15 tests

### 2. Formula Evaluation Error Handling Tests
**Location:** `src/formula.rs`
**Priority:** Critical
**Missing Tests:**
- Division by zero: `=10/0`, `=AVG(10, 0, 0)`
- Invalid cell references: `=A999999`, `=ZZ999999`
- Malformed formulas: `=SUM(`, `=IF(`, `=SUM(A1:A2`
- Invalid function arguments: `=SUM()`, `=IF(1)`, `=ROUND(5)`
- Type mismatches: `=SUM("text", 5)`, `=IF("not a number", 1, 0)`
- Overflow/underflow: `=POWER(10, 1000)`, `=SQRT(-1)`
**Test Cases Needed:** ~20-30 tests

### 3. File I/O Error Handling Tests
**Location:** `src/spreadsheet.rs`, `src/save.rs`
**Priority:** High
**Missing Tests:**
- Non-existent file loading
- Permission denied (read-only file)
- Corrupted CSV/Excel files
- Empty files
- Files with invalid encoding
- Very large files (>100MB)
- Concurrent file access
- Disk full scenarios
- Invalid file paths (special characters, too long)
**Test Cases Needed:** ~15-20 tests

### 4. Update System Tests
**Location:** `src/update.rs`
**Priority:** High
**Missing Tests:**
- Network failure scenarios
- Invalid JSON response from GitHub API
- Missing asset for platform
- Download interruption
- Corrupted download
- Insufficient disk space
- Permission denied during installation
- Executable replacement failure
- Rollback scenarios
**Test Cases Needed:** ~10-15 tests (requires mocking)

### 5. State Management Tests
**Location:** `src/spreadsheet.rs`, `src/input.rs`
**Priority:** High
**Missing Tests:**
- Mode transitions (edit → ready → visual → command)
- State consistency after errors
- Undo/redo operations (if implemented)
- Multiple simultaneous operations
- State persistence across file operations
**Test Cases Needed:** ~20-30 tests

## High Priority Testing Gaps

### 6. Formula Function Edge Cases
**Location:** `src/formula.rs`
**Priority:** High
**Missing Tests:**
- **SUM/AVG/MIN/MAX:** Empty ranges, single cell, very large ranges, mixed types
- **IF:** Nested IFs (10+ levels), complex conditions, all branches
- **VLOOKUP:** Exact vs approximate match, unsorted data, missing values, out of bounds column index
- **COUNTIF/SUMIF:** Complex criteria, wildcards, empty ranges, type mismatches
- **String functions:** Empty strings, Unicode, very long strings (>10KB), special characters
- **Math functions:** Edge values (0, negative, very large, very small, infinity, NaN)
**Test Cases Needed:** ~50-70 tests per function category

### 7. Cell Reference Parsing Tests
**Location:** `src/spreadsheet.rs`, `src/formula.rs`
**Priority:** High
**Missing Tests:**
- Invalid formats: `A0`, `A-1`, `AA0`, `1A`, `A`, `1`
- Boundary cases: `A1`, `Z1`, `AA1`, `ZZ1`, `A999999`
- Case sensitivity: `a1` vs `A1`
- Whitespace handling: ` A1 `, `A 1`
- Invalid characters: `A@1`, `A#1`
- Column overflow: Column 2^32
**Test Cases Needed:** ~20-30 tests

### 8. Clipboard Operations Tests
**Location:** `src/spreadsheet.rs`
**Priority:** High
**Missing Tests:**
- Copy single cell, range, entire row/column
- Cut and paste (verify source cleared)
- Paste at boundaries (row 0, col 0, last row/col)
- Paste overlapping source
- Paste very large ranges
- Multiple paste operations
- Paste from external clipboard (system clipboard)
- Paste with styles
- Paste formulas (relative vs absolute references)
**Test Cases Needed:** ~25-35 tests

### 9. Row/Column Operations Tests
**Location:** `src/spreadsheet.rs`
**Priority:** High
**Missing Tests:**
- Insert/delete at boundaries (row 0, col 0, last row/col)
- Insert/delete multiple rows/columns
- Insert/delete with formulas referencing affected cells
- Insert/delete with selection active
- Delete all rows/columns
- Cursor position after insert/delete
- Style preservation after insert/delete
**Test Cases Needed:** ~20-30 tests

### 10. Selection and Navigation Tests
**Location:** `src/spreadsheet.rs`, `src/input.rs`
**Priority:** High
**Missing Tests:**
- Selection with Shift+Arrow keys
- Selection at boundaries
- Selection with visual mode
- Selection with row/column select mode
- Jump to last/first data cell
- Navigation with empty spreadsheet
- Navigation with very large spreadsheet
- Selection stats calculation edge cases
**Test Cases Needed:** ~25-35 tests

## Medium Priority Testing Gaps

### 11. Formula Autocomplete Tests
**Location:** `src/spreadsheet.rs`
**Priority:** Medium
**Missing Tests:**
- Autocomplete activation/deactivation
- Navigation through suggestions (Up/Down)
- Selection with Enter/Tab
- Backspace behavior
- Case-insensitive matching
- Empty prefix handling
- Very long formula names
**Test Cases Needed:** ~15-20 tests

### 12. Visual Mode and Styling Tests
**Location:** `src/spreadsheet.rs`, `src/style.rs`
**Priority:** Medium
**Missing Tests:**
- Apply styles to single cell vs range
- Style persistence after cell edit
- Style clearing
- Style application to empty cells
- Multiple style properties simultaneously
- Style inheritance (if any)
- Dark mode toggle
**Test Cases Needed:** ~20-30 tests

### 13. Find Mode Tests
**Location:** `src/spreadsheet.rs`
**Priority:** Medium
**Missing Tests:**
- Case-insensitive search
- Empty query
- No matches found
- Multiple matches navigation
- Find with special characters
- Find in formulas vs values
- Find with selection active
**Test Cases Needed:** ~15-20 tests

### 14. Command Mode Tests
**Location:** `src/spreadsheet.rs`
**Priority:** Medium
**Missing Tests:**
- Cell reference navigation (`:A1`, `:B23`)
- Invalid commands
- Quit command (`:q`, `:quit`)
- Empty command buffer
- Command cancellation
**Test Cases Needed:** ~10-15 tests

### 15. File Format Tests
**Location:** `src/spreadsheet.rs`, `src/save.rs`
**Priority:** Medium
**Missing Tests:**
- CSV round-trip (save and reload)
- TSV round-trip
- Excel round-trip (if possible)
- Special characters in CSV (quotes, commas, newlines)
- Empty cells handling
- Large files (>10MB)
- Unicode characters
- Line ending differences (CRLF vs LF)
**Test Cases Needed:** ~20-30 tests

### 16. Settings Management Tests
**Location:** `src/settings.rs`
**Priority:** Medium
**Missing Tests:**
- Settings file creation
- Settings file parsing (valid/invalid)
- Settings persistence
- Default settings
- Missing settings file
- Corrupted settings file
- Concurrent access (if possible)
**Test Cases Needed:** ~10-15 tests

### 17. Arithmetic Expression Tests
**Location:** `src/formula.rs`
**Priority:** Medium
**Missing Tests:**
- Operator precedence: `=1+2*3`, `=10-5/2`
- Parentheses: `=(1+2)*3`
- Negative numbers: `=-5+3`
- Decimal arithmetic: `=0.1+0.2`
- Very large numbers
- Very small numbers (precision)
- Mixed cell references and literals
**Test Cases Needed:** ~15-25 tests

### 18. SHELL Function Tests
**Location:** `src/formula.rs`
**Priority:** Medium-High (Security)
**Missing Tests:**
- Valid command execution
- Command with output
- Command with error
- Empty output
- Multi-line output
- Tabular output detection
- Command with special characters
- Command injection attempts (security)
- Timeout handling (if implemented)
**Test Cases Needed:** ~15-20 tests (requires careful security considerations)

## Low Priority Testing Gaps

### 19. UI Rendering Tests
**Location:** `src/ui.rs`
**Priority:** Low
**Missing Tests:**
- Text truncation/wrapping
- Column width adjustments
- Row height adjustments
- Dark mode rendering
- Color application
- Alignment rendering
- Very long cell content
- Empty spreadsheet rendering
- Large spreadsheet scrolling
**Test Cases Needed:** ~20-30 tests (requires UI testing framework)

### 20. Performance Tests
**Location:** Multiple
**Priority:** Low-Medium
**Missing Tests:**
- Large spreadsheet creation (10K+ cells)
- Formula evaluation performance (1000+ formulas)
- File loading performance (large files)
- Rendering performance (large visible area)
- Memory usage with large spreadsheets
- Clipboard operations with large data
**Test Cases Needed:** ~10-15 benchmarks

### 21. Integration Tests
**Location:** Multiple
**Priority:** Medium
**Missing Tests:**
- Complete workflow: Load → Edit → Save
- Formula workflow: Enter → Reference selection → Evaluate
- Styling workflow: Select → Visual mode → Apply style
- Clipboard workflow: Copy → Navigate → Paste
- File operations workflow: Open → Edit → Save → Reload
- Multi-mode workflow: Edit → Visual → Command → Ready
**Test Cases Needed:** ~15-25 end-to-end tests

### 22. Cross-Platform Tests
**Location:** Multiple
**Priority:** Low-Medium
**Missing Tests:**
- Path handling (Windows vs Unix)
- Line ending handling
- File permissions
- Terminal behavior differences
- Update system platform-specific behavior
- SHELL command execution (Windows vs Unix)
**Test Cases Needed:** ~10-15 platform-specific tests

### 23. Error Message Tests
**Location:** Multiple
**Priority:** Low
**Missing Tests:**
- Error message clarity
- Error message consistency
- Error recovery suggestions
- User-friendly error messages
**Test Cases Needed:** ~10-15 tests

### 24. Boundary Condition Tests
**Location:** Multiple
**Priority:** Medium
**Missing Tests:**
- Empty spreadsheet (no cells)
- Single cell spreadsheet
- Maximum size spreadsheet
- Zero-width columns
- Zero-height rows
- Cursor at (0,0) and (max_row, max_col)
- Formula referencing out-of-bounds cells
**Test Cases Needed:** ~20-30 tests

### 25. Data Type Tests
**Location:** `src/ui.rs`, `src/types.rs`
**Priority:** Low-Medium
**Missing Tests:**
- Data type formatting (Currency, Percentage, Date, Time)
- Data type conversion
- Invalid data type assignments
- Data type persistence
- Data type with formulas
**Test Cases Needed:** ~15-20 tests

## Test Infrastructure Gaps

### 26. Test Coverage Metrics
**Priority:** Medium
**Missing:**
- Code coverage measurement
- Coverage reporting
- Coverage thresholds in CI
- Coverage for each module

### 27. Property-Based Tests
**Priority:** Low-Medium
**Missing:**
- Property-based tests for formula evaluation
- Property-based tests for cell operations
- Fuzzing for file parsing
- Random input generation

### 28. Mock Infrastructure
**Priority:** Medium
**Missing:**
- Mock for file system operations
- Mock for network operations (update checker)
- Mock for terminal/UI operations
- Mock for clipboard operations

### 29. Test Data Fixtures
**Priority:** Low
**Missing:**
- Sample CSV files (various formats)
- Sample Excel files
- Large test datasets
- Edge case test files

### 30. Continuous Integration Tests
**Priority:** Medium
**Missing:**
- Automated test runs on PR
- Cross-platform CI (Windows, macOS, Linux)
- Performance regression tests
- Memory leak detection

## Summary Statistics

**Total Test Gaps Identified:** 30 categories

**Priority Breakdown:**
- **Critical:** 5 categories (~100-150 test cases)
- **High:** 5 categories (~150-200 test cases)
- **Medium:** 10 categories (~200-300 test cases)
- **Low:** 10 categories (~150-200 test cases)

**Estimated Total Test Cases Needed:** ~600-850 tests

**Current Test Count:** ~50-60 tests (based on existing test modules)

**Coverage Gap:** ~90% of needed tests are missing

## Recommended Testing Strategy

1. **Phase 1 (Critical):** Implement circular reference detection and tests (#1)
2. **Phase 2 (High Priority):** Add error handling tests (#2, #3, #4)
3. **Phase 3 (Core Functionality):** Expand formula tests (#6), cell operations (#8, #9)
4. **Phase 4 (Integration):** End-to-end workflow tests (#21)
5. **Phase 5 (Polish):** Edge cases (#24), performance (#20), cross-platform (#22)

## Test Framework Recommendations

- **Unit Tests:** Use Rust's built-in `#[test]` framework (already in use)
- **Integration Tests:** Create `tests/` directory with end-to-end tests
- **Property Tests:** Consider `proptest` crate for property-based testing
- **Mocking:** Consider `mockall` or similar for mocking external dependencies
- **Coverage:** Use `cargo-tarpaulin` or `grcov` for coverage measurement
- **Benchmarks:** Use `criterion` for performance benchmarks
