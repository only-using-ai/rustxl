use std::{borrow::Cow, collections::HashSet};

use crate::{formula_ref, shell, spreadsheet::Spreadsheet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum FormulaError {
    Cycle,
    Reference,
}

impl FormulaError {
    fn display(self) -> String {
        match self {
            Self::Cycle => "#CYCLE!".to_string(),
            Self::Reference => formula_ref::ERROR.to_string(),
        }
    }
}

struct FormulaEvaluator<'a> {
    sheet: &'a Spreadsheet,
    active_cells: HashSet<(usize, usize)>,
    // Numeric and range helpers use Option for normal nonnumeric values. Keep
    // a formula error separately so their callers cannot silently treat it as blank.
    error: Option<FormulaError>,
}

enum AggregateInput<'a> {
    Cell(usize, usize),
    Scalar(&'a str),
}

fn binary_operator(expr: &str) -> Option<usize> {
    formula_ref::top_level_delimiters(expr, b"+-")
        .filter(|&index| index > 0)
        .last()
        .or_else(|| formula_ref::top_level_delimiters(expr, b"*/").last())
}

impl Spreadsheet {
    pub fn evaluate_cell(&self, row: usize, col: usize) -> String {
        FormulaEvaluator::new(self).evaluate_cell(row, col)
    }

    #[cfg(test)]
    pub fn evaluate_formula(&self, formula: &str, row: usize, col: usize) -> String {
        FormulaEvaluator::new(self).evaluate_formula(formula, row, col)
    }
}

impl<'sheet> FormulaEvaluator<'sheet> {
    fn new(sheet: &'sheet Spreadsheet) -> Self {
        Self {
            sheet,
            active_cells: HashSet::new(),
            error: None,
        }
    }

    fn evaluate_cell(&mut self, row: usize, col: usize) -> String {
        if let Some(error) = self.error {
            return error.display();
        }
        let content = self.sheet.get_cell(row, col);
        if !content.starts_with('=') {
            return content.to_string();
        }
        if !self.active_cells.insert((row, col)) {
            self.error = Some(FormulaError::Cycle);
            return FormulaError::Cycle.display();
        }
        let result = self.evaluate_formula(content, row, col);
        self.active_cells.remove(&(row, col));
        result
    }

    fn evaluate_formula(&mut self, formula: &str, row: usize, col: usize) -> String {
        if let Some(error) = self.error {
            return error.display();
        }
        let result = self.evaluate_expression(formula, row, col);
        self.error.map_or(result, FormulaError::display)
    }

    fn evaluate_expression(&mut self, formula: &str, row: usize, col: usize) -> String {
        let expr = formula.strip_prefix('=').unwrap_or(formula).trim();
        if let Some(args) = shell::shell_args(expr) {
            return if shell::shell_command(args).is_some() {
                "[SHELL]".to_string()
            } else {
                "#ERROR".to_string()
            };
        }
        if let Ok(value) = expr.parse::<f64>() {
            return value.to_string();
        }
        if let Some(operator) = binary_operator(expr) {
            return self
                .evaluate_binary(expr, operator, row, col)
                .map_or_else(|| "#ERROR".to_string(), |value| value.to_string());
        }
        let expr_upper = expr.to_uppercase();

        // Handle SUM function (case-insensitive)
        if expr_upper.starts_with("SUM(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_sum(inner, (row, col));
        }

        // Handle AVG function (case-insensitive)
        if expr_upper.starts_with("AVG(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_avg(inner, (row, col));
        }

        // Handle MIN function (case-insensitive)
        if expr_upper.starts_with("MIN(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_min(inner, (row, col));
        }

        // Handle MAX function (case-insensitive)
        if expr_upper.starts_with("MAX(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_max(inner, (row, col));
        }

        // Handle CORREL function (case-insensitive)
        if expr_upper.starts_with("CORREL(") && expr_upper.ends_with(')') {
            let inner = &expr[7..expr.len() - 1];
            return self.evaluate_correl(inner, (row, col));
        }

        // Handle IF function (case-insensitive)
        if expr_upper.starts_with("IF(") && expr_upper.ends_with(')') {
            let inner = &expr[3..expr.len() - 1];
            return self.evaluate_if(inner, row, col);
        }

        // Handle COUNT function (case-insensitive)
        if expr_upper.starts_with("COUNT(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_count(inner, (row, col));
        }

        // Handle COUNTA function (case-insensitive)
        if expr_upper.starts_with("COUNTA(") && expr_upper.ends_with(')') {
            let inner = &expr[7..expr.len() - 1];
            return self.evaluate_counta(inner, (row, col));
        }

        // Handle COUNTIF function (case-insensitive)
        if expr_upper.starts_with("COUNTIF(") && expr_upper.ends_with(')') {
            let inner = &expr[8..expr.len() - 1];
            return self.evaluate_countif(inner);
        }

        // Handle SUMIF function (case-insensitive)
        if expr_upper.starts_with("SUMIF(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_sumif(inner);
        }

        // Handle AVERAGEIF function (case-insensitive)
        if expr_upper.starts_with("AVERAGEIF(") && expr_upper.ends_with(')') {
            let inner = &expr[10..expr.len() - 1];
            return self.evaluate_averageif(inner);
        }

        // Handle ROUND function (case-insensitive)
        if expr_upper.starts_with("ROUND(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_round(inner, row, col);
        }

        // Handle CONCATENATE function (case-insensitive)
        if expr_upper.starts_with("CONCATENATE(") && expr_upper.ends_with(')') {
            let inner = &expr[12..expr.len() - 1];
            return self.evaluate_concatenate(inner, row, col);
        }

        // Handle CONCAT function (case-insensitive)
        if expr_upper.starts_with("CONCAT(") && expr_upper.ends_with(')') {
            let inner = &expr[7..expr.len() - 1];
            return self.evaluate_concatenate(inner, row, col);
        }

        // Handle LEFT function (case-insensitive)
        if expr_upper.starts_with("LEFT(") && expr_upper.ends_with(')') {
            let inner = &expr[5..expr.len() - 1];
            return self.evaluate_left(inner, row, col);
        }

        // Handle RIGHT function (case-insensitive)
        if expr_upper.starts_with("RIGHT(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_right(inner, row, col);
        }

        // Handle MID function (case-insensitive)
        if expr_upper.starts_with("MID(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_mid(inner, row, col);
        }

        // Handle LEN function (case-insensitive)
        if expr_upper.starts_with("LEN(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_len(inner, row, col);
        }

        // Handle VLOOKUP function (case-insensitive)
        if expr_upper.starts_with("VLOOKUP(") && expr_upper.ends_with(')') {
            let inner = &expr[8..expr.len() - 1];
            return self.evaluate_vlookup(inner, row, col);
        }

        // Handle AND function (case-insensitive)
        if expr_upper.starts_with("AND(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_and(inner, row, col);
        }

        // Handle OR function (case-insensitive)
        if expr_upper.starts_with("OR(") && expr_upper.ends_with(')') {
            let inner = &expr[3..expr.len() - 1];
            return self.evaluate_or(inner, row, col);
        }

        // Handle ABS function (case-insensitive)
        if expr_upper.starts_with("ABS(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_abs(inner, row, col);
        }

        // Handle TRIM function (case-insensitive)
        if expr_upper.starts_with("TRIM(") && expr_upper.ends_with(')') {
            let inner = &expr[5..expr.len() - 1];
            return self.evaluate_trim(inner, row, col);
        }

        // Handle UPPER function (case-insensitive)
        if expr_upper.starts_with("UPPER(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_upper(inner, row, col);
        }

        // Handle NOT function (case-insensitive)
        if expr_upper.starts_with("NOT(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_not(inner, row, col);
        }

        // Handle LOWER function (case-insensitive)
        if expr_upper.starts_with("LOWER(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_lower(inner, row, col);
        }

        // Handle MOD function (case-insensitive)
        if expr_upper.starts_with("MOD(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_mod(inner, row, col);
        }

        // Handle SQRT function (case-insensitive)
        if expr_upper.starts_with("SQRT(") && expr_upper.ends_with(')') {
            let inner = &expr[5..expr.len() - 1];
            return self.evaluate_sqrt(inner, row, col);
        }

        // Handle POWER function (case-insensitive)
        if expr_upper.starts_with("POWER(") && expr_upper.ends_with(')') {
            let inner = &expr[6..expr.len() - 1];
            return self.evaluate_power(inner, row, col);
        }

        // Handle IFERROR function (case-insensitive)
        if expr_upper.starts_with("IFERROR(") && expr_upper.ends_with(')') {
            let inner = &expr[8..expr.len() - 1];
            return self.evaluate_iferror(inner, row, col);
        }

        // Handle INT function (case-insensitive)
        if expr_upper.starts_with("INT(") && expr_upper.ends_with(')') {
            let inner = &expr[4..expr.len() - 1];
            return self.evaluate_int(inner, row, col);
        }

        // Handle PROPER function (case-insensitive)
        if expr_upper.starts_with("PROPER(") && expr_upper.ends_with(')') {
            let inner = &expr[7..expr.len() - 1];
            return self.evaluate_proper(inner, row, col);
        }

        // Handle PRODUCT function (case-insensitive)
        if expr_upper.starts_with("PRODUCT(") && expr_upper.ends_with(')') {
            let inner = &expr[8..expr.len() - 1];
            return self.evaluate_product(inner, (row, col));
        }

        // Handle MEDIAN function (case-insensitive)
        if expr_upper.starts_with("MEDIAN(") && expr_upper.ends_with(')') {
            let inner = &expr[7..expr.len() - 1];
            return self.evaluate_median(inner, (row, col));
        }

        if let Some((row, col)) = self.parse_cell_ref(expr) {
            return self.evaluate_cell(row, col);
        }

        "#ERROR".to_string()
    }

    fn aggregate_argument<'arg>(
        &mut self,
        arg: &'arg str,
    ) -> Result<impl Iterator<Item = AggregateInput<'arg>> + use<'arg>, ()> {
        let arg = arg.trim();
        let colon = formula_ref::top_level_delimiters(arg, b":").next();
        let (single, range) = if let Some(colon) = colon {
            let start = self.parse_cell_ref(&arg[..colon]);
            let end = self.parse_cell_ref(&arg[colon + 1..]);
            let (Some((sr, sc)), Some((er, ec))) = (start, end) else {
                return Err(());
            };
            let rows = sr.min(er)..=sr.max(er);
            let cols = sc.min(ec)..=sc.max(ec);
            (None, Some((rows, cols)))
        } else {
            let input = match self.parse_cell_ref(arg) {
                Some((row, col)) => AggregateInput::Cell(row, col),
                None => AggregateInput::Scalar(arg),
            };
            (Some(input), None)
        };
        let cells = range.into_iter().flat_map(|(rows, cols)| {
            rows.flat_map(move |row| cols.clone().map(move |col| AggregateInput::Cell(row, col)))
        });
        Ok(single.into_iter().chain(cells))
    }

    fn visit_aggregate_inputs<'args>(
        &mut self,
        args: &'args str,
        mut visit: impl FnMut(&mut Self, AggregateInput<'args>),
    ) -> Result<(), ()> {
        for arg in self.split_function_args(args) {
            for input in self.aggregate_argument(arg)? {
                visit(self, input);
                if self.error.is_some() {
                    return Err(());
                }
            }
        }
        Ok(())
    }

    fn visit_numeric_inputs(
        &mut self,
        args: &str,
        origin: (usize, usize),
        mut visit: impl FnMut(f64),
    ) -> Result<(), ()> {
        self.visit_aggregate_inputs(args, |evaluator, input| {
            if let Some(value) = evaluator.numeric_input(input, origin) {
                visit(value);
            }
        })
    }

    fn numeric_input(
        &mut self,
        input: AggregateInput<'_>,
        (row, col): (usize, usize),
    ) -> Option<f64> {
        match input {
            AggregateInput::Cell(row, col) => self.get_cell_value_at(row, col),
            AggregateInput::Scalar(arg) => self.evaluate_arg_as_number(arg, row, col),
        }
    }

    fn evaluate_sum(&mut self, args: &str, origin: (usize, usize)) -> String {
        let mut sum = 0.0;
        if self
            .visit_numeric_inputs(args, origin, |value| sum += value)
            .is_err()
        {
            return "#ERROR".to_string();
        }
        format!("{}", sum)
    }

    fn evaluate_avg(&mut self, args: &str, origin: (usize, usize)) -> String {
        let mut sum = 0.0;
        let mut count = 0;
        let result = self.visit_numeric_inputs(args, origin, |value| {
            sum += value;
            count += 1;
        });
        if result.is_err() || count == 0 {
            return "#ERROR".to_string();
        }

        let avg = sum / count as f64;
        if avg.fract() == 0.0 {
            format!("{:.0}", avg)
        } else {
            format!("{}", avg)
        }
    }

    fn evaluate_min(&mut self, args: &str, origin: (usize, usize)) -> String {
        let mut min: Option<f64> = None;
        let result = self.visit_numeric_inputs(args, origin, |value| {
            min = Some(min.map_or(value, |min| min.min(value)));
        });
        if result.is_err() {
            return "#ERROR".to_string();
        }

        match min {
            Some(val) if val.fract() == 0.0 => format!("{:.0}", val),
            Some(val) => format!("{}", val),
            None => "#ERROR".to_string(),
        }
    }

    fn evaluate_max(&mut self, args: &str, origin: (usize, usize)) -> String {
        let mut max: Option<f64> = None;
        let result = self.visit_numeric_inputs(args, origin, |value| {
            max = Some(max.map_or(value, |max| max.max(value)));
        });
        if result.is_err() {
            return "#ERROR".to_string();
        }

        match max {
            Some(val) if val.fract() == 0.0 => format!("{:.0}", val),
            Some(val) => format!("{}", val),
            None => "#ERROR".to_string(),
        }
    }

    fn evaluate_correl(&mut self, args: &str, origin: (usize, usize)) -> String {
        let parts = self.split_function_args(args);
        let [first, second] = parts.as_slice() else {
            return "#ERROR".to_string();
        };
        let first = self.aggregate_argument(first);
        let second = self.aggregate_argument(second);
        let (Ok(mut first), Ok(mut second)) = (first, second) else {
            return "#ERROR".to_string();
        };

        let mut pairs = Vec::new();
        let mut equal_lengths = true;
        loop {
            let (left, right) = (first.next(), second.next());
            if left.is_none() && right.is_none() {
                break;
            }
            // Track the range lengths before filtering out nonnumeric values.
            equal_lengths &= left.is_some() && right.is_some();
            let left = left.and_then(|input| self.numeric_input(input, origin));
            let right = right.and_then(|input| self.numeric_input(input, origin));
            if self.error.is_some() {
                return "#ERROR".to_string();
            }
            if let Some(pair) = left.zip(right) {
                pairs.push(pair);
            }
        }
        if !equal_lengths || pairs.is_empty() {
            return "#ERROR".to_string();
        }

        let n = pairs.len() as f64;
        let mean1 = pairs.iter().map(|(left, _)| left).sum::<f64>() / n;
        let mean2 = pairs.iter().map(|(_, right)| right).sum::<f64>() / n;
        let mut covariance = 0.0;
        let mut var1 = 0.0;
        let mut var2 = 0.0;

        for (left, right) in pairs {
            let diff1 = left - mean1;
            let diff2 = right - mean2;
            covariance += diff1 * diff2;
            var1 += diff1 * diff1;
            var2 += diff2 * diff2;
        }

        // Check for zero variance (division by zero)
        if var1 == 0.0 || var2 == 0.0 {
            return "#DIV/0!".to_string();
        }

        let correlation = covariance / (var1.sqrt() * var2.sqrt());

        // Format with reasonable precision
        format!("{:.6}", correlation)
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

    fn split_function_args<'a>(&self, args: &'a str) -> Vec<&'a str> {
        let mut start = 0;
        formula_ref::top_level_delimiters(args, b",")
            .chain(std::iter::once(args.len()))
            .map(|end| {
                let argument = &args[start..end];
                start = end + 1;
                argument
            })
            .collect()
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

        if let Some(literal) = formula_ref::quoted_literal(arg) {
            return literal.to_string();
        }

        if arg.contains('(') {
            return self.evaluate_formula(arg, current_row, current_col);
        }

        if let Some((row, col)) = self.parse_cell_ref(arg) {
            return self.evaluate_cell(row, col);
        }

        if arg.parse::<f64>().is_err()
            && let Some(operator) = binary_operator(arg)
        {
            if let Some(value) = self.evaluate_binary(arg, operator, current_row, current_col) {
                return value.to_string();
            }
            if let Some(error) = self.error {
                return error.display();
            }
        }

        arg.to_string()
    }

    pub fn evaluate_arg_as_number(&mut self, arg: &str, current_row: usize, current_col: usize) -> Option<f64> {
        let arg = arg.trim();

        if formula_ref::quoted_literal(arg).is_some() {
            return None;
        }

        self.evaluate_formula(arg, current_row, current_col)
            .parse()
            .ok()
    }

    fn evaluate_binary(
        &mut self,
        expr: &str,
        operator: usize,
        row: usize,
        col: usize,
    ) -> Option<f64> {
        let left = self.evaluate_arg_as_number(&expr[..operator], row, col);
        let right = self.evaluate_arg_as_number(&expr[operator + 1..], row, col);
        let (left, right) = (left?, right?);
        match expr.as_bytes()[operator] {
            b'+' => Some(left + right),
            b'-' => Some(left - right),
            b'*' => Some(left * right),
            b'/' => Some(left / right),
            _ => None,
        }
    }

    fn parse_cell_ref(&mut self, input: &str) -> Option<(usize, usize)> {
        let input = input.trim();
        if input.eq_ignore_ascii_case(formula_ref::ERROR) {
            self.error.get_or_insert(FormulaError::Reference);
        }
        formula_ref::CellReference::parse(input).map(|cell| (cell.row, cell.col))
    }

    fn raw_cell_with_dependency(&mut self, row: usize, col: usize) -> &'sheet str {
        let content = self.sheet.get_cell(row, col);
        if content.starts_with('=') {
            self.evaluate_cell(row, col);
        }
        content
    }

    pub fn get_cell_value_at(&mut self, row: usize, col: usize) -> Option<f64> {
        self.evaluate_cell(row, col).parse().ok()
    }

    fn evaluate_count(&mut self, args: &str, origin: (usize, usize)) -> String {
        let mut count = 0;
        if self
            .visit_numeric_inputs(args, origin, |_| count += 1)
            .is_err()
        {
            return "#ERROR".to_string();
        }
        format!("{}", count)
    }

    fn evaluate_counta(&mut self, args: &str, (row, col): (usize, usize)) -> String {
        let mut count = 0;
        let result = self.visit_aggregate_inputs(args, |evaluator, input| {
            let nonempty = match input {
                AggregateInput::Cell(row, col) => {
                    !evaluator.raw_cell_with_dependency(row, col).is_empty()
                }
                AggregateInput::Scalar(arg) if formula_ref::quoted_literal(arg).is_some() => true,
                AggregateInput::Scalar(arg) => !evaluator.evaluate_arg(arg, row, col).is_empty(),
            };
            if nonempty {
                count += 1;
            }
        });
        if result.is_err() {
            return "#ERROR".to_string();
        }
        format!("{}", count)
    }

    fn evaluate_countif(&mut self, args: &str) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 2 {
            return "#ERROR".to_string();
        }

        let range = parts[0].trim();
        let criteria = self.resolve_criteria(parts[1]);

        let mut count = 0;

        // Collect all cells from the range
        let cells = self.collect_range_cells(range);
        for (row, col) in cells {
            let cell_value = self.raw_cell_with_dependency(row, col).to_string();
            if self.cell_matches_criteria(&cell_value, &criteria) {
                count += 1;
            }
        }

        format!("{}", count)
    }

    fn evaluate_sumif(&mut self, args: &str) -> String {
        let parts = self.split_function_args(args);
        if parts.len() < 2 || parts.len() > 3 {
            return "#ERROR".to_string();
        }

        let range = parts[0].trim();
        let criteria = self.resolve_criteria(parts[1]);
        let sum_range = if parts.len() == 3 {
            Some(parts[2].trim())
        } else {
            None
        };
        if sum_range.is_some_and(|range| range.eq_ignore_ascii_case(formula_ref::ERROR)) {
            self.error.get_or_insert(FormulaError::Reference);
            return formula_ref::ERROR.to_string();
        }

        // Handle ranges the same way COUNTIF does - iterate directly
        let mut sum = 0.0;
        
        // Process range similar to how COUNTIF does it
        if let Some((start, end)) = range.split_once(':') {
            let start = start.trim();
            let end = end.trim();
            if let (Some((sr, sc)), Some((er, ec))) =
                (self.parse_cell_ref(start), self.parse_cell_ref(end))
            {
                let min_row = sr.min(er);
                let max_row = sr.max(er);
                let min_col = sc.min(ec);
                let max_col = sc.max(ec);
                
                // Determine sum range cells
                let sum_cells: Vec<(usize, usize)> = if let Some(sr) = sum_range {
                    // Parse sum range
                    if let Some((sum_start, sum_end)) = sr.split_once(':') {
                        let sum_start = sum_start.trim();
                        let sum_end = sum_end.trim();
                        if let (Some((ssr, ssc)), Some((ser, sec))) =
                            (self.parse_cell_ref(sum_start), self.parse_cell_ref(sum_end))
                        {
                            let mut cells = Vec::new();
                            let sum_min_row = ssr.min(ser);
                            let sum_max_row = ssr.max(ser);
                            let sum_min_col = ssc.min(sec);
                            let sum_max_col = ssc.max(sec);
                            for row in sum_min_row..=sum_max_row {
                                for col in sum_min_col..=sum_max_col {
                                    cells.push((row, col));
                                }
                            }
                            cells
                        } else {
                            return "#ERROR".to_string();
                        }
                    } else if let Some((row, col)) = self.parse_cell_ref(sr) {
                        vec![(row, col)]
                    } else {
                        return "#ERROR".to_string();
                    }
                } else {
                    // Use criteria range as sum range
                    let mut cells = Vec::new();
                    for row in min_row..=max_row {
                        for col in min_col..=max_col {
                            cells.push((row, col));
                        }
                    }
                    cells
                };
                
                // Check if ranges have same size
                let criteria_size = (max_row - min_row + 1) * (max_col - min_col + 1);
                if criteria_size != sum_cells.len() {
                    return "#ERROR".to_string();
                }
                
                // Iterate through criteria range and sum matching cells
                let mut criteria_idx = 0;
                for row in min_row..=max_row {
                    for col in min_col..=max_col {
                        let cell_value = self.raw_cell_with_dependency(row, col).to_string();
                        if self.cell_matches_criteria(&cell_value, &criteria) {
                            if let Some((sum_row, sum_col)) = sum_cells.get(criteria_idx) {
                                if let Some(val) = self.get_cell_value_at(*sum_row, *sum_col) {
                                    sum += val;
                                }
                            }
                        }
                        criteria_idx += 1;
                    }
                }
            } else {
                return "#ERROR".to_string();
            }
        } else if let Some((row, col)) = self.parse_cell_ref(range) {
            // Single cell range
            let cell_value = self.raw_cell_with_dependency(row, col).to_string();
            if self.cell_matches_criteria(&cell_value, &criteria) {
                if let Some(sr) = sum_range {
                    if let Some((sum_row, sum_col)) = self.parse_cell_ref(sr) {
                        if let Some(val) = self.get_cell_value_at(sum_row, sum_col) {
                            sum += val;
                        }
                    }
                } else {
                    if let Some(val) = self.get_cell_value_at(row, col) {
                        sum += val;
                    }
                }
            }
        } else {
            return "#ERROR".to_string();
        }

        format!("{}", sum)
    }

    fn evaluate_averageif(&mut self, args: &str) -> String {
        let parts = self.split_function_args(args);
        if parts.len() < 2 || parts.len() > 3 {
            return "#ERROR".to_string();
        }

        let range = parts[0].trim();
        let criteria = self.resolve_criteria(parts[1]);
        let avg_range = if parts.len() == 3 {
            Some(parts[2].trim())
        } else {
            None
        };

        let mut sum = 0.0;
        let mut count = 0;

        // Collect cells from criteria range
        let criteria_cells = self.collect_range_cells(range);
        let avg_cells = if let Some(ar) = avg_range {
            self.collect_range_cells(ar)
        } else {
            criteria_cells.clone()
        };

        // Both ranges must have the same size
        if criteria_cells.len() != avg_cells.len() {
            return "#ERROR".to_string();
        }

        for (i, (row, col)) in criteria_cells.iter().enumerate() {
            let cell_value = self.raw_cell_with_dependency(*row, *col).to_string();
            if self.cell_matches_criteria(&cell_value, &criteria) {
                if let Some((avg_row, avg_col)) = avg_cells.get(i) {
                    if let Some(val) = self.get_cell_value_at(*avg_row, *avg_col) {
                        sum += val;
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            return "#DIV/0!".to_string();
        }

        let avg = sum / (count as f64);
        if avg.fract() == 0.0 {
            format!("{:.0}", avg)
        } else {
            format!("{}", avg)
        }
    }

    pub fn evaluate_round(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 2 {
            return "#ERROR".to_string();
        }

        let number_str = parts[0].trim();
        let num_digits_str = parts[1].trim();

        let number = match self.evaluate_arg_as_number(number_str, current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };
        let num_digits = match self.evaluate_arg_as_number(num_digits_str, current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };

        let multiplier = 10_f64.powi(num_digits as i32);
        let rounded = (number * multiplier).round() / multiplier;

        if rounded.fract() == 0.0 {
            format!("{:.0}", rounded)
        } else {
            format!("{}", rounded)
        }
    }

    pub fn evaluate_concatenate(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        let mut result = String::new();

        for part in parts {
            let part = part.trim();
            let value = self.evaluate_arg(part, current_row, current_col);
            result.push_str(&value);
        }

        result
    }

    pub fn evaluate_left(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 2 {
            return "#ERROR".to_string();
        }

        let text_str = parts[0].trim();
        let num_chars_str = parts[1].trim();

        let text = self.evaluate_arg(text_str, current_row, current_col);
        let num_chars = match self.evaluate_arg_as_number(num_chars_str, current_row, current_col) {
            Some(n) => n as usize,
            None => return "#ERROR".to_string(),
        };

        let chars: Vec<char> = text.chars().collect();
        let end = num_chars.min(chars.len());
        chars[..end].iter().collect()
    }

    pub fn evaluate_right(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 2 {
            return "#ERROR".to_string();
        }

        let text_str = parts[0].trim();
        let num_chars_str = parts[1].trim();

        let text = self.evaluate_arg(text_str, current_row, current_col);
        let num_chars = match self.evaluate_arg_as_number(num_chars_str, current_row, current_col) {
            Some(n) => n as usize,
            None => return "#ERROR".to_string(),
        };

        let chars: Vec<char> = text.chars().collect();
        let start = if num_chars > chars.len() {
            0
        } else {
            chars.len() - num_chars
        };
        chars[start..].iter().collect()
    }

    pub fn evaluate_mid(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 3 {
            return "#ERROR".to_string();
        }

        let text_str = parts[0].trim();
        let start_num_str = parts[1].trim();
        let num_chars_str = parts[2].trim();

        let text = self.evaluate_arg(text_str, current_row, current_col);
        let start_num = match self.evaluate_arg_as_number(start_num_str, current_row, current_col) {
            Some(n) => n as usize,
            None => return "#ERROR".to_string(),
        };
        let num_chars = match self.evaluate_arg_as_number(num_chars_str, current_row, current_col) {
            Some(n) => n as usize,
            None => return "#ERROR".to_string(),
        };

        let chars: Vec<char> = text.chars().collect();
        if start_num == 0 || start_num > chars.len() {
            return String::new();
        }

        let start = start_num - 1; // Excel uses 1-based indexing
        let end = (start + num_chars).min(chars.len());
        chars[start..end].iter().collect()
    }

    pub fn evaluate_len(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let text = self.evaluate_arg(args.trim(), current_row, current_col);

        format!("{}", text.chars().count())
    }

    pub fn evaluate_vlookup(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() < 3 || parts.len() > 4 {
            return "#ERROR".to_string();
        }

        let lookup_value_str = parts[0].trim();
        let table_range = parts[1].trim();
        let col_index_num_str = parts[2].trim();
        let approximate = if let Some(mode) = parts.get(3) {
            let Some(approximate) = self.vlookup_mode(mode, current_row, current_col) else {
                return "#ERROR".to_string();
            };
            approximate
        } else {
            true
        };

        let lookup_value = self.evaluate_arg(lookup_value_str, current_row, current_col);
        let col_index = match self.evaluate_arg_as_number(col_index_num_str, current_row, current_col) {
            Some(n) => n as usize,
            None => return "#ERROR".to_string(),
        };

        if col_index == 0 {
            return "#ERROR".to_string();
        }

        // Collect table cells
        let table_cells = self.collect_range_cells(table_range);
        if table_cells.is_empty() {
            return "#N/A".to_string();
        }

        // Find the number of columns in the table
        let first_row = table_cells[0].0;
        let mut max_col = table_cells[0].1;
        for (row, col) in &table_cells {
            if *row == first_row {
                max_col = max_col.max(*col);
            }
        }
        let num_cols = max_col - table_cells[0].1 + 1;

        if col_index > num_cols {
            return "#ERROR".to_string();
        }

        // Find matching row
        let target_col = table_cells[0].1; // First column for lookup
        let mut matching_row = None;
        let lookup_num = lookup_value.parse::<f64>().ok();

        for &(row, col) in &table_cells {
            if col != target_col {
                continue;
            }
            let cell_value = self.evaluate_cell(row, col);
            if let Some(error) = self.error {
                return error.display();
            }
            match (cell_value.parse::<f64>(), lookup_num) {
                (Ok(key), Some(lookup)) if approximate => {
                    if key <= lookup {
                        matching_row = Some(row);
                    } else {
                        break;
                    }
                }
                (Ok(key), Some(lookup)) if key == lookup => {
                    matching_row = Some(row);
                    break;
                }
                (Err(_), _) if cell_value == lookup_value => {
                    matching_row = Some(row);
                    break;
                }
                _ => {}
            }
        }

        if let Some(row) = matching_row {
            let result_col = target_col + col_index - 1;
            self.evaluate_cell(row, result_col)
        } else {
            "#N/A".to_string()
        }
    }

    fn vlookup_mode(&mut self, mode: &str, row: usize, col: usize) -> Option<bool> {
        if formula_ref::quoted_literal(mode.trim()).is_some() {
            return None;
        }
        let value = self.evaluate_arg(mode, row, col);
        if value.eq_ignore_ascii_case("TRUE") {
            return Some(true);
        }
        if value.eq_ignore_ascii_case("FALSE") {
            return Some(false);
        }
        value
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map(|number| number != 0.0)
    }

    fn collect_range_cells(&mut self, range: &str) -> Vec<(usize, usize)> {
        let mut cells = Vec::new();
        let range = range.trim();

        if let Some((start, end)) = range.split_once(':') {
            let start = start.trim();
            let end = end.trim();
            if let (Some((sr, sc)), Some((er, ec))) =
                (self.parse_cell_ref(start), self.parse_cell_ref(end))
            {
                let min_row = sr.min(er);
                let max_row = sr.max(er);
                let min_col = sc.min(ec);
                let max_col = sc.max(ec);
                for row in min_row..=max_row {
                    for col in min_col..=max_col {
                        cells.push((row, col));
                    }
                }
            }
        } else if let Some((row, col)) = self.parse_cell_ref(range) {
            cells.push((row, col));
        }

        cells
    }

    fn resolve_criteria<'arg>(&mut self, criterion: &'arg str) -> Cow<'arg, str> {
        let criterion = criterion.trim();
        if let Some((row, col)) = self.parse_cell_ref(criterion) {
            Cow::Owned(self.evaluate_cell(row, col))
        } else {
            Cow::Borrowed(criterion)
        }
    }

    fn cell_matches_criteria(&self, cell_value: &str, criteria: &str) -> bool {
        let criteria = criteria.trim();

        // Remove quotes if present
        let criteria = if criteria.len() >= 2 {
            let first_char = criteria.chars().next().unwrap();
            let last_char = criteria.chars().last().unwrap();
            if (first_char == '"' && last_char == '"') || (first_char == '\'' && last_char == '\'') {
                &criteria[1..criteria.len() - 1]
            } else {
                criteria
            }
        } else {
            criteria
        };

        // Check for comparison operators
        let operators = [">=", "<=", "<>", ">", "<", "="];
        for op in operators {
            if let Some(pos) = criteria.find(op) {
                let _left = criteria[..pos].trim();
                let right = criteria[pos + op.len()..].trim();

                // Try to parse cell_value as number
                let cell_num = cell_value.parse::<f64>().ok();
                let right_num = right.parse::<f64>().ok();

                if let (Some(cn), Some(rn)) = (cell_num, right_num) {
                    return match op {
                        ">=" => cn >= rn,
                        "<=" => cn <= rn,
                        "<>" => (cn - rn).abs() > f64::EPSILON,
                        ">" => cn > rn,
                        "<" => cn < rn,
                        "=" => (cn - rn).abs() < f64::EPSILON,
                        _ => false,
                    };
                }
            }
        }

        // String comparison (exact match or wildcard)
        if criteria.contains('*') || criteria.contains('?') {
            // Simple wildcard matching
            let _pattern = criteria.replace('*', ".*").replace('?', ".");
            // For now, just do simple contains check
            cell_value.contains(&criteria.replace('*', "").replace('?', ""))
        } else {
            cell_value == criteria
        }
    }

    pub fn evaluate_and(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.is_empty() {
            return "#ERROR".to_string();
        }

        for part in parts {
            let part = part.trim();
            
            // Check if it's a boolean literal (TRUE/FALSE without quotes)
            let part_upper = part.to_uppercase();
            if part_upper == "TRUE" {
                continue; // TRUE, continue to next argument
            }
            if part_upper == "FALSE" {
                return "FALSE".to_string();
            }
            
            // Try to evaluate as a condition (e.g., "5>3")
            if let Some(cond_result) = self.evaluate_condition(part, current_row, current_col) {
                if !cond_result {
                    return "FALSE".to_string();
                }
                continue;
            }
            
            // Try to evaluate as a number
            let value = self.evaluate_arg_as_number(part, current_row, current_col);
            let is_true = if let Some(num) = value {
                num.abs() > f64::EPSILON
            } else {
                // Try to evaluate as text/boolean
                let text = self.evaluate_arg(part, current_row, current_col);
                let text_upper = text.to_uppercase();
                text_upper == "TRUE" || text_upper == "1"
            };

            if !is_true {
                return "FALSE".to_string();
            }
        }

        "TRUE".to_string()
    }

    pub fn evaluate_or(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.is_empty() {
            return "#ERROR".to_string();
        }

        for part in parts {
            let part = part.trim();
            
            // Check if it's a boolean literal (TRUE/FALSE without quotes)
            let part_upper = part.to_uppercase();
            if part_upper == "TRUE" {
                return "TRUE".to_string();
            }
            if part_upper == "FALSE" {
                continue; // FALSE, continue to next argument
            }
            
            // Try to evaluate as a condition (e.g., "5>3")
            if let Some(cond_result) = self.evaluate_condition(part, current_row, current_col) {
                if cond_result {
                    return "TRUE".to_string();
                }
                continue;
            }
            
            // Try to evaluate as a number
            let value = self.evaluate_arg_as_number(part, current_row, current_col);
            let is_true = if let Some(num) = value {
                num.abs() > f64::EPSILON
            } else {
                // Try to evaluate as text/boolean
                let text = self.evaluate_arg(part, current_row, current_col);
                let text_upper = text.to_uppercase();
                text_upper == "TRUE" || text_upper == "1"
            };

            if is_true {
                return "TRUE".to_string();
            }
        }

        "FALSE".to_string()
    }

    pub fn evaluate_abs(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let number = match self.evaluate_arg_as_number(parts[0].trim(), current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };

        let abs_value = number.abs();
        if abs_value.fract() == 0.0 {
            format!("{:.0}", abs_value)
        } else {
            format!("{}", abs_value)
        }
    }

    pub fn evaluate_trim(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let text = self.evaluate_arg(parts[0].trim(), current_row, current_col);

        // TRIM removes leading and trailing spaces, and reduces multiple spaces to single spaces
        let trimmed: String = text
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");

        trimmed
    }

    pub fn evaluate_upper(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let text = self.evaluate_arg(parts[0].trim(), current_row, current_col);

        text.to_uppercase()
    }

    pub fn evaluate_not(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let arg = parts[0].trim();
        
        // Check if it's a boolean literal (TRUE/FALSE without quotes)
        let arg_upper = arg.to_uppercase();
        if arg_upper == "TRUE" {
            return "FALSE".to_string();
        }
        if arg_upper == "FALSE" {
            return "TRUE".to_string();
        }
        
        // Try to evaluate as a condition (e.g., "5>3")
        if let Some(cond_result) = self.evaluate_condition(arg, current_row, current_col) {
            return if cond_result { "FALSE".to_string() } else { "TRUE".to_string() };
        }
        
        // Try to evaluate as a number
        let value = self.evaluate_arg_as_number(arg, current_row, current_col);
        let is_true = if let Some(num) = value {
            num.abs() > f64::EPSILON
        } else {
            // Try to evaluate as text/boolean
            let text = self.evaluate_arg(arg, current_row, current_col);
            let text_upper = text.to_uppercase();
            text_upper == "TRUE" || text_upper == "1"
        };

        if is_true {
            "FALSE".to_string()
        } else {
            "TRUE".to_string()
        }
    }

    pub fn evaluate_lower(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let text = self.evaluate_arg(parts[0].trim(), current_row, current_col);

        text.to_lowercase()
    }

    pub fn evaluate_mod(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 2 {
            return "#ERROR".to_string();
        }

        let number = match self.evaluate_arg_as_number(parts[0].trim(), current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };
        let divisor = match self.evaluate_arg_as_number(parts[1].trim(), current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };

        if divisor == 0.0 {
            return "#DIV/0!".to_string();
        }

        let result = number % divisor;
        if result.fract() == 0.0 {
            format!("{:.0}", result)
        } else {
            format!("{}", result)
        }
    }

    pub fn evaluate_sqrt(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let number = match self.evaluate_arg_as_number(parts[0].trim(), current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };

        if number < 0.0 {
            return "#NUM!".to_string();
        }

        let result = number.sqrt();
        if result.fract() == 0.0 {
            format!("{:.0}", result)
        } else {
            format!("{}", result)
        }
    }

    pub fn evaluate_power(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 2 {
            return "#ERROR".to_string();
        }

        let base = match self.evaluate_arg_as_number(parts[0].trim(), current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };
        let exponent = match self.evaluate_arg_as_number(parts[1].trim(), current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };

        let result = base.powf(exponent);
        
        // Check for invalid results (NaN or Infinity)
        if result.is_nan() || result.is_infinite() {
            return "#NUM!".to_string();
        }

        if result.fract() == 0.0 {
            format!("{:.0}", result)
        } else {
            format!("{}", result)
        }
    }

    pub fn evaluate_iferror(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 2 {
            return "#ERROR".to_string();
        }

        let value = parts[0].trim();
        let value_if_error = parts[1].trim();

        // Try to evaluate the first argument as a formula if it contains operators or functions
        let result = if formula_ref::quoted_literal(value).is_none()
            && (value.contains('(') || binary_operator(value).is_some())
        {
            self.evaluate_formula(value, current_row, current_col)
        } else {
            // Try as regular argument
            self.evaluate_arg(value, current_row, current_col)
        };
        
        if self.error == Some(FormulaError::Cycle) {
            return result;
        }
        // IFERROR may handle a deleted reference like other formula errors.
        self.error = None;
        // Check if result is an error (starts with #) or infinity/NaN
        if result.starts_with("#") || result == "inf" || result == "-inf" || result == "nan" || result == "NaN" {
            // It's an error, return the value_if_error
            self.evaluate_arg(value_if_error, current_row, current_col)
        } else {
            // Not an error, return the result
            result
        }
    }

    pub fn evaluate_int(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let number = match self.evaluate_arg_as_number(parts[0].trim(), current_row, current_col) {
            Some(n) => n,
            None => return "#ERROR".to_string(),
        };

        // INT rounds down toward negative infinity (floor for positive, but different for negative)
        let result = if number >= 0.0 {
            number.floor()
        } else {
            number.floor() // floor() already rounds down for negative numbers
        };

        format!("{:.0}", result)
    }

    pub fn evaluate_proper(&mut self, args: &str, current_row: usize, current_col: usize) -> String {
        let parts = self.split_function_args(args);
        if parts.len() != 1 {
            return "#ERROR".to_string();
        }

        let text = self.evaluate_arg(parts[0].trim(), current_row, current_col);

        // PROPER converts to title case: first letter of each word uppercase, rest lowercase
        let mut result = String::new();
        let mut capitalize_next = true;
        
        for c in text.chars() {
            if c.is_whitespace() {
                result.push(c);
                capitalize_next = true;
            } else if capitalize_next {
                result.push(c.to_uppercase().next().unwrap());
                capitalize_next = false;
            } else {
                result.push(c.to_lowercase().next().unwrap());
            }
        }
        
        result
    }

    fn evaluate_product(&mut self, args: &str, origin: (usize, usize)) -> String {
        let mut product = 1.0;
        let mut has_values = false;
        let result = self.visit_numeric_inputs(args, origin, |value| {
            product *= value;
            has_values = true;
        });
        if result.is_err() || !has_values {
            return "#ERROR".to_string();
        }

        if product.fract() == 0.0 {
            format!("{:.0}", product)
        } else {
            format!("{}", product)
        }
    }

    fn evaluate_median(&mut self, args: &str, origin: (usize, usize)) -> String {
        let mut values = Vec::new();
        let result = self.visit_numeric_inputs(args, origin, |value| values.push(value));
        if result.is_err() || values.is_empty() {
            return "#ERROR".to_string();
        }

        // Sort values
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let len = values.len();
        let median = if len % 2 == 0 {
            // Even number of values: average of two middle values
            (values[len / 2 - 1] + values[len / 2]) / 2.0
        } else {
            // Odd number of values: middle value
            values[len / 2]
        };

        if median.fract() == 0.0 {
            format!("{:.0}", median)
        } else {
            format!("{}", median)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cell_ref() {
        let sheet = Spreadsheet::new();
        let mut evaluator = FormulaEvaluator::new(&sheet);
        assert_eq!(evaluator.parse_cell_ref("A1"), Some((0, 0)));
        assert_eq!(evaluator.parse_cell_ref("B2"), Some((1, 1)));
        assert_eq!(evaluator.parse_cell_ref("Z1"), Some((0, 25)));
        assert_eq!(evaluator.parse_cell_ref("a1"), Some((0, 0)));
        assert_eq!(evaluator.parse_cell_ref("$aa$5"), Some((4, 26)));
        assert_eq!(evaluator.parse_cell_ref("A0"), None);
        assert_eq!(evaluator.parse_cell_ref("1A"), None);
        assert_eq!(evaluator.parse_cell_ref("AAAAAAAAAAAAAAAAAAAAAAAA1"), None);
    }

    #[test]
    fn circular_reference_propagates_through_value_paths() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(1, 0, "5".into());
        for formula in [
            "=A1",
            "=A1+1",
            "=SUM(A1:A2)",
            "=ABS(A1)",
            "=ROUND(SUM(A1:A2),0)",
            "=COUNTA(A1:A2)",
            "=COUNTIF(A1,\"x\")",
            "=CONCAT(A1,\" text\")",
            "=IFERROR(A1,\"fallback\")",
        ] {
            sheet.set_cell(0, 0, formula.into());
            assert_eq!(sheet.evaluate_cell(0, 0), "#CYCLE!", "{formula}");
        }
    }

    #[test]
    fn indirect_cycle_recovers_after_edit_and_if_keeps_unused_branch_lazy() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "=B1".into());
        sheet.set_cell(0, 1, "=IF(1,SUM(A1:A1),5)".into());
        sheet.set_cell(0, 2, "=SUM(B1,B1)".into());
        sheet.set_cell(0, 3, "=IF(0,D1,7)".into());

        for col in 0..3 {
            assert_eq!(sheet.evaluate_cell(0, col), "#CYCLE!");
        }
        assert_eq!(sheet.evaluate_cell(0, 3), "7");

        sheet.set_cell(0, 0, "2".into());
        assert_eq!(sheet.evaluate_cell(0, 1), "2");
        assert_eq!(sheet.evaluate_cell(0, 2), "4");
    }

    #[test]
    fn nested_aggregate_arguments_detect_cycles_and_keep_if_lazy() {
        let mut sheet = Spreadsheet::new();
        for (function, count_or_value) in [
            ("SUM", "7"),
            ("AVG", "7"),
            ("MIN", "7"),
            ("MAX", "7"),
            ("COUNT", "1"),
            ("COUNTA", "1"),
            ("PRODUCT", "7"),
            ("MEDIAN", "7"),
        ] {
            sheet.set_cell(0, 0, format!("={function}(ABS(A1))"));
            assert_eq!(sheet.evaluate_cell(0, 0), "#CYCLE!", "{function}");
            sheet.set_cell(0, 0, format!("={function}(IF(0,A1,7))"));
            assert_eq!(sheet.evaluate_cell(0, 0), count_or_value, "{function}");
        }

        sheet.set_cell(0, 0, "=CORREL(ABS(A1),B1:B2)".into());
        assert_eq!(sheet.evaluate_cell(0, 0), "#CYCLE!");
        let formula = r#"=SUM(IF(1,7,CONCAT("),:",A1)),8)"#;
        sheet.set_cell(0, 0, formula.into());
        assert_eq!(sheet.evaluate_cell(0, 0), "15");
    }

    #[test]
    fn deleted_references_propagate_from_nested_aggregate_arguments() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(1, 0, "-7".into());
        let functions = [
            "SUM", "AVG", "MIN", "MAX", "COUNT", "COUNTA", "PRODUCT", "MEDIAN",
        ];
        for (col, function) in functions.iter().enumerate() {
            sheet.set_cell(0, col, format!("={function}(ABS(A2))"));
        }
        sheet.set_cell(0, functions.len(), "=SUM(IF(0,A2,7))".into());
        sheet.selected_rows = Some((1, 1));
        sheet.delete_selected_rows();

        for (col, function) in functions.iter().enumerate() {
            assert_eq!(sheet.get_cell(0, col), format!("={function}(ABS(#REF!))"));
            assert_eq!(sheet.evaluate_cell(0, col), "#REF!", "{function}");
        }
        assert_eq!(sheet.get_cell(0, functions.len()), "=SUM(IF(0,#REF!,7))");
        assert_eq!(sheet.evaluate_cell(0, functions.len()), "7");
    }

    #[test]
    fn aggregate_scalar_arithmetic_shares_dependency_tracking() {
        let mut sheet = Spreadsheet::new();
        for (formula, result) in [
            ("=SUM(1+A1)", "#CYCLE!"),
            ("=COUNT(A1*2)", "#CYCLE!"),
            ("=COUNTA(1/A1)", "#CYCLE!"),
            ("=COUNT(ABS(1+A1))", "#CYCLE!"),
            ("=SUM(1+ABS(A1))", "#CYCLE!"),
            ("=SUM(IF(1,1+A1,7))", "#CYCLE!"),
            ("=SUM(IF(0,1+A1,7))", "7"),
            ("=SUM(1+#REF!)", "#REF!"),
            ("=COUNT(#REF!*2)", "#REF!"),
            ("=COUNTA(1/#REF!)", "#REF!"),
            ("=SUM(1+2)", "3"),
            ("=SUM(ABS(-2)+3*4-2/2)", "13"),
            ("=SUM(1e-3)", "0.001"),
            (r#"=SUM("1+A1",'1+#REF!',2)"#, "2"),
            (r#"=SUM("before+A1+after"-2,2)"#, "2"),
            (r#"=COUNTA("1+A1",'1+#REF!')"#, "2"),
            (r#"=IFERROR("before+A1+after",7)"#, "before+A1+after"),
        ] {
            sheet.set_cell(0, 0, formula.into());
            assert_eq!(sheet.evaluate_cell(0, 0), result, "{formula}");
        }
    }

    #[test]
    fn reference_error_is_visible_after_coercion_and_aggregation() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "=#REF!".into());
        sheet.set_cell(1, 0, "5".into());
        sheet.set_cell(0, 1, "text".into());

        for formula in [
            "=A1",
            "=B1+#REF!",
            "=1+#REF!",
            "=SUM(A1:A2)",
            "=SUM(#REF!)",
            "=COUNT(#REF!)",
            "=COUNTA(A1:A2)",
            "=ROUND(A1,0)",
            "=ROUND(SUM(#REF!),0)",
            "=COUNTIF(#REF!,\"x\")",
            "=COUNTIF(A2,#REF!)",
            "=SUMIF(A2,0,#REF!)",
            "=VLOOKUP(5,#REF!,1)",
        ] {
            assert_eq!(sheet.evaluate_formula(formula, 5, 1), "#REF!", "{formula}");
        }
        assert_eq!(sheet.evaluate_formula("=IF(0,#REF!,7)", 5, 1), "7");
        assert_eq!(sheet.evaluate_formula("=IFERROR(A1,7)", 5, 1), "7");
        assert_eq!(
            sheet.evaluate_formula("=CONCAT(\"#REF!\",\" literal\")", 5, 1),
            "#REF! literal"
        );
        assert_eq!(sheet.evaluate_formula("=COUNTIF(A2:A2,A2)", 5, 1), "1");
    }

    #[test]
    fn inserting_rows_rewrites_moved_formulas_and_expands_ranges() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(1, 0, "2".into());
        sheet.set_cell(2, 0, "3".into());
        sheet.set_cell(3, 0, "4".into());
        sheet.set_cell(3, 1, "=SUM($A$2 : a4)".into());
        sheet.set_cell(0, 2, "=A3".into());
        sheet.selected_rows = Some((0, 1));
        sheet.insert_rows_after_selected();

        assert_eq!(sheet.get_cell(5, 1), "=SUM($A$2 : a6)");
        assert_eq!(sheet.evaluate_cell(5, 1), "9");
        assert_eq!(sheet.get_cell(0, 2), "=A5");
        assert_eq!(sheet.evaluate_cell(0, 2), "3");
    }

    #[test]
    fn deleting_rows_shrinks_ranges_and_marks_fully_removed_references() {
        let mut sheet = Spreadsheet::new();
        for row in 0..5 {
            sheet.set_cell(row, 0, (row + 1).to_string());
        }
        sheet.set_cell(0, 1, "=SUM(A3:A1)".into());
        sheet.set_cell(3, 1, "=SUM(A2:A5)".into());
        sheet.set_cell(0, 2, "=A2+1".into());
        sheet.set_cell(3, 2, "=SUM(A2:A3)".into());
        sheet.selected_rows = Some((1, 2));
        sheet.delete_selected_rows();

        for (row, col, source, value) in [
            (0, 1, "=SUM(A1:A1)", "1"),
            (1, 1, "=SUM(A2:A3)", "9"),
            (0, 2, "=#REF!+1", "#REF!"),
            (1, 2, "=SUM(#REF!)", "#REF!"),
        ] {
            assert_eq!(sheet.get_cell(row, col), source);
            assert_eq!(sheet.evaluate_cell(row, col), value);
        }
    }

    #[test]
    fn column_edits_shift_multi_letter_references_and_maintain_range_direction() {
        let mut sheet = Spreadsheet::new();
        sheet.num_cols = 30;
        sheet.set_cell(0, 25, "3".into());
        sheet.set_cell(0, 26, "7".into());
        sheet.set_cell(0, 27, "=SUM($aa$1 : Z1)".into());
        sheet.set_cell(0, 1, "=AA1".into());
        sheet.set_cell(0, 2, "=Z1".into());
        sheet.set_cell(0, 3, "=SHELL(echo AA1 Z1)".into());
        sheet.selected_cols = Some((24, 25));
        sheet.insert_columns_after_selected();

        assert_eq!(sheet.get_cell(0, 29), "=SUM($ac$1 : Z1)");
        assert_eq!(sheet.evaluate_cell(0, 29), "10");
        assert_eq!(sheet.get_cell(0, 1), "=AC1");
        assert_eq!(sheet.evaluate_cell(0, 1), "7");

        sheet.selected_cols = Some((25, 26));
        sheet.delete_selected_columns();
        assert_eq!(sheet.get_cell(0, 27), "=SUM($aa$1 : Z1)");
        assert_eq!(sheet.evaluate_cell(0, 27), "7");
        assert_eq!(sheet.get_cell(0, 1), "=AA1");
        assert_eq!(sheet.evaluate_cell(0, 1), "7");
        assert_eq!(sheet.get_cell(0, 2), "=#REF!");
        assert_eq!(sheet.evaluate_cell(0, 2), "#REF!");
        assert_eq!(sheet.get_cell(0, 3), "=SHELL(echo AA1 Z1)");
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
        let sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=1+2", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=10-3", 0, 0), "7");
        assert_eq!(sheet.evaluate_formula("=4*5", 0, 0), "20");
        assert_eq!(sheet.evaluate_formula("=20/4", 0, 0), "5");
    }

    #[test]
    fn test_evaluate_min() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "5".to_string());
        sheet.set_cell(2, 0, "30".to_string());

        assert_eq!(sheet.evaluate_formula("=MIN(A1:A3)", 0, 0), "5");
        assert_eq!(sheet.evaluate_formula("=MIN(A3:A1)", 0, 0), "5"); // reversed range
        assert_eq!(sheet.evaluate_formula("=MIN(10,5,30)", 0, 0), "5");
        assert_eq!(sheet.evaluate_formula("=min(A1:A3)", 0, 0), "5"); // case insensitive
    }

    #[test]
    fn test_evaluate_min_simple() {
        // Test exactly what the user is doing: 1, 2, 3 in A1:A3, formula in A4
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(2, 0, "3".to_string());

        // Formula would be entered in A4 (row 3, col 0)
        assert_eq!(sheet.evaluate_formula("=MIN(A1:A3)", 3, 0), "1");
        assert_eq!(sheet.evaluate_formula("=MAX(A1:A3)", 3, 0), "3");
    }

    #[test]
    fn test_full_editing_flow_min() {
        // Simulate the full editing flow as it happens in the UI
        let mut sheet = Spreadsheet::new();
        
        // Step 1: Enter values in A1, A2, A3
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(2, 0, "3".to_string());
        
        // Step 2: Move cursor to A4
        sheet.cursor_row = 3;
        sheet.cursor_col = 0;
        
        // Step 3: Start editing and type =MIN(
        sheet.start_editing();
        for c in "=MIN(".chars() {
            sheet.handle_char_input(c);
        }
        
        // After typing '(' it enters ref selection mode
        assert!(sheet.formula_mode);
        assert!(sheet.selecting_ref);
        
        // Step 4: Move to A1 and extend to A3
        sheet.ref_cursor_row = 0;
        sheet.ref_cursor_col = 0;
        sheet.update_ref_in_buffer();
        
        // Extend to A3
        sheet.ref_anchor = Some((0, 0));
        sheet.ref_cursor_row = 2;
        sheet.update_ref_in_buffer();
        
        // Step 5: Finish editing (this auto-closes the paren)
        sheet.finish_editing();
        
        // Check what was stored
        let stored = sheet.get_cell(3, 0).to_string();
        println!("Stored formula: '{}'", stored);
        
        // Evaluate the formula
        let result = sheet.evaluate_formula(&stored, 3, 0);
        println!("Result: '{}'", result);
        
        assert_eq!(result, "1");
    }

    #[test]
    fn test_min_with_spaces() {
        // Test with spaces around the range
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(2, 0, "3".to_string());

        // These should all work
        assert_eq!(sheet.evaluate_formula("=MIN(A1:A3)", 3, 0), "1");
        assert_eq!(sheet.evaluate_formula("=MIN( A1:A3)", 3, 0), "1");
        assert_eq!(sheet.evaluate_formula("=MIN(A1:A3 )", 3, 0), "1");
        assert_eq!(sheet.evaluate_formula("=MIN( A1:A3 )", 3, 0), "1");
    }

    #[test]
    fn test_min_manual_typing() {
        // Test when user types the formula manually character by character
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(2, 0, "3".to_string());

        // Manually typed formula
        sheet.cursor_row = 3;
        sheet.cursor_col = 0;
        sheet.start_editing();
        
        // Type =MIN(A1:A3) character by character
        for c in "=MIN(A1:A3)".chars() {
            sheet.handle_char_input(c);
        }
        
        // Don't use ref selection, just type it out
        sheet.selecting_ref = false;
        
        println!("Edit buffer: '{}'", sheet.edit_buffer);
        
        sheet.finish_editing();
        
        let stored = sheet.get_cell(3, 0).to_string();
        println!("Stored: '{}'", stored);
        
        let result = sheet.evaluate_formula(&stored, 3, 0);
        println!("Result: '{}'", result);
        
        assert_eq!(result, "1");
    }

    #[test]
    fn test_evaluate_max() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "5".to_string());
        sheet.set_cell(2, 0, "30".to_string());

        assert_eq!(sheet.evaluate_formula("=MAX(A1:A3)", 0, 0), "30");
        assert_eq!(sheet.evaluate_formula("=MAX(A3:A1)", 0, 0), "30"); // reversed range
        assert_eq!(sheet.evaluate_formula("=MAX(10,5,30)", 0, 0), "30");
        assert_eq!(sheet.evaluate_formula("=max(A1:A3)", 0, 0), "30"); // case insensitive
    }

    #[test]
    fn test_evaluate_correl() {
        let mut sheet = Spreadsheet::new();
        // Perfect positive correlation
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(2, 0, "3".to_string());
        sheet.set_cell(0, 1, "2".to_string());
        sheet.set_cell(1, 1, "4".to_string());
        sheet.set_cell(2, 1, "6".to_string());

        let result = sheet.evaluate_formula("=CORREL(A1:A3, B1:B3)", 0, 0);
        let correl: f64 = result.parse().unwrap();
        assert!((correl - 1.0).abs() < 0.0001); // Should be 1.0 for perfect positive correlation

        // Test case insensitive
        let result2 = sheet.evaluate_formula("=correl(A1:A3, B1:B3)", 0, 0);
        let correl2: f64 = result2.parse().unwrap();
        assert!((correl2 - 1.0).abs() < 0.0001);
    }

    #[test]
    fn correl_pairs_cells_before_skipping_blank_and_text_values() {
        let mut sheet = Spreadsheet::new();
        for (row, (left, right)) in [
            ("1", "4"),
            ("2", ""),
            ("", "10"),
            ("4", "1"),
            ("5", "=IF(1,\"missing\",0)"),
            ("missing", "-1"),
        ]
        .into_iter()
        .enumerate()
        {
            sheet.set_cell(row, 0, left.into());
            sheet.set_cell(row, 1, right.into());
        }

        for (formula, expected) in [
            ("=CORREL(A1:A6,B1:B6)", "-1.000000"),
            ("=CORREL(A6:A1,B1:B6)", "-1.000000"),
            // The numeric counts agree even though the range lengths differ.
            ("=CORREL(A1:A4,B1:B5)", "#ERROR"),
        ] {
            assert_eq!(sheet.evaluate_formula(formula, 0, 2), expected, "{formula}");
        }

        sheet.set_cell(3, 0, "1".into());
        assert_eq!(
            sheet.evaluate_formula("=CORREL(A1:A6,B1:B6)", 0, 2),
            "#DIV/0!"
        );
        sheet.set_cell(5, 1, "=#REF!".into());
        assert_eq!(
            sheet.evaluate_formula("=CORREL(A1:A4,B1:B6)", 0, 2),
            "#REF!"
        );
    }

    #[test]
    fn test_evaluate_count() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "text".to_string());
        sheet.set_cell(3, 0, "30".to_string());

        assert_eq!(sheet.evaluate_formula("=COUNT(A1:A4)", 0, 0), "3"); // Only numeric values
        assert_eq!(sheet.evaluate_formula("=COUNT(10,20,30)", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=count(A1:A4)", 0, 0), "3"); // case insensitive
    }

    #[test]
    fn test_evaluate_counta() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "text".to_string());
        sheet.set_cell(3, 0, "".to_string()); // Empty cell

        assert_eq!(sheet.evaluate_formula("=COUNTA(A1:A4)", 0, 0), "3"); // Non-empty cells
        assert_eq!(sheet.evaluate_formula("=COUNTA(A1:A3)", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=counta(A1:A3)", 0, 0), "3"); // case insensitive
    }

    #[test]
    fn test_evaluate_countif() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "10".to_string());
        sheet.set_cell(3, 0, "30".to_string());

        assert_eq!(sheet.evaluate_formula("=COUNTIF(A1:A4,\"10\")", 0, 0), "2");
        assert_eq!(sheet.evaluate_formula("=COUNTIF(A1:A4,\">20\")", 0, 0), "1");
        assert_eq!(sheet.evaluate_formula("=COUNTIF(A1:A4,\"<20\")", 0, 0), "2");
        assert_eq!(sheet.evaluate_formula("=countif(A1:A4,\"10\")", 0, 0), "2"); // case insensitive
    }

    #[test]
    fn test_evaluate_sumif() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "A".to_string());
        sheet.set_cell(1, 0, "B".to_string());
        sheet.set_cell(2, 0, "A".to_string());
        sheet.set_cell(0, 1, "10".to_string());
        sheet.set_cell(1, 1, "20".to_string());
        sheet.set_cell(2, 1, "30".to_string());

        // Test with numeric criteria first (simpler case)
        sheet.set_cell(0, 2, "10".to_string());
        sheet.set_cell(1, 2, "20".to_string());
        sheet.set_cell(2, 2, "10".to_string());
        
        // Test the simpler case without sum_range (criteria range = sum range)
        assert_eq!(sheet.evaluate_formula("=SUMIF(B1:B3,\">15\")", 0, 0), "50"); // Sum range = criteria range: 20 + 30 = 50
        
        // Now test with separate sum range
        assert_eq!(sheet.evaluate_formula("=SUMIF(C1:C3,\"10\",B1:B3)", 0, 0), "40"); // Sum where C1:C3 = "10" -> B1 + B3 = 10 + 30 = 40
        
        // Now test with text criteria
        assert_eq!(sheet.evaluate_formula("=SUMIF(A1:A3,\"A\",B1:B3)", 0, 0), "40"); // Sum where A1:A3 = "A"
        assert_eq!(sheet.evaluate_formula("=sumif(A1:A3,\"A\",B1:B3)", 0, 0), "40"); // case insensitive
    }

    #[test]
    fn test_evaluate_averageif() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "10".to_string());
        sheet.set_cell(3, 0, "30".to_string());

        assert_eq!(sheet.evaluate_formula("=AVERAGEIF(A1:A4,\">15\")", 0, 0), "25"); // Average of 20 and 30
        assert_eq!(sheet.evaluate_formula("=AVERAGEIF(A1:A4,\"10\")", 0, 0), "10");
        assert_eq!(sheet.evaluate_formula("=averageif(A1:A4,\">15\")", 0, 0), "25"); // case insensitive
    }

    #[test]
    fn test_evaluate_round() {
        let sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=ROUND(3.14159,2)", 0, 0), "3.14");
        assert_eq!(sheet.evaluate_formula("=ROUND(3.14159,0)", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=ROUND(3.5,0)", 0, 0), "4");
        assert_eq!(sheet.evaluate_formula("=round(3.14159,2)", 0, 0), "3.14"); // case insensitive
    }

    #[test]
    fn test_evaluate_concatenate() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=CONCATENATE(\"Hello\",\" \",\"World\")", 0, 0), "Hello World");
        assert_eq!(sheet.evaluate_formula("=CONCAT(\"A\",\"B\",\"C\")", 0, 0), "ABC");
        assert_eq!(sheet.evaluate_formula("=concatenate(\"Hello\",\"World\")", 0, 0), "HelloWorld"); // case insensitive
        
        sheet.set_cell(0, 0, "Hello".to_string());
        sheet.set_cell(0, 1, "World".to_string());
        assert_eq!(sheet.evaluate_formula("=CONCATENATE(A1,\" \",B1)", 0, 0), "Hello World");
    }

    #[test]
    fn direct_references_use_the_evaluated_value() {
        let mut sheet = Spreadsheet::new();
        for (row, (source, value)) in [
            ("Low", "Low"),
            (r#"=IF(1,"a,b",0)"#, "a,b"),
            ("001", "001"),
            ("", ""),
        ]
        .into_iter()
        .enumerate()
        {
            sheet.set_cell(row, 0, source.into());
            sheet.set_cell(row, 1, format!("=$a${}", row + 1));
            assert_eq!(sheet.evaluate_cell(row, 1), value);
        }
        assert_eq!(sheet.evaluate_formula("=B3+1", 0, 2), "2");
    }

    #[test]
    fn quoted_arguments_preserve_delimiters_and_literal_quotes() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "\"a,b\"".into());
        for (formula, expected) in [
            (r#"=CONCATENATE("a,b","c")"#, "a,bc"),
            (r#"=CONCAT('a),b',UPPER("c(,d"))"#, "a),bC(,D"),
            (r#"=UPPER('"a,b"')"#, "\"A,B\""),
            (r#"=LEFT('"A1,B2"',1)"#, "\""),
            (r#"=LEN('"a,b"')"#, "5"),
            ("=LEN(A1)", "5"),
        ] {
            assert_eq!(sheet.evaluate_formula(formula, 0, 1), expected, "{formula}");
        }
    }

    #[test]
    fn test_evaluate_left() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=LEFT(\"Hello World\",5)", 0, 0), "Hello");
        assert_eq!(sheet.evaluate_formula("=LEFT(\"Hello\",10)", 0, 0), "Hello"); // More chars than available
        assert_eq!(sheet.evaluate_formula("=left(\"Hello\",3)", 0, 0), "Hel"); // case insensitive
        
        sheet.set_cell(0, 0, "Hello World".to_string());
        assert_eq!(sheet.evaluate_formula("=LEFT(A1,5)", 0, 0), "Hello");
    }

    #[test]
    fn test_evaluate_right() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=RIGHT(\"Hello World\",5)", 0, 0), "World");
        assert_eq!(sheet.evaluate_formula("=RIGHT(\"Hello\",10)", 0, 0), "Hello"); // More chars than available
        assert_eq!(sheet.evaluate_formula("=right(\"Hello\",3)", 0, 0), "llo"); // case insensitive
        
        sheet.set_cell(0, 0, "Hello World".to_string());
        assert_eq!(sheet.evaluate_formula("=RIGHT(A1,5)", 0, 0), "World");
    }

    #[test]
    fn test_evaluate_mid() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=MID(\"Hello World\",7,5)", 0, 0), "World");
        assert_eq!(sheet.evaluate_formula("=MID(\"Hello World\",1,5)", 0, 0), "Hello");
        assert_eq!(sheet.evaluate_formula("=MID(\"Hello\",3,10)", 0, 0), "llo"); // More chars than available
        assert_eq!(sheet.evaluate_formula("=mid(\"Hello\",2,3)", 0, 0), "ell"); // case insensitive
        
        sheet.set_cell(0, 0, "Hello World".to_string());
        assert_eq!(sheet.evaluate_formula("=MID(A1,7,5)", 0, 0), "World");
    }

    #[test]
    fn test_evaluate_len() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=LEN(\"Hello\")", 0, 0), "5");
        assert_eq!(sheet.evaluate_formula("=LEN(\"\")", 0, 0), "0");
        assert_eq!(sheet.evaluate_formula("=LEN(\"Hello World\")", 0, 0), "11");
        assert_eq!(sheet.evaluate_formula("=len(\"Hello\")", 0, 0), "5"); // case insensitive
        
        sheet.set_cell(0, 0, "Hello World".to_string());
        assert_eq!(sheet.evaluate_formula("=LEN(A1)", 0, 0), "11");
    }

    #[test]
    fn test_evaluate_vlookup() {
        let mut sheet = Spreadsheet::new();
        // Set up a lookup table: A1:B3
        // A1=1, B1=100
        // A2=2, B2=200
        // A3=3, B3=300
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(0, 1, "100".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(1, 1, "200".to_string());
        sheet.set_cell(2, 0, "3".to_string());
        sheet.set_cell(2, 1, "300".to_string());

        assert_eq!(sheet.evaluate_formula("=VLOOKUP(2,A1:B3,2)", 0, 0), "200");
        assert_eq!(sheet.evaluate_formula("=VLOOKUP(1,A1:B3,2)", 0, 0), "100");
        assert_eq!(sheet.evaluate_formula("=VLOOKUP(3,A1:B3,2)", 0, 0), "300");
        assert_eq!(sheet.evaluate_formula("=vlookup(2,A1:B3,2)", 0, 0), "200"); // case insensitive
        
        // Test with text lookup
        sheet.set_cell(0, 0, "Apple".to_string());
        sheet.set_cell(0, 1, "Red".to_string());
        sheet.set_cell(1, 0, "Banana".to_string());
        sheet.set_cell(1, 1, "Yellow".to_string());
        assert_eq!(sheet.evaluate_formula("=VLOOKUP(\"Banana\",A1:B2,2)", 0, 0), "Yellow");
    }

    #[test]
    fn vlookup_uses_exact_and_approximate_modes() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".into());
        sheet.set_cell(0, 1, "Low".into());
        sheet.set_cell(1, 0, "20".into());
        sheet.set_cell(1, 1, "High".into());
        sheet.set_cell(0, 2, "=IF(1,FALSE,TRUE)".into());

        for (mode, middle, above) in [
            (",FALSE", "#N/A", "#N/A"),
            (",0", "#N/A", "#N/A"),
            (",C1", "#N/A", "#N/A"),
            (",tRuE", "Low", "High"),
            (",1", "Low", "High"),
            (",-0.1", "Low", "High"),
            ("", "Low", "High"),
        ] {
            for (key, expected) in [(9, "#N/A"), (10, "Low"), (15, middle), (21, above)] {
                let formula = format!("=VLOOKUP({key},A1:B2,2{mode})");
                assert_eq!(
                    sheet.evaluate_formula(&formula, 0, 3),
                    expected,
                    "{formula}"
                );
            }
        }

        for mode in ["", "MAYBE", "NaN", "\"TRUE\""] {
            let formula = format!("=VLOOKUP(10,A1:B2,2,{mode})");
            assert_eq!(
                sheet.evaluate_formula(&formula, 0, 3),
                "#ERROR",
                "{formula}"
            );
        }

        sheet.set_cell(0, 0, "0".into());
        sheet.set_cell(1, 0, "0.0000000000000001".into());
        assert_eq!(
            sheet.evaluate_formula("=VLOOKUP(0.0000000000000001,A1:B2,2,FALSE)", 0, 3),
            "High"
        );
    }

    #[test]
    fn vlookup_compares_evaluated_numeric_and_text_keys() {
        let mut sheet = Spreadsheet::new();
        sheet.set_cell(0, 0, "10".into());
        sheet.set_cell(0, 1, "Low".into());
        sheet.set_cell(1, 0, "=10+10".into());
        sheet.set_cell(1, 1, "High".into());
        for (formula, expected) in [
            ("=VLOOKUP(20,A1:B2,2,FALSE)", "High"),
            ("=VLOOKUP(21,A1:B2,2,TRUE)", "High"),
            ("=VLOOKUP(15,A1:B2,2,TRUE)", "Low"),
        ] {
            assert_eq!(sheet.evaluate_formula(formula, 0, 3), expected, "{formula}");
        }

        sheet.set_cell(1, 0, r#"=CONCAT("High",", noon")"#.into());
        assert_eq!(
            sheet.evaluate_formula(r#"=VLOOKUP("High, noon",A1:B2,2,FALSE)"#, 0, 3),
            "High"
        );

        for (key, expected) in [("=D1", "#CYCLE!"), ("=#REF!", "#REF!")] {
            sheet.set_cell(1, 0, key.into());
            for mode in ["FALSE", "TRUE"] {
                sheet.set_cell(0, 3, format!("=VLOOKUP(21,A1:B2,2,{mode})"));
                assert_eq!(sheet.evaluate_cell(0, 3), expected, "{mode}");
            }
        }
    }

    #[test]
    fn test_evaluate_and() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=AND(TRUE,TRUE)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=AND(TRUE,FALSE)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=AND(1,1)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=AND(1,0)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=AND(5>3,10>5)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=AND(5>3,2>5)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=and(TRUE,TRUE)", 0, 0), "TRUE"); // case insensitive
        
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(0, 1, "1".to_string());
        assert_eq!(sheet.evaluate_formula("=AND(A1,B1)", 0, 0), "TRUE");
    }

    #[test]
    fn test_evaluate_or() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=OR(TRUE,FALSE)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=OR(FALSE,FALSE)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=OR(1,0)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=OR(0,0)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=OR(5>3,2>5)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=OR(2>5,3>5)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=or(TRUE,FALSE)", 0, 0), "TRUE"); // case insensitive
        
        sheet.set_cell(0, 0, "0".to_string());
        sheet.set_cell(0, 1, "1".to_string());
        assert_eq!(sheet.evaluate_formula("=OR(A1,B1)", 0, 0), "TRUE");
    }

    #[test]
    fn test_evaluate_abs() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=ABS(5)", 0, 0), "5");
        assert_eq!(sheet.evaluate_formula("=ABS(-5)", 0, 0), "5");
        assert_eq!(sheet.evaluate_formula("=ABS(0)", 0, 0), "0");
        assert_eq!(sheet.evaluate_formula("=ABS(-3.14)", 0, 0), "3.14");
        assert_eq!(sheet.evaluate_formula("=abs(-10)", 0, 0), "10"); // case insensitive
        
        sheet.set_cell(0, 0, "-15".to_string());
        assert_eq!(sheet.evaluate_formula("=ABS(A1)", 0, 0), "15");
    }

    #[test]
    fn test_evaluate_trim() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=TRIM(\"  hello  \")", 0, 0), "hello");
        assert_eq!(sheet.evaluate_formula("=TRIM(\"  hello  world  \")", 0, 0), "hello world");
        assert_eq!(sheet.evaluate_formula("=TRIM(\"hello\")", 0, 0), "hello");
        assert_eq!(sheet.evaluate_formula("=trim(\"  test  \")", 0, 0), "test"); // case insensitive
        
        sheet.set_cell(0, 0, "  spaced  text  ".to_string());
        assert_eq!(sheet.evaluate_formula("=TRIM(A1)", 0, 0), "spaced text");
    }

    #[test]
    fn test_evaluate_upper() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=UPPER(\"hello\")", 0, 0), "HELLO");
        assert_eq!(sheet.evaluate_formula("=UPPER(\"Hello World\")", 0, 0), "HELLO WORLD");
        assert_eq!(sheet.evaluate_formula("=UPPER(\"HELLO\")", 0, 0), "HELLO");
        assert_eq!(sheet.evaluate_formula("=upper(\"test\")", 0, 0), "TEST"); // case insensitive
        
        sheet.set_cell(0, 0, "lowercase text".to_string());
        assert_eq!(sheet.evaluate_formula("=UPPER(A1)", 0, 0), "LOWERCASE TEXT");
    }

    #[test]
    fn test_evaluate_not() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=NOT(TRUE)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=NOT(FALSE)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=NOT(1)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=NOT(0)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=NOT(5>3)", 0, 0), "FALSE");
        assert_eq!(sheet.evaluate_formula("=NOT(2>5)", 0, 0), "TRUE");
        assert_eq!(sheet.evaluate_formula("=not(TRUE)", 0, 0), "FALSE"); // case insensitive
        
        sheet.set_cell(0, 0, "1".to_string());
        assert_eq!(sheet.evaluate_formula("=NOT(A1)", 0, 0), "FALSE");
        sheet.set_cell(0, 0, "0".to_string());
        assert_eq!(sheet.evaluate_formula("=NOT(A1)", 0, 0), "TRUE");
    }

    #[test]
    fn test_evaluate_lower() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=LOWER(\"HELLO\")", 0, 0), "hello");
        assert_eq!(sheet.evaluate_formula("=LOWER(\"Hello World\")", 0, 0), "hello world");
        assert_eq!(sheet.evaluate_formula("=LOWER(\"hello\")", 0, 0), "hello");
        assert_eq!(sheet.evaluate_formula("=lower(\"TEST\")", 0, 0), "test"); // case insensitive
        
        sheet.set_cell(0, 0, "UPPERCASE TEXT".to_string());
        assert_eq!(sheet.evaluate_formula("=LOWER(A1)", 0, 0), "uppercase text");
    }

    #[test]
    fn test_evaluate_mod() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=MOD(10,3)", 0, 0), "1");
        assert_eq!(sheet.evaluate_formula("=MOD(10,5)", 0, 0), "0");
        assert_eq!(sheet.evaluate_formula("=MOD(15,4)", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=MOD(-10,3)", 0, 0), "-1");
        assert_eq!(sheet.evaluate_formula("=MOD(10.5,3)", 0, 0), "1.5");
        assert_eq!(sheet.evaluate_formula("=mod(10,3)", 0, 0), "1"); // case insensitive
        
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(0, 1, "3".to_string());
        assert_eq!(sheet.evaluate_formula("=MOD(A1,B1)", 0, 0), "1");
        
        // Test division by zero
        assert_eq!(sheet.evaluate_formula("=MOD(10,0)", 0, 0), "#DIV/0!");
    }

    #[test]
    fn test_evaluate_sqrt() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=SQRT(4)", 0, 0), "2");
        assert_eq!(sheet.evaluate_formula("=SQRT(9)", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=SQRT(16)", 0, 0), "4");
        assert_eq!(sheet.evaluate_formula("=SQRT(0)", 0, 0), "0");
        assert_eq!(sheet.evaluate_formula("=SQRT(2)", 0, 0), "1.4142135623730951");
        assert_eq!(sheet.evaluate_formula("=sqrt(25)", 0, 0), "5"); // case insensitive
        
        sheet.set_cell(0, 0, "16".to_string());
        assert_eq!(sheet.evaluate_formula("=SQRT(A1)", 0, 0), "4");
        
        // Test negative number
        assert_eq!(sheet.evaluate_formula("=SQRT(-1)", 0, 0), "#NUM!");
    }

    #[test]
    fn test_evaluate_power() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=POWER(2,3)", 0, 0), "8");
        assert_eq!(sheet.evaluate_formula("=POWER(5,2)", 0, 0), "25");
        assert_eq!(sheet.evaluate_formula("=POWER(10,0)", 0, 0), "1");
        assert_eq!(sheet.evaluate_formula("=POWER(2,0.5)", 0, 0), "1.4142135623730951");
        assert_eq!(sheet.evaluate_formula("=POWER(3,4)", 0, 0), "81");
        assert_eq!(sheet.evaluate_formula("=power(2,3)", 0, 0), "8"); // case insensitive
        
        sheet.set_cell(0, 0, "2".to_string());
        sheet.set_cell(0, 1, "4".to_string());
        assert_eq!(sheet.evaluate_formula("=POWER(A1,B1)", 0, 0), "16");
        
        // Test with negative exponent
        assert_eq!(sheet.evaluate_formula("=POWER(2,-2)", 0, 0), "0.25");
        
        // Test with zero base and negative exponent (should produce #NUM!)
        let result = sheet.evaluate_formula("=POWER(0,-1)", 0, 0);
        assert!(result == "#NUM!" || result == "inf"); // Depending on implementation
    }

    #[test]
    fn test_evaluate_iferror() {
        let mut sheet = Spreadsheet::new();
        // Test with no error
        assert_eq!(sheet.evaluate_formula("=IFERROR(5+3,\"error\")", 0, 0), "8");
        assert_eq!(sheet.evaluate_formula("=IFERROR(10/2,\"error\")", 0, 0), "5");
        
        // Test with division by zero error
        assert_eq!(sheet.evaluate_formula("=IFERROR(10/0,\"error\")", 0, 0), "error");
        assert_eq!(sheet.evaluate_formula("=IFERROR(10/0,\"Division by zero\")", 0, 0), "Division by zero");
        
        // Test with #ERROR
        assert_eq!(sheet.evaluate_formula("=IFERROR(#ERROR,\"fallback\")", 0, 0), "fallback");
        
        // Test with cell reference that has error
        sheet.set_cell(0, 0, "=10/0".to_string());
        assert_eq!(sheet.evaluate_formula("=IFERROR(A1,\"error\")", 0, 1), "error");
        
        // Test case insensitive
        assert_eq!(sheet.evaluate_formula("=iferror(5+3,\"error\")", 0, 0), "8");
        
        // Test with nested formula that errors
        assert_eq!(sheet.evaluate_formula("=IFERROR(SQRT(-1),\"invalid\")", 0, 0), "invalid");
    }

    #[test]
    fn test_evaluate_int() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=INT(3.7)", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=INT(3.2)", 0, 0), "3");
        assert_eq!(sheet.evaluate_formula("=INT(-3.7)", 0, 0), "-4");
        assert_eq!(sheet.evaluate_formula("=INT(-3.2)", 0, 0), "-4");
        assert_eq!(sheet.evaluate_formula("=INT(5)", 0, 0), "5");
        assert_eq!(sheet.evaluate_formula("=INT(0)", 0, 0), "0");
        assert_eq!(sheet.evaluate_formula("=int(3.7)", 0, 0), "3"); // case insensitive
        
        sheet.set_cell(0, 0, "7.9".to_string());
        assert_eq!(sheet.evaluate_formula("=INT(A1)", 0, 0), "7");
        
        sheet.set_cell(0, 0, "-7.9".to_string());
        assert_eq!(sheet.evaluate_formula("=INT(A1)", 0, 0), "-8");
    }

    #[test]
    fn test_evaluate_proper() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=PROPER(\"hello world\")", 0, 0), "Hello World");
        assert_eq!(sheet.evaluate_formula("=PROPER(\"HELLO WORLD\")", 0, 0), "Hello World");
        assert_eq!(sheet.evaluate_formula("=PROPER(\"hELLo WoRLd\")", 0, 0), "Hello World");
        assert_eq!(sheet.evaluate_formula("=PROPER(\"hello\")", 0, 0), "Hello");
        assert_eq!(sheet.evaluate_formula("=PROPER(\"a b c\")", 0, 0), "A B C");
        assert_eq!(sheet.evaluate_formula("=proper(\"test\")", 0, 0), "Test"); // case insensitive
        
        sheet.set_cell(0, 0, "john smith".to_string());
        assert_eq!(sheet.evaluate_formula("=PROPER(A1)", 0, 0), "John Smith");
        
        // Test with multiple spaces
        assert_eq!(sheet.evaluate_formula("=PROPER(\"hello  world\")", 0, 0), "Hello  World");
    }

    #[test]
    fn test_evaluate_product() {
        let mut sheet = Spreadsheet::new();
        assert_eq!(sheet.evaluate_formula("=PRODUCT(2,3,4)", 0, 0), "24");
        assert_eq!(sheet.evaluate_formula("=PRODUCT(5,2)", 0, 0), "10");
        assert_eq!(sheet.evaluate_formula("=PRODUCT(10)", 0, 0), "10");
        assert_eq!(sheet.evaluate_formula("=PRODUCT(2.5,4)", 0, 0), "10");
        assert_eq!(sheet.evaluate_formula("=PRODUCT(-2,3)", 0, 0), "-6");
        assert_eq!(sheet.evaluate_formula("=product(2,3)", 0, 0), "6"); // case insensitive
        
        sheet.set_cell(0, 0, "2".to_string());
        sheet.set_cell(0, 1, "3".to_string());
        sheet.set_cell(0, 2, "4".to_string());
        assert_eq!(sheet.evaluate_formula("=PRODUCT(A1:C1)", 0, 0), "24");
        assert_eq!(sheet.evaluate_formula("=PRODUCT(A1,B1,C1)", 0, 0), "24");
        
        // Test with range
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(2, 0, "3".to_string());
        assert_eq!(sheet.evaluate_formula("=PRODUCT(A1:A3)", 0, 0), "6");
        
        // Test with zero
        assert_eq!(sheet.evaluate_formula("=PRODUCT(5,0,10)", 0, 0), "0");
    }

    #[test]
    fn test_evaluate_median() {
        let mut sheet = Spreadsheet::new();
        // Odd number of values
        assert_eq!(sheet.evaluate_formula("=MEDIAN(1,2,3)", 0, 0), "2");
        assert_eq!(sheet.evaluate_formula("=MEDIAN(1,3,5,7,9)", 0, 0), "5");
        assert_eq!(sheet.evaluate_formula("=MEDIAN(10,20,30)", 0, 0), "20");
        
        // Even number of values (average of two middle values)
        assert_eq!(sheet.evaluate_formula("=MEDIAN(1,2,3,4)", 0, 0), "2.5");
        assert_eq!(sheet.evaluate_formula("=MEDIAN(10,20,30,40)", 0, 0), "25");
        
        // Single value
        assert_eq!(sheet.evaluate_formula("=MEDIAN(5)", 0, 0), "5");
        
        // Two values
        assert_eq!(sheet.evaluate_formula("=MEDIAN(10,20)", 0, 0), "15");
        
        // Unsorted values (should still work)
        assert_eq!(sheet.evaluate_formula("=MEDIAN(3,1,2)", 0, 0), "2");
        assert_eq!(sheet.evaluate_formula("=MEDIAN(5,1,3,2,4)", 0, 0), "3");
        
        assert_eq!(sheet.evaluate_formula("=median(1,2,3)", 0, 0), "2"); // case insensitive
        
        // Test with cell references
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "30".to_string());
        assert_eq!(sheet.evaluate_formula("=MEDIAN(A1:A3)", 0, 0), "20");
        
        // Test with range (even number)
        sheet.set_cell(0, 0, "1".to_string());
        sheet.set_cell(1, 0, "2".to_string());
        sheet.set_cell(2, 0, "3".to_string());
        sheet.set_cell(3, 0, "4".to_string());
        assert_eq!(sheet.evaluate_formula("=MEDIAN(A1:A4)", 0, 0), "2.5");
        
        // Test with mixed arguments (A1:A3 = [1,2,3], plus 40 = [1,2,3,40], median = 2.5)
        assert_eq!(sheet.evaluate_formula("=MEDIAN(A1:A3,40)", 0, 0), "2.5");
        
        // Test with different values to get median of 25
        sheet.set_cell(0, 0, "10".to_string());
        sheet.set_cell(1, 0, "20".to_string());
        sheet.set_cell(2, 0, "30".to_string());
        assert_eq!(sheet.evaluate_formula("=MEDIAN(A1:A3,40)", 0, 0), "25");
    }
}
