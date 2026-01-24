use crate::spreadsheet::Spreadsheet;

impl Spreadsheet {
    pub fn evaluate_cell(&self, row: usize, col: usize) -> String {
        let content = self.get_cell(row, col);
        if content.starts_with('=') {
            self.evaluate_formula(content)
        } else {
            content.to_string()
        }
    }

    pub fn evaluate_formula(&self, formula: &str) -> String {
        let expr = formula.strip_prefix('=').unwrap_or(formula).trim();
        let expr_upper = expr.to_uppercase();

        // Handle SUM function (case-insensitive)
        if expr_upper.starts_with("SUM(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_sum(inner);
        }

        // Handle IF function (case-insensitive)
        if expr_upper.starts_with("IF(") && expr_upper.ends_with(')') {
            let inner = &expr[3..expr.len() - 1];
            return self.evaluate_if(inner);
        }

        // Handle simple arithmetic
        if let Ok(val) = self.evaluate_arithmetic(expr) {
            return format!("{}", val);
        }

        // Try to parse as cell reference
        if let Some(val) = self.get_cell_value(expr) {
            return format!("{}", val);
        }

        "#ERROR".to_string()
    }

    pub fn evaluate_sum(&self, args: &str) -> String {
        let mut sum = 0.0;

        for arg in args.split(',') {
            let arg = arg.trim();

            if let Some((start, end)) = arg.split_once(':') {
                if let (Some((sr, sc)), Some((er, ec))) =
                    (self.parse_cell_ref(start), self.parse_cell_ref(end))
                {
                    let min_row = sr.min(er);
                    let max_row = sr.max(er);
                    let min_col = sc.min(ec);
                    let max_col = sc.max(ec);
                    for row in min_row..=max_row {
                        for col in min_col..=max_col {
                            if let Some(val) = self.get_cell_value_at(row, col) {
                                sum += val;
                            }
                        }
                    }
                } else {
                    return "#ERROR".to_string();
                }
            } else if let Some(val) = self.get_cell_value(arg) {
                sum += val;
            } else if let Ok(val) = arg.parse::<f64>() {
                sum += val;
            }
        }

        format!("{}", sum)
    }

    pub fn evaluate_if(&self, args: &str) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 3 {
            return "#ERROR".to_string();
        }

        let condition = parts[0].trim();
        let value_true = parts[1].trim();
        let value_false = parts[2].trim();

        let result = self.evaluate_condition(condition);

        match result {
            Some(true) => self.evaluate_arg(value_true),
            Some(false) => self.evaluate_arg(value_false),
            None => "#ERROR".to_string(),
        }
    }

    pub fn split_function_args<'a>(&self, args: &'a str) -> Vec<&'a str> {
        let mut parts = Vec::new();
        let mut depth = 0;
        let mut start = 0;

        for (i, c) in args.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(&args[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        parts.push(&args[start..]);
        parts
    }

    pub fn evaluate_condition(&self, condition: &str) -> Option<bool> {
        let operators = [">=", "<=", "<>", ">", "<", "="];

        for op in operators {
            if let Some(pos) = condition.find(op) {
                let left = condition[..pos].trim();
                let right = condition[pos + op.len()..].trim();

                let left_val = self.evaluate_arg_as_number(left)?;
                let right_val = self.evaluate_arg_as_number(right)?;

                return Some(match op {
                    ">=" => left_val >= right_val,
                    "<=" => left_val <= right_val,
                    "<>" => (left_val - right_val).abs() > f64::EPSILON,
                    ">" => left_val > right_val,
                    "<" => left_val < right_val,
                    "=" => (left_val - right_val).abs() < f64::EPSILON,
                    _ => return None,
                });
            }
        }

        self.evaluate_arg_as_number(condition).map(|v| v.abs() > f64::EPSILON)
    }

    pub fn evaluate_arg(&self, arg: &str) -> String {
        let arg = arg.trim();

        if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
            return arg[1..arg.len() - 1].to_string();
        }

        if arg.contains('(') {
            return self.evaluate_formula(&format!("={}", arg));
        }

        if let Some((row, col)) = self.parse_cell_ref(arg) {
            return self.evaluate_cell(row, col);
        }

        if arg.parse::<f64>().is_ok() {
            return arg.to_string();
        }

        arg.to_string()
    }

    pub fn evaluate_arg_as_number(&self, arg: &str) -> Option<f64> {
        let arg = arg.trim();

        if arg.contains('(') {
            let result = self.evaluate_formula(&format!("={}", arg));
            return result.parse().ok();
        }

        if let Some(val) = self.get_cell_value(arg) {
            return Some(val);
        }

        arg.parse().ok()
    }

    pub fn evaluate_arithmetic(&self, expr: &str) -> Result<f64, ()> {
        let expr = expr.trim();

        for (i, c) in expr.char_indices().rev() {
            if c == '+' || c == '-' {
                if i == 0 {
                    continue;
                }
                let left = self.evaluate_arithmetic(&expr[..i])?;
                let right = self.evaluate_arithmetic(&expr[i + 1..])?;
                return Ok(if c == '+' { left + right } else { left - right });
            }
        }

        for (i, c) in expr.char_indices().rev() {
            if c == '*' || c == '/' {
                let left = self.evaluate_arithmetic(&expr[..i])?;
                let right = self.evaluate_arithmetic(&expr[i + 1..])?;
                return Ok(if c == '*' { left * right } else { left / right });
            }
        }

        if let Ok(n) = expr.parse::<f64>() {
            return Ok(n);
        }

        if let Some(val) = self.get_cell_value(expr) {
            return Ok(val);
        }

        Err(())
    }

    pub fn parse_cell_ref(&self, cell_ref: &str) -> Option<(usize, usize)> {
        let cell_ref = cell_ref.trim().to_uppercase();
        let mut col_str = String::new();
        let mut row_str = String::new();

        for c in cell_ref.chars() {
            if c.is_ascii_alphabetic() {
                col_str.push(c);
            } else if c.is_ascii_digit() {
                row_str.push(c);
            }
        }

        if col_str.is_empty() || row_str.is_empty() {
            return None;
        }

        let col = col_str
            .chars()
            .fold(0, |acc, c| acc * 26 + (c as usize - 'A' as usize + 1))
            - 1;
        let row = row_str.parse::<usize>().ok()? - 1;

        Some((row, col))
    }

    pub fn get_cell_value(&self, cell_ref: &str) -> Option<f64> {
        let (row, col) = self.parse_cell_ref(cell_ref)?;
        self.get_cell_value_at(row, col)
    }

    pub fn get_cell_value_at(&self, row: usize, col: usize) -> Option<f64> {
        let content = self.get_cell(row, col);
        if content.starts_with('=') {
            self.evaluate_formula(content).parse().ok()
        } else {
            content.parse().ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cell_ref() {
        let sheet = Spreadsheet::new();
        assert_eq!(sheet.parse_cell_ref("A1"), Some((0, 0)));
        assert_eq!(sheet.parse_cell_ref("B2"), Some((1, 1)));
        assert_eq!(sheet.parse_cell_ref("Z1"), Some((0, 25)));
        assert_eq!(sheet.parse_cell_ref("a1"), Some((0, 0)));
    }

    #[test]
    fn test_evaluate_sum() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "30".to_string());

        assert_eq!(sheet.evaluate_formula("=SUM(A1:A3)"), "60");
        assert_eq!(sheet.evaluate_formula("=SUM(A3:A1)"), "60"); // reversed range
    }

    #[test]
    fn test_evaluate_if() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());

        assert_eq!(sheet.evaluate_formula("=IF(A1>5,\"yes\",\"no\")"), "yes");
        assert_eq!(sheet.evaluate_formula("=IF(A1<5,\"yes\",\"no\")"), "no");
    }

    #[test]
    fn test_arithmetic() {
        let sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=1+2"), "3");
        assert_eq!(sheet.evaluate_formula("=10-3"), "7");
        assert_eq!(sheet.evaluate_formula("=4*5"), "20");
        assert_eq!(sheet.evaluate_formula("=20/4"), "5");
    }
}
