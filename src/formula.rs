use crate::spreadsheet::Spreadsheet;

impl Spreadsheet {
    pub fn evaluate_cell(&mut self, row: usize, col: usize) -> String {
        let content = self.get_cell(row, col).to_string();
        if content.starts_with('=') {
            self.evaluate_formula(&content, row, col)
        } else {
            content
        }
    }

    pub fn evaluate_formula(&mut self, formula: &str, row: usize, col: usize) -> String {
        let expr = formula.strip_prefix('=').unwrap_or(formula).trim();
        let expr_upper = expr.to_uppercase();

        // Handle SHELL function (case-insensitive)
        if expr_upper.starts_with("SHELL(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_shell(inner, row, col);
        }

        // Handle SUM function (case-insensitive)
        if expr_upper.starts_with("SUM(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_sum(inner);
        }

        // Handle AVG function (case-insensitive)
        if expr_upper.starts_with("AVG(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_avg(inner);
        }

        // Handle IF function (case-insensitive)
        if expr_upper.starts_with("IF(") && expr_upper.ends_with(')') {
            let inner = &expr[3..expr.len() - 1];
            return self.evaluate_if(inner, row, col);
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

    pub fn evaluate_sum(&mut self, args: &str) -> String {
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

    pub fn evaluate_avg(&mut self, args: &str) -> String {
        let mut sum = 0.0;
        let mut count = 0.0;

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
                                count += 1.0;
                            }
                        }
                    }
                } else {
                    return "#ERROR".to_string();
                }
            } else if let Some(val) = self.get_cell_value(arg) {
                sum += val;
                count += 1.0;
            } else if let Ok(val) = arg.parse::<f64>() {
                sum += val;
                count += 1.0;
            }
        }

        if count == 0.0 {
            return "#ERROR".to_string();
        }

        let avg = sum / count;
        // Format with reasonable precision
        if avg.fract() == 0.0 {
            format!("{:.0}", avg)
        } else {
            format!("{}", avg)
        }
    }

    pub fn evaluate_if(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 3 {
            return "#ERROR".to_string();
        }

        let condition = parts[0].trim();
        let value_true = parts[1].trim();
        let value_false = parts[2].trim();

        let result = self.evaluate_condition(condition, current_row, current_col);

        match result {
            Some(true) => self.evaluate_arg(value_true, current_row, current_col),
            Some(false) => self.evaluate_arg(value_false, current_row, current_col),
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

    pub fn evaluate_condition(&mut self, condition: &str, current_row: usize, current_col: usize) -> Option<bool> {
        let operators = [">=", "<=", "<>", ">", "<", "="];

        for op in operators {
            if let Some(pos) = condition.find(op) {
                let left = condition[..pos].trim();
                let right = condition[pos + op.len()..].trim();

                let left_val = self.evaluate_arg_as_number(left, current_row, current_col)?;
                let right_val = self.evaluate_arg_as_number(right, current_row, current_col)?;

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

        self.evaluate_arg_as_number(condition, current_row, current_col).map(|v| v.abs() > f64::EPSILON)
    }

    pub fn evaluate_arg(&mut self, arg: &str, current_row: usize, current_col: usize) -> String {
        let arg = arg.trim();

        // Check for matching quotes (single or double)
        if arg.len() >= 2 {
            let first_char = arg.chars().next().unwrap();
            let last_char = arg.chars().last().unwrap();
            if (first_char == '"' && last_char == '"') || (first_char == '\'' && last_char == '\'') {
                return arg[1..arg.len() - 1].to_string();
            }
        }

        if arg.contains('(') {
            return self.evaluate_formula(&format!("={}", arg), current_row, current_col);
        }

        if let Some((row, col)) = self.parse_cell_ref(arg) {
            return self.evaluate_cell(row, col);
        }

        if arg.parse::<f64>().is_ok() {
            return arg.to_string();
        }

        arg.to_string()
    }

    pub fn evaluate_arg_as_number(&mut self, arg: &str, current_row: usize, current_col: usize) -> Option<f64> {
        let arg = arg.trim();

        if arg.contains('(') {
            let result = self.evaluate_formula(&format!("={}", arg), current_row, current_col);
            return result.parse().ok();
        }

        if let Some(val) = self.get_cell_value(arg) {
            return Some(val);
        }

        arg.parse().ok()
    }

    pub fn evaluate_arithmetic(&mut self, expr: &str) -> Result<f64, ()> {
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

    pub fn get_cell_value(&mut self, cell_ref: &str) -> Option<f64> {
        let (row, col) = self.parse_cell_ref(cell_ref)?;
        self.get_cell_value_at(row, col)
    }

    pub fn get_cell_value_at(&mut self, row: usize, col: usize) -> Option<f64> {
        let content = self.get_cell(row, col).to_string();
        if content.starts_with('=') {
            // SHELL formulas don't return numeric values, so skip them
            let expr = content.strip_prefix('=').unwrap_or(&content).trim();
            let expr_upper = expr.to_uppercase();
            if expr_upper.starts_with("SHELL(") {
                return None;
            }
            // For other formulas, evaluate and try to parse as number
            self.evaluate_formula(&content, row, col).parse().ok()
        } else {
            content.parse().ok()
        }
    }

    pub fn evaluate_shell(&mut self, args: &str, start_row: usize, start_col: usize) -> String {
        // Parse the command argument - handle quoted strings (single or double quotes)
        let command = if args.len() >= 2 {
            let first_char = args.chars().next().unwrap();
            let last_char = args.chars().last().unwrap();
            if (first_char == '"' && last_char == '"') || (first_char == '\'' && last_char == '\'') {
                args[1..args.len() - 1].to_string()
            } else {
                args.trim().to_string()
            }
        } else {
            args.trim().to_string()
        };

        if command.is_empty() {
            return "#ERROR".to_string();
        }

        // Execute the shell command
        let output = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(["/C", &command])
                .output()
        } else {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
        };

        match output {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return format!("#ERROR: {}", stderr.trim());
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                let output_text = stdout.trim();

                if output_text.is_empty() {
                    return "OK".to_string();
                }

                // Try to detect if output is tabular (multiple columns)
                let lines: Vec<&str> = output_text.lines().collect();
                if lines.is_empty() {
                    return "OK".to_string();
                }

                // Check if it looks like a table (multiple columns separated by whitespace)
                // Consider it tabular if at least 50% of non-empty lines have multiple columns
                let non_empty_lines: Vec<&str> = lines.iter()
                    .filter(|line| !line.trim().is_empty())
                    .copied()
                    .collect();
                
                if non_empty_lines.is_empty() {
                    return "OK".to_string();
                }

                let multi_col_lines = non_empty_lines.iter()
                    .filter(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        parts.len() > 1
                    })
                    .count();
                
                let is_tabular = multi_col_lines * 2 >= non_empty_lines.len(); // At least 50%

                if is_tabular {
                    // Parse as table - split by whitespace
                    let mut current_row = start_row;
                    let mut max_cols = 0;
                    
                    for line in lines {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if !parts.is_empty() {
                            max_cols = max_cols.max(parts.len());
                            let mut current_col = start_col;
                            for part in parts {
                                // Only write if we're in the first row or if the cell is empty
                                // This prevents overwriting existing data in subsequent rows
                                if current_row == start_row || self.get_cell(current_row, current_col).is_empty() {
                                    self.set_cell(current_row, current_col, part.to_string());
                                }
                                current_col += 1;
                            }
                            current_row += 1;
                        }
                    }
                    
                    // Update dimensions if needed
                    if current_row > self.num_rows {
                        self.num_rows = current_row;
                    }
                    if start_col + max_cols > self.num_cols {
                        self.num_cols = start_col + max_cols;
                    }
                    "OK".to_string()
                } else {
                    // Simple text output - write to current cell
                    // If it's multi-line but not tabular, join with spaces or keep as-is
                    let text = if lines.len() > 1 {
                        // For multi-line non-tabular output, join with newlines
                        output_text.to_string()
                    } else {
                        output_text.to_string()
                    };
                    self.set_cell(start_row, start_col, text);
                    "OK".to_string()
                }
            }
            Err(e) => {
                format!("#ERROR: {}", e)
            }
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

        assert_eq!(sheet.evaluate_formula("=SUM(A1:A3)", 0, 0), "60");
        assert_eq!(sheet.evaluate_formula("=SUM(A3:A1)", 0, 0), "60"); // reversed range
    }

    #[test]
    fn test_evaluate_avg() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "30".to_string());

        assert_eq!(sheet.evaluate_formula("=AVG(A1:A3)", 0, 0), "20");
        assert_eq!(sheet.evaluate_formula("=AVG(A3:A1)", 0, 0), "20"); // reversed range
        assert_eq!(sheet.evaluate_formula("=AVG(10,20,30)", 0, 0), "20");
        assert_eq!(sheet.evaluate_formula("=AVG(5,10)", 0, 0), "7.5");
    }

    #[test]
    fn test_evaluate_if() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());

        assert_eq!(sheet.evaluate_formula("=IF(A1>5,\"yes\",\"no\")", 0, 0), "yes");
        assert_eq!(sheet.evaluate_formula("=IF(A1<5,\"yes\",\"no\")", 0, 0), "no");
        // Test single quotes
        assert_eq!(sheet.evaluate_formula("=IF(A1>5,'yes','no')", 0, 0), "yes");
        assert_eq!(sheet.evaluate_formula("=IF(A1<5,'yes','no')", 0, 0), "no");
    }

    #[test]
    fn test_arithmetic() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=1+2", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=10-3", 0, 0), "7");
        assert_eq!(sheet.evaluate_formula("=4*5", 0, 0), "20");
        assert_eq!(sheet.evaluate_formula("=20/4", 0, 0), "5");
    }
}
