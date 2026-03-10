//! This dialect group parses simple CSV dialects which
//! have single-byte field separator and can be parsed
//! unambiguously (no unescaped/unquoted field/record
//! separators inside cell values)
//!
//! todo! create dialect group for broken csv files, restoring
//!  rows by detecting ambiguous interval and trying to divide it
//!  in various ways, determining best by looking up for minimal
//!  statistic change (min/max col, num/alphanum, enum,
//!  grex(https://pemistahl.github.io/grex-js/ rules)
//!

use owo_colors::OwoColorize;
use crate::dialects::key_value::KeyValueDialect;
use super::super::{Dialect, DialectGroupValidator, format_error};


#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct KeyValueDialectValidator {
    field_separator: u8,

    broken_rows: usize,

    current_row: usize,
    current_col: usize,
    current_cell_byte: usize,
    current_byte: usize,
}

impl DialectGroupValidator for KeyValueDialectValidator {
    fn try_process_chunk(&mut self, chunk: &[u8]) -> Result<(), String> {
        for (pos, c) in chunk.iter().enumerate() {
            self.try_process_byte(c)
                .map_err(|e| self.format_error(e, chunk, pos))?;

            self.current_byte += 1;
        }

        Ok(())
    }


    fn finalize(&mut self) -> Option<Dialect> {
        // if >50% rows are just key:value
        if self.broken_rows * 2 < self.current_row  {
            Some(Dialect::KeyValue(KeyValueDialect {
                total_rows: self.current_row,
                field_separator: self.field_separator,
            }))
        } else {
            None
        }
    }
    fn describe(&self) -> String {
        let dialect = "KeyValueDialect";
        let sep = char::from(self.field_separator);
        format!("{} ({})", dialect.green().bold(), sep.yellow())
    }
}


impl KeyValueDialectValidator {
    #[allow(clippy::single_element_loop)]
    pub fn make() -> Vec<Self> {
        let mut v = Self::default();
        v.field_separator = b':';
        vec![v]
    }

    #[inline]
    fn try_process_byte(&mut self, c: &u8) -> Result<(), &'static str> {
        // these try_* functions returns true if byte is accepted/consumed

        if self.try_next_row(c)? {
            return Ok(());
        }

        if self.try_next_field(c)? {
            return Ok(());
        }

        self.try_next_char(c)?;
        Ok(())
    }

    #[inline]
    fn try_next_row(&mut self, c: &u8) -> Result<bool, &'static str> {
        if *c == b'\r' {
            Ok(true)
        } else if *c == b'\n' {
            self.end_row()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    #[inline]
    fn try_next_field(&mut self, c: &u8) -> Result<bool, &'static str> {
        if *c == self.field_separator {
            self.end_field()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    #[inline]
    fn try_next_char(&mut self, _c: &u8) -> Result<(), &'static str> {
        self.current_cell_byte += 1;
        const MAX_FIELD_BYTES: usize = 512;
        if self.current_cell_byte > MAX_FIELD_BYTES {
            Err("Cell value too long")
        } else {
            Ok(())
        }
    }

    #[inline]
    fn end_field(&mut self) -> Result<(), &'static str>  {
        self.current_cell_byte = 0;
        self.current_col += 1;
        Ok(())
    }

    #[inline]
    fn end_row(&mut self) -> Result<(), &'static str> {
        if self.current_col == 0 {
            return Err("Only one column found")
        }

        if self.current_col != 1 {
            self.broken_rows += 1;
        }

        self.current_col = 0;
        self.current_row += 1;

        if self.broken_rows == self.current_row && self.broken_rows > 10000 {
            // todo: also may be http:// in right side, i've seen that somewhere
            Err("10k rows analyzed, 3+ columns detected")
        } else {
            Ok(())
        }
    }

    fn format_error(&self, desc: &'static str, buffer: &[u8], pos: usize) -> String {
        format_error(desc, buffer, pos, self.current_row, self.current_col, self.current_byte)
    }
}
