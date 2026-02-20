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

use std::cmp::{min, max};
use std::string::FromUtf8Error;
use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};
use owo_colors::OwoColorize;
use super::super::{Dialect, DialectGroupValidator};
use super::{RecordTerminator, SingleByteDialect};


#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct SingleByteDialectValidator {
    first_row: Vec<Vec<u8>>,

    quote_active: bool,
    escape_active: bool,
    quote_char: Option<u8>,
    escape_char: Option<u8>,

    current_cell_is_numeric: bool,
    current_cell_is_ascii: bool,

    ascii_columns: Vec<bool>,
    numeric_columns: Vec<bool>,
    col_min_len: Vec<usize>,
    col_max_len: Vec<usize>,

    prev_char_was_cr: bool,
    record_terminator: RecordTerminator,
    field_separator_is_terminator: bool,

    field_separator: u8,

    has_escaped_line_breaks: bool,
    has_quoted_line_breaks: bool,

    current_row: usize,
    current_col: usize,
    current_cell_byte: usize,
    current_byte: usize,

    has_headers_user: Option<bool>
}

impl DialectGroupValidator for SingleByteDialectValidator {
    fn try_process_chunk(&mut self, chunk: &[u8]) -> Result<(), String> {
        for (pos, c) in chunk.iter().enumerate() {
            self.try_process_byte(c)
                .map_err(|e| self.format_error(e, chunk, pos))?;

            self.current_byte += 1;
        }

        Ok(())
    }


    fn finalize(&mut self) -> Option<Dialect> {
        self.check_field_separator_is_terminator();

        let empty_columns: Vec<bool> = self.col_max_len
            .iter()
            .map(|x| *x == 0)
            .collect();

        let numeric_columns = self.numeric_columns.clone();

        // That's either invalid CSV or completely empty file, 
        // in any case we won't parse it.
        if empty_columns.iter().all(|x|*x) {
            return None
        }

        const MIN_ROWS: usize = 5;
        if self.current_row < MIN_ROWS {
            return None;
        }

        Some(Dialect::SingleByte(SingleByteDialect {
            header: self.try_get_headers(),
            field_separator: self.field_separator,
            quote_char: self.quote_char,
            escape_char: self.escape_char,
            empty_columns,
            numeric_columns,
            record_terminator: self.record_terminator.clone(),
            field_separator_is_terminator: self.field_separator_is_terminator,
            has_escaped_line_breaks: self.has_escaped_line_breaks,
            has_quoted_line_breaks: self.has_quoted_line_breaks,
            total_rows: self.current_row,
        }))
    }
    fn describe(&self) -> String {
        let record_term = match &self.record_terminator {
            RecordTerminator::Crlf        => "CRLF".to_string(),
            RecordTerminator::Byte(b'\n') => "LF".to_string(),
            RecordTerminator::Byte(b'\r') => "CR".to_string(),
            RecordTerminator::Byte(b)     => format!("Byte(0x{b:02X})"),
        };

        format!("{} field_separator: {} | quote_char: {} | escape_char: {} | record_terminator: {} | field_separator_is_terminator: {} | has_escaped_line_breaks: {} | has_quoted_line_breaks: {}",
            "SingleByte Dialect".cyan().bold(),
            format!("{:?}", char::from(self.field_separator)).yellow(),
            format!("{:?}", self.quote_char.map(char::from)).yellow(),
            format!("{:?}", self.escape_char.map(char::from)).yellow(),
            record_term.yellow(),
            self.field_separator_is_terminator.yellow(),
            self.has_escaped_line_breaks.yellow(),
            self.has_quoted_line_breaks.yellow())
    }
}


impl SingleByteDialectValidator {
    // #[allow(clippy::single_element_loop)]
    // pub fn _make() -> Vec<Self> {
    //     let mut variants = vec![
    //         Self {
    //             escape_char: None,
    //             quote_char: Some(b'"'),
    //             field_separator: b';',
    //             record_terminator: RecordTerminator::Crlf,
    //             has_quoted_line_breaks: true,
    //             has_escaped_line_breaks: false,
    //             ..Default::default()
    //         }
    //     ];
    //
    //     for v in &mut variants {
    //         v.push_first_row_cell();
    //     }
    //
    //     variants
    // }
    
    #[allow(clippy::single_element_loop)]
    pub fn make(has_headers: Option<bool>) -> Vec<Self> {
        let mut variants = vec![SingleByteDialectValidator {
            has_headers_user: has_headers,
            ..Default::default()
        }];

        for mut v in variants.clone().into_iter() {
            v.has_quoted_line_breaks = true;
            variants.push(v);
        }

        for mut v in variants.clone().into_iter() {
            v.has_quoted_line_breaks = true;
            variants.push(v);
        }

        for mut v in variants.clone().into_iter() {
            v.escape_char = Some(b'\\');
            variants.push(v);
        }

        for mut v in variants.clone().into_iter() {
            for q in [b'"', b'\''] {
                v.quote_char = Some(q);
                variants.push(v.clone());
            }
        }

        for mut v in variants.clone().into_iter() {
            for q in [b'\t', b',', b';', b'|', b':'] {
                v.field_separator = q;
                variants.push(v.clone());
            }
        }

        for mut v in variants.clone().into_iter() {
            for q in [RecordTerminator::Byte(b'\n')] {
                v.record_terminator = q;
                variants.push(v.clone());
            }
        }

        for v in &mut variants {
            v.push_first_row_cell();
        }

        variants
    }

    #[inline]
    fn try_process_byte(&mut self, c: &u8) -> Result<(), &'static str> {
        // these try_* functions returns true if byte is accepted/consumed
        if self.try_escape(c)? {
            if !self.has_escaped_line_breaks {
                self.try_next_row(c)?;
            }
            return Ok(());
        }

        if self.try_quote(c)? {
            if !self.has_quoted_line_breaks {
                self.try_next_row(c)?;
            }
            return Ok(());
        }

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
    fn try_escape(&mut self, c: &u8) -> Result<bool, &'static str> {
        if self.escape_active {
            self.escape_active = false;
            return Ok(true);
        }

        if let Some(e) = self.escape_char {
            if *c == e {
                self.escape_active = true;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    #[inline]
    fn try_quote(&mut self, c: &u8) -> Result<bool, &'static str>  {
        if let Some(q) = self.quote_char {
            let was_active = self.quote_active;
            // switch if current char is quote
            self.quote_active ^= q == *c;
            Ok(was_active)
        } else {
            Ok(false)
        }
    }

    #[inline]
    fn try_next_row(&mut self, c: &u8) -> Result<bool, &'static str> {
        match &self.record_terminator {
            RecordTerminator::Byte(t) => {
                if c == t {
                    self.end_row()?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            RecordTerminator::Crlf => {
                if *c == b'\r' {
                    self.prev_char_was_cr = true;
                    Ok(true)
                } else if *c == b'\n' && self.prev_char_was_cr {
                    self.prev_char_was_cr = false;
                    self.end_row()?;
                    Ok(true)
                } else {
                    self.prev_char_was_cr = false;
                    Ok(false)
                }
            }
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
    fn try_next_char(&mut self, c: &u8) -> Result<(), &'static str> {
        if self.current_row == 0 {
            self.push_first_row_char(c);
        }
        self.current_cell_is_numeric &= c.is_ascii_digit();
        self.current_cell_is_ascii &= c.is_ascii();
        self.current_cell_byte += 1;
        const MAX_FIELD_BYTES: usize = 1024 * 1024 * 10; // 10 MiB
        if self.current_cell_byte > MAX_FIELD_BYTES {
            Err("Cell value too long")
        } else {
            Ok(())
        }
    }

    #[inline]
    fn end_field(&mut self) -> Result<(), &'static str>  {
        const MAX_COLUMNS: usize = 5000;

        if self.current_row != 0 {
            if self.current_col == self.ascii_columns.len() {
                return Err("Inconsistent row length")
            }

            self.ascii_columns[self.current_col] &= self.current_cell_is_ascii;
            self.numeric_columns[self.current_col] &= self.current_cell_is_numeric;
            self.col_min_len[self.current_col] = min(self.col_min_len[self.current_col], self.current_cell_byte);
            self.col_max_len[self.current_col] = max(self.col_max_len[self.current_col], self.current_cell_byte);
        } else {
            self.push_first_row_cell();
            if self.current_col > MAX_COLUMNS {
                return Err("Too many columns (first row)")
            }
        }

        self.quote_active = false;
        self.escape_active = false;
        self.current_cell_is_ascii = true;
        self.current_cell_is_numeric = true;
        self.current_cell_byte = 0;
        self.current_col += 1;
        Ok(())
    }

    #[inline]
    fn end_row(&mut self) -> Result<(), &'static str> {
        if self.current_row != 0 && self.current_col != self.first_row.len() - 1 {
            return Err("Inconsistent row length (missing column)")
        }
        if self.current_col == 0 {
            return Err("Only one column found")
        }

        self.end_field()?;
        // .end_field() always starts new column
        if self.current_row == 0 {
            self.pop_first_row_cell();
        }

        self.prev_char_was_cr = false;
        self.current_col = 0;
        self.current_row += 1;

        Ok(())
    }

    #[cold]
    fn push_first_row_char(&mut self, c: &u8) {
        self.first_row[self.current_col].push(*c);
    }

    #[cold]
    fn push_first_row_cell(&mut self) {
        self.first_row.push(vec![]);
        self.col_min_len.push(usize::MAX);
        self.col_max_len.push(usize::MIN);
        self.ascii_columns.push(true);
        self.numeric_columns.push(true);
    }

    #[cold]
    fn pop_first_row_cell(&mut self) {
        self.first_row.pop();
        self.col_min_len.pop();
        self.col_max_len.pop();
        self.ascii_columns.pop();
        self.numeric_columns.pop();
    }

    fn check_field_separator_is_terminator(&mut self) {
        let last_col_is_empty = *self.col_max_len.last().unwrap() == 0;
        let last_col_name_is_empty = self.first_row.last().unwrap().is_empty();

        if last_col_is_empty && last_col_name_is_empty {
            self.field_separator_is_terminator = true;
            self.pop_first_row_cell();
        }
    }

    /// Detect headers, if present then return them.
    /// Assuming headers exists if either:
    /// - first line value is shorter than the shortest value in that column 
    /// - first line value is longer than the longest value in that column
    ///   - that also includes case when whole column is empty but first line
    /// - first line value is non-numeric, but all other values are
    /// - first value is non-ascii but all other values are
    ///   (e.g. header is non-english)
    fn try_get_headers(&self) -> Option<Vec<String>> {
        let first_row = self.first_row
            .iter()
            .map(|x| String::from_utf8(x.clone()))
            .collect::<Result<Vec<String>, FromUtf8Error>>()
            .ok();

        if let Some(header) = first_row {
            if self.has_headers_user == Some(true) {
                return Some(header);
            }

            if self.has_headers_user == Some(false) {
                return None;
            }

            const MAX_HEADER_SIZE: usize = 256;
            for column_name in header.iter() {
                if column_name.len() > MAX_HEADER_SIZE {
                    return None;
                }


                let column_name_lc = column_name.to_lowercase();
                // kinda dirty, but works for my case
                if KNOWN_HEADERS.contains(&&*column_name_lc) {
                    return Some(header);
                }

                if column_name_lc.ends_with("_id") {
                    return Some(header);
                }
                if column_name_lc.ends_with("name") {
                    return Some(header);
                }
            }

            for col_id in 0..header.len() {
                let col_min = self.col_min_len[col_id];
                let col_max = self.col_max_len[col_id];
                let col_header_len = header[col_id].len();

                if !(col_min..=col_max).contains(&col_header_len) {
                    return Some(header)
                }
            }

            for (col_id, is_ascii) in self.ascii_columns.iter().enumerate() {
                if *is_ascii && !self.first_row[col_id].is_ascii() {
                    return Some(header)
                }
            }

            for (col_id, is_numeric) in self.numeric_columns.iter().enumerate() {
                if *is_numeric && !header[col_id].chars().all(|c|c.is_ascii_digit()) {
                    return Some(header)
                }
            }
        }

        None
    }
    
    fn format_error(&self, desc: &'static str, buffer: &[u8], pos: usize) -> String {
        const CONTEXT_SIZE: usize = 256;

        let ctx_min = max(0, pos.clamp(CONTEXT_SIZE, usize::MAX) - CONTEXT_SIZE);
        let ctx_max = min(buffer.len() - 1, pos + CONTEXT_SIZE);
        let context = String::from_utf8_lossy(&buffer[ctx_min..ctx_max]);
        let error_message = format!("{desc} at {}:{} (offset={}) near", self.current_row, self.current_col, self.current_byte);
        format!("{}\n`{context}`", error_message.red())
    }
}


const KNOWN_HEADERS: &'static [&'static str; 25] = &[
    "email",
    "id",
    "full_name",
    "phone_number",
    "address",
    "phone",
    "password",
    "first_name",
    "fio",
    "адрес",
    "date_of_birth",
    "time",
    "status",
    "city",
    "admin",
    "country",
    "created_at",
    "gender",
    "instagram",
    "ip",
    "last_name",
    "lastname",
    "vip",
    "work",
    "телефон"
];
