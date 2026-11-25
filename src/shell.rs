use std::process::Command;

use crate::spreadsheet::Spreadsheet;

pub struct ShellPrompt {
    pub step: ShellStep,
    pub scroll: usize,
}

pub enum ShellStep {
    Run {
        row: usize,
        col: usize,
        command: String,
    },
    Apply(ShellOutput),
    Message(String),
}

pub struct ShellOutput {
    row: usize,
    col: usize,
    rows: Vec<Vec<String>>,
    width: usize,
}

pub fn shell_args(expr: &str) -> Option<&str> {
    if expr.get(..6)?.eq_ignore_ascii_case("SHELL(") {
        expr[6..].strip_suffix(')')
    } else {
        None
    }
}

pub fn shell_command(args: &str) -> Option<&str> {
    let args = args.trim();
    let command = args
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| args.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(args);
    (!command.trim().is_empty()).then_some(command)
}

pub fn run_command(command: &str) -> Result<String, String> {
    let output = if cfg!(windows) {
        Command::new("cmd").args(["/C", command]).output()
    } else {
        Command::new("sh").args(["-c", command]).output()
    }
    .map_err(|error| format!("Could not run command ({error})."))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = format!("Command failed ({}).", output.status);
    if stderr.trim().is_empty() {
        Err(message)
    } else {
        Err(format!("{message}\n{}", stderr.trim()))
    }
}

impl ShellOutput {
    fn from_stdout(row: usize, col: usize, stdout: &str) -> Option<Self> {
        let text = stdout.trim();
        if text.is_empty() {
            return None;
        }

        let lines: Vec<_> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        let multi_column = lines
            .iter()
            .filter(|line| line.split_whitespace().nth(1).is_some())
            .count();
        let rows = if multi_column >= lines.len().div_ceil(2) {
            lines
                .iter()
                .map(|line| line.split_whitespace().map(str::to_string).collect())
                .collect()
        } else {
            vec![vec![text.to_string()]]
        };
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        Some(Self {
            row,
            col,
            rows,
            width,
        })
    }

    pub fn range(&self) -> String {
        let start = format!("{}{}", Spreadsheet::col_name(self.col), self.row + 1);
        if self.rows.len() == 1 && self.width == 1 {
            start
        } else {
            let end_col = Spreadsheet::col_name(self.col + self.width - 1);
            let end_row = self.row + self.rows.len();
            format!("{start}:{end_col}{end_row}")
        }
    }

    pub fn counts(&self, sheet: &Spreadsheet) -> (usize, usize) {
        let mut writes = 0;
        let mut occupied = 0;
        for (row, cells) in self.rows.iter().enumerate() {
            for col in 0..cells.len() {
                writes += 1;
                if !sheet.get_cell(self.row + row, self.col + col).is_empty() {
                    occupied += 1;
                }
            }
        }
        (writes, occupied)
    }

    fn apply(self, sheet: &mut Spreadsheet) {
        sheet.num_rows = sheet.num_rows.max(self.row + self.rows.len());
        sheet.num_cols = sheet.num_cols.max(self.col + self.width);
        for (row, cells) in self.rows.into_iter().enumerate() {
            for (col, value) in cells.into_iter().enumerate() {
                sheet.set_cell(self.row + row, self.col + col, value);
            }
        }
    }
}

impl Spreadsheet {
    pub fn begin_shell_run(&mut self) -> Result<(), &'static str> {
        let (row, col) = (self.cursor_row, self.cursor_col);
        let args = self
            .get_cell(row, col)
            .strip_prefix('=')
            .and_then(|expr| shell_args(expr.trim()))
            .ok_or("Select a cell containing =SHELL(...)")?;
        let command = shell_command(args)
            .ok_or("SHELL needs a command")?
            .to_string();
        self.shell_prompt = Some(ShellPrompt {
            step: ShellStep::Run { row, col, command },
            scroll: 0,
        });
        Ok(())
    }

    pub fn confirm_shell_run(&mut self, runner: impl FnOnce(&str) -> Result<String, String>) {
        let Some(prompt) = &mut self.shell_prompt else {
            return;
        };
        let ShellStep::Run { row, col, command } = &prompt.step else {
            return;
        };
        prompt.step = match runner(command) {
            Ok(stdout) => match ShellOutput::from_stdout(*row, *col, &stdout) {
                Some(output) => ShellStep::Apply(output),
                None => ShellStep::Message("Command produced no output.".to_string()),
            },
            Err(message) => ShellStep::Message(message),
        };
        prompt.scroll = 0;
    }

    pub fn apply_shell_output(&mut self) {
        match self.shell_prompt.take() {
            Some(ShellPrompt {
                step: ShellStep::Apply(output),
                ..
            }) => output.apply(self),
            other => self.shell_prompt = other,
        }
    }
}
