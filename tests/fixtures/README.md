# Test fixtures

## `three_sheets.xlsx`

A small workbook used by the loader characterization tests in
`src/spreadsheet.rs`. It is deliberately tiny and contains one instance of each
value kind the loader has to handle:

| Sheet   | Contents |
|---------|----------|
| `Alpha` | Text headers, two integers, and `C2 = SUM(B2:B3)` |
| `Beta`  | A boolean, a date, and a float |
| `Gamma` | `B1 = 'Alpha'!B2`, a cross-sheet reference |

Formula cells carry cached results (`Alpha!C2` → `30`, `Gamma!B1` → `10`), which
is what a spreadsheet application writes and therefore what a reader encounters
in practice. A file produced by a library that omits cached values would exercise
a different path, so the fixture is generated and then recalculated.

### Regenerating

```bash
python3 - <<'PY'
from openpyxl import Workbook
from datetime import date

wb = Workbook()
alpha = wb.active
alpha.title = "Alpha"
alpha["A1"], alpha["B1"] = "Name", "Qty"
alpha["A2"], alpha["B2"] = "widget", 10
alpha["A3"], alpha["B3"] = "gadget", 20
alpha["C2"] = "=SUM(B2:B3)"

beta = wb.create_sheet("Beta")
beta["A1"], beta["A2"] = "flag", True
beta["B1"], beta["B2"] = "when", date(2026, 3, 14)
beta["C1"] = 3.5

gamma = wb.create_sheet("Gamma")
gamma["A1"] = "cross"
gamma["B1"] = "='Alpha'!B2"

wb.save("three_sheets_raw.xlsx")
PY

# Recalculate so formula cells gain cached results.
libreoffice --headless --convert-to xlsx --outdir . three_sheets_raw.xlsx
mv three_sheets_raw.xlsx three_sheets.xlsx   # keep the recalculated copy
```

The fixture contains no real-world data.
