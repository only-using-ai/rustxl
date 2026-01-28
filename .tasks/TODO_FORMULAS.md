# Formula Implementation TODO

## Currently Implemented (rustxl)

The following formulas are **supported** in rustxl:

| Category | Formula | Notes |
|----------|---------|-------|
| Custom | SHELL | Executes shell commands (not in Excel) |
| Math/Stats | SUM | |
| Math/Stats | AVG | Excel equivalent: AVERAGE |
| Math/Stats | MIN | |
| Math/Stats | MAX | |
| Math/Stats | CORREL | |
| Math/Stats | COUNT | |
| Math/Stats | COUNTA | |
| Math/Stats | COUNTIF | |
| Math/Stats | SUMIF | |
| Math/Stats | AVERAGEIF | |
| Math/Stats | ROUND | |
| Math/Stats | ABS | |
| Logical | IF | |
| Logical | AND | |
| Logical | OR | |
| Logical | NOT | |
| Logical | IFERROR | |
| Text | CONCATENATE | |
| Text | CONCAT | |
| Text | LEFT | |
| Text | RIGHT | |
| Text | MID | |
| Text | LEN | |
| Text | TRIM | |
| Text | UPPER | |
| Text | LOWER | |
| Text | PROPER | |
| Math/Stats | MOD | |
| Math/Stats | SQRT | |
| Math/Stats | POWER | |
| Math/Stats | INT | |
| Math/Stats | PRODUCT | |
| Math/Stats | MEDIAN | |
| Lookup | VLOOKUP | |
| — | Arithmetic | +, -, *, / and cell references |

---

## Excel Formulas Not Yet Implemented

Formulas below are supported by Excel but **not** implemented in rustxl. Grouped by Excel’s categories.

### Logical

| Function | Description |
|----------|-------------|
| IFNA | Returns value if expression is #N/A, else expression result |
| IFS | Returns value for first TRUE condition (Excel 2019+) |
| SWITCH | Evaluates expression against list, returns first match (Excel 2016+) |
| TRUE | Returns logical TRUE |
| FALSE | Returns logical FALSE |
| XOR | Logical exclusive OR (Excel 2013+) |

### Lookup & Reference

| Function | Description |
|----------|-------------|
| HLOOKUP | Lookup in first row, return value from indicated row |
| INDEX | Returns value at row/column index in reference or array |
| MATCH | Returns position of item in range |
| XLOOKUP | Modern lookup; search range, return corresponding value (Excel 2021+) |
| LOOKUP | Vector or array form lookup |
| INDIRECT | Returns reference given by text |
| OFFSET | Reference offset from a given reference |
| CHOOSE | Chooses value from list by index |
| ROW | Row number of reference |
| ROWS | Number of rows in reference |
| COLUMN | Column number of reference |
| COLUMNS | Number of columns in reference |
| ADDRESS | Returns cell address as text |
| TRANSPOSE | Transpose of array |
| FILTER | Filter range by criteria (Excel 2021+) |
| UNIQUE | Unique values in list/range (Excel 2021+) |
| SORT | Sort range or array (Excel 2021+) |

### Math & Trigonometry

| Function | Description |
|----------|-------------|
| AVERAGE | Average of arguments (rustxl has AVG) |
| TRUNC | Truncates to integer |
| ROUNDUP | Rounds up away from zero |
| ROUNDDOWN | Rounds down toward zero |
| FLOOR | Rounds down to multiple |
| CEILING | Rounds up to multiple |
| MROUND | Rounds to desired multiple |
| SUMPRODUCT | Sum of products of corresponding arrays |
| SUMIFS | Sum cells meeting multiple criteria (Excel 2019+) |
| SUBTOTAL | Subtotal (SUM, AVERAGE, etc.) ignoring filtered-out rows |
| SIGN | Sign of number |
| PI | Returns π |
| EXP | e raised to power |
| LN | Natural logarithm |
| LOG | Logarithm to specified base |
| LOG10 | Base-10 logarithm |
| COS, SIN, TAN | Trigonometry |
| ACOS, ASIN, ATAN | Inverse trig |
| DEGREES | Radians to degrees |
| RADIANS | Degrees to radians |
| FACT | Factorial |
| COMBIN | Number of combinations |
| GCD | Greatest common divisor |
| LCM | Least common multiple |
| RAND | Random number 0–1 |
| RANDBETWEEN | Random integer between two numbers |

### Text

| Function | Description |
|----------|-------------|
| FIND | Find text in text (case-sensitive) |
| SEARCH | Find text (case-insensitive) |
| SUBSTITUTE | Replace old text with new text |
| REPLACE | Replace characters at position |
| REPT | Repeat text n times |
| VALUE | Convert text to number |
| TEXT | Format number as text |
| EXACT | Compare two strings (case-sensitive) |
| CHAR | Character from code number |
| CODE | Code of first character |
| TEXTJOIN | Join text with delimiter (Excel 2019+) |
| TEXTAFTER | Text after delimiter (Excel 2024+) |
| TEXTBEFORE | Text before delimiter (Excel 2024+) |
| CLEAN | Remove non-printable characters |
| FIXED | Format number with fixed decimals |
| DOLLAR | Format as currency text |

### Statistical

| Function | Description |
|----------|-------------|
| AVERAGEIFS | Average with multiple criteria (Excel 2019+) |
| COUNTIFS | Count with multiple criteria (Excel 2019+) |
| COUNTBLANK | Count blank cells |
| MODE.SNGL | Most frequent value |
| STDEV.S | Sample standard deviation |
| STDEV.P | Population standard deviation |
| VAR.S | Sample variance |
| VAR.P | Population variance |
| LARGE | k-th largest value |
| SMALL | k-th smallest value |
| PERCENTILE.INC | k-th percentile |
| RANK.EQ | Rank of number in list |
| FORECAST.LINEAR | Linear forecast (Excel 2016+) |
| SLOPE | Slope of regression line |
| INTERCEPT | Intercept of regression line |
| LINEST | Linear trend parameters |

### Date & Time

| Function | Description |
|----------|-------------|
| TODAY | Current date |
| NOW | Current date and time |
| DATE | Serial number from year, month, day |
| DAY | Day of month from serial |
| MONTH | Month from serial |
| YEAR | Year from serial |
| HOUR | Hour from serial time |
| MINUTE | Minute from serial time |
| SECOND | Second from serial time |
| DATEVALUE | Serial number from date text |
| TIMEVALUE | Serial number from time text |
| TIME | Serial time from hour, minute, second |
| DAYS | Days between two dates |
| DATEDIF | Days/months/years between two dates |
| WEEKDAY | Day of week (1–7) |
| WEEKNUM | Week number in year |
| EOMONTH | Last day of month offset |
| EDATE | Date offset by months |
| NETWORKDAYS | Workdays between two dates |
| WORKDAY | Date after n workdays |
| YEARFRAC | Fraction of year between two dates |

### Information

| Function | Description |
|----------|-------------|
| ISBLANK | TRUE if cell is blank |
| ISERROR | TRUE if any error |
| ISERR | TRUE if error except #N/A |
| ISNA | TRUE if #N/A |
| ISNUMBER | TRUE if number |
| ISTEXT | TRUE if text |
| ISLOGICAL | TRUE if TRUE/FALSE |
| ISEVEN | TRUE if even |
| ISODD | TRUE if odd |
| NA | Returns #N/A |
| TYPE | Data type code |
| CELL | Cell format/location/contents |
| ERROR.TYPE | Number for error type |

### Financial (common)

| Function | Description |
|----------|-------------|
| PMT | Loan payment |
| FV | Future value |
| PV | Present value |
| RATE | Interest rate per period |
| NPER | Number of periods |
| NPV | Net present value |
| IRR | Internal rate of return |

### Database (optional)

| Function | Description |
|----------|-------------|
| DSUM | Sum matching database rows |
| DAVERAGE | Average matching rows |
| DCOUNT | Count numbers in matching rows |
| DCOUNTA | Count non-blank in matching rows |
| DMAX | Max in matching rows |
| DMIN | Min in matching rows |
| DGET | Single value matching criteria |

### Compatibility / legacy (optional)

| Function | Description |
|----------|-------------|
| FORECAST | Linear forecast (legacy; prefer FORECAST.LINEAR) |
| STDEV | Sample stdev (legacy; prefer STDEV.S) |
| VAR | Sample variance (legacy; prefer VAR.S) |
| PERCENTILE | Percentile (legacy) |
| QUARTILE | Quartile (legacy) |
| RANK | Rank (legacy) |

---

## Suggested Implementation Priority

1. **High (very common):** AND, OR, NOT, IFERROR, TRIM, UPPER, LOWER, ABS, SUMIFS, COUNTIFS, AVERAGE (alias), TODAY, NOW, DATE, DAY, MONTH, YEAR, ISBLANK, ISNUMBER, ISTEXT, ISERROR.
2. **Medium:** INDEX, MATCH, HLOOKUP, XLOOKUP, SUBSTITUTE, FIND, SEARCH, VALUE, TEXT, MEDIAN, ROUNDUP, ROUNDDOWN, MOD, INT.
3. **Lower:** Date/time (WEEKDAY, EOMONTH, NETWORKDAYS, etc.), financial (PMT, FV, PV), statistical (STDEV, VAR, LARGE, SMALL), database (DSUM, etc.).

---

*Generated from rustxl codebase and [Excel functions by category](https://support.microsoft.com/en-us/office/excel-functions-by-category-5f91f4e9-7b42-46d2-9bd1-63f26a86c0eb).*
