use crate::{shell, spreadsheet::Spreadsheet};

pub const ERROR: &str = "#REF!";

#[derive(Clone, Copy)]
pub enum Axis {
    Row,
    Column,
}

#[derive(Clone, Copy)]
pub enum Edit {
    Insert { at: usize, count: usize },
    Delete { first: usize, last: usize },
}

pub struct CellReference {
    pub row: usize,
    pub col: usize,
    letters_start: usize,
    letters_end: usize,
    digits_start: usize,
}

impl CellReference {
    pub fn parse(text: &str) -> Option<Self> {
        let letters_start = usize::from(text.starts_with('$'));
        let letters = text[letters_start..]
            .bytes()
            .take_while(u8::is_ascii_alphabetic)
            .count();
        if letters == 0 {
            return None;
        }
        let letters_end = letters_start + letters;
        let digits_start = letters_end + usize::from(text[letters_end..].starts_with('$'));
        let digits = &text[digits_start..];
        if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let row = digits.parse::<usize>().ok()?.checked_sub(1)?;
        let col = text[letters_start..letters_end]
            .bytes()
            .try_fold(0usize, |col, letter| {
                let digit = usize::from(letter.to_ascii_uppercase() - b'A' + 1);
                col.checked_mul(26)?.checked_add(digit)
            })?
            .checked_sub(1)?;
        Some(Self {
            row,
            col,
            letters_start,
            letters_end,
            digits_start,
        })
    }

    fn index(&self, axis: Axis) -> usize {
        match axis {
            Axis::Row => self.row,
            Axis::Column => self.col,
        }
    }

    fn write(&self, output: &mut String, original: &str, axis: Axis, index: usize) {
        if index == self.index(axis) {
            output.push_str(original);
            return;
        }
        match axis {
            Axis::Row => {
                output.push_str(&original[..self.digits_start]);
                output.push_str(&(index + 1).to_string());
            }
            Axis::Column => {
                output.push_str(&original[..self.letters_start]);
                let mut col = Spreadsheet::col_name(index);
                if original.as_bytes()[self.letters_start].is_ascii_lowercase() {
                    col.make_ascii_lowercase();
                }
                output.push_str(&col);
                output.push_str(&original[self.letters_end..]);
            }
        }
    }
}

impl Edit {
    fn index(self, index: usize) -> Option<usize> {
        match self {
            Self::Insert { at, count } if index >= at => {
                index.checked_add(count).filter(|&i| i < usize::MAX)
            }
            Self::Delete { first, last } if (first..=last).contains(&index) => None,
            Self::Delete { first, last } if index > last => Some(index - (last - first + 1)),
            _ => Some(index),
        }
    }

    fn range(self, start: usize, end: usize) -> Option<(usize, usize)> {
        let (mut low, mut high) = (start.min(end), start.max(end));
        if let Self::Delete { first, last } = self {
            if (first..=last).contains(&low) {
                low = last.checked_add(1)?;
            }
            if (first..=last).contains(&high) {
                high = first.checked_sub(1)?;
            }
            if low > high {
                return None;
            }
        }
        let (low, high) = (self.index(low)?, self.index(high)?);
        if start <= end {
            Some((low, high))
        } else {
            Some((high, low))
        }
    }
}

pub fn rewrite(formula: &str, axis: Axis, edit: Edit) -> Option<String> {
    let expression = formula.strip_prefix('=')?;
    if shell::shell_args(expression.trim()).is_some() {
        return None;
    }

    let mut output = String::new();
    let mut copied = 0;
    let mut cursor = 1;
    while cursor < formula.len() {
        let start = cursor;
        let rest = &formula[start..];
        let ch = rest.chars().next()?;
        if matches!(ch, '"' | '\'') {
            cursor = quoted_end(formula, start);
            continue;
        }
        if !is_word_char(ch) {
            cursor += ch.len_utf8();
            continue;
        }

        let word = word_prefix(rest);
        cursor += word.len();
        let suffix = formula[cursor..].trim_start();
        if suffix.starts_with('(') {
            if word.eq_ignore_ascii_case("SHELL") {
                cursor = parentheses_end(formula, formula.len() - suffix.len());
            }
            continue;
        }
        let Some(cell) = CellReference::parse(word) else {
            continue;
        };

        let index = cell.index(axis);
        if let Some(after_colon) = suffix.strip_prefix(':') {
            let after_colon = after_colon.trim_start();
            let end_word = word_prefix(after_colon);
            if let Some(end_cell) = CellReference::parse(end_word) {
                let separator = &formula[cursor..formula.len() - after_colon.len()];
                cursor = formula.len() - after_colon.len() + end_word.len();
                let end_index = end_cell.index(axis);
                let mapped = edit.range(index, end_index);
                if mapped != Some((index, end_index)) {
                    output.push_str(&formula[copied..start]);
                    match mapped {
                        Some((new_start, new_end)) => {
                            cell.write(&mut output, word, axis, new_start);
                            output.push_str(separator);
                            end_cell.write(&mut output, end_word, axis, new_end);
                        }
                        None => output.push_str(ERROR),
                    }
                    copied = cursor;
                }
                continue;
            }
        }

        let mapped = edit.index(index);
        if mapped != Some(index) {
            output.push_str(&formula[copied..start]);
            match mapped {
                Some(new_index) => cell.write(&mut output, word, axis, new_index),
                None => output.push_str(ERROR),
            }
            copied = cursor;
        }
    }

    if copied == 0 {
        None
    } else {
        output.push_str(&formula[copied..]);
        Some(output)
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '$' | '.' | '#' | '!')
}

fn word_prefix(text: &str) -> &str {
    &text[..text.find(|ch| !is_word_char(ch)).unwrap_or(text.len())]
}

pub fn top_level_delimiters(text: &str, delimiter: u8) -> impl Iterator<Item = usize> + '_ {
    let bytes = text.as_bytes();
    let mut cursor = 0;
    std::iter::from_fn(move || {
        let mut depth = 0usize;
        while let Some(&byte) = bytes.get(cursor) {
            let index = cursor;
            cursor += 1;
            match byte {
                b'"' | b'\'' => cursor = quoted_end(text, index),
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ if byte == delimiter && depth == 0 => return Some(index),
                _ => {}
            }
        }
        None
    })
}

fn quoted_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == quote {
            if bytes.get(i + 1) != Some(&quote) {
                return i + 1;
            }
            i += 2;
        } else if bytes[i] == b'\\' && matches!(bytes.get(i + 1), Some(b'\'' | b'"' | b'\\')) {
            i += 2;
        } else {
            i += 1;
        }
    }
    bytes.len()
}

fn parentheses_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 1;
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                i = quoted_end(text, i);
                continue;
            }
            b'\\' if i + 1 < bytes.len() => i += 1,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewriting_preserves_quoted_text_and_opaque_shell_arguments() {
        let edit = Edit::Insert { at: 0, count: 1 };
        let expression = r#"=IF(A1,CONCAT("A1 ""표 B2""", 'B2'),SHELL(echo A1 $(echo C3)))"#;
        let expected = r#"=IF(A2,CONCAT("A1 ""표 B2""", 'B2'),SHELL(echo A1 $(echo C3)))"#;
        assert_eq!(
            rewrite(expression, Axis::Row, edit).as_deref(),
            Some(expected)
        );
        assert_eq!(rewrite("=sHeLl(echo A1 B2)", Axis::Row, edit), None);
        assert_eq!(
            rewrite("=VLOOKUP(1E3,A1:B2,2)", Axis::Row, edit).as_deref(),
            Some("=VLOOKUP(1E3,A2:B3,2)")
        );
        assert_eq!(rewrite("write A1:B2", Axis::Row, edit), None);
    }

    #[test]
    fn deletion_shrinks_ranges_at_both_edges_and_preserves_their_orientation() {
        let edit = Edit::Delete { first: 1, last: 2 };
        for (before, after) in [
            ("=SUM(A1:A2)", "=SUM(A1:A1)"),
            ("=SUM(A3:A5)", "=SUM(A2:A3)"),
            ("=SUM(A5:A3)", "=SUM(A3:A2)"),
            ("=SUM(A1:A5)", "=SUM(A1:A3)"),
            ("=SUM(A2:C3)", "=SUM(#REF!)"),
        ] {
            assert_eq!(rewrite(before, Axis::Row, edit).as_deref(), Some(after));
        }
    }
}
