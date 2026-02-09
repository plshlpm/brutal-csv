//! Pollock Benchmark Test Framework
//!
//! Standard implementation following the HPI Pollock benchmark structure:
//! - polluted_files/csv/       - Input CSV files with various issues
//! - polluted_files/clean/     - Expected clean output
//! - polluted_files/parameters/ - JSON with expected dialect parameters
//!
//! Reference: https://github.com/HPI-Information-Systems/Pollock

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use brutal_csv::dialects::{Dialect, RecordTerminator};
use brutal_csv::CsvSniffer;
use serde::Deserialize;

/// Expected dialect parameters from Pollock benchmark JSON
#[derive(Debug, Deserialize, Clone)]
pub struct PollockParameters {
    pub encoding: String,
    pub delimiter: String,
    pub quotechar: String,
    pub escapechar: String,
    pub row_delimiter: String,
    pub header_lines: usize,
    pub preamble_lines: usize,
    pub footnote_lines: usize,
    pub column_names: Vec<String>,
    pub n_columns: usize,
}

impl PollockParameters {
    /// Parse delimiter string to byte (handles hex like "0x3B")
    pub fn delimiter_byte(&self) -> Option<u8> {
        parse_char_spec(&self.delimiter)
    }

    /// Parse quotechar string to byte
    pub fn quotechar_byte(&self) -> Option<u8> {
        parse_char_spec(&self.quotechar)
    }

    /// Parse escapechar string to byte
    pub fn escapechar_byte(&self) -> Option<u8> {
        parse_char_spec(&self.escapechar)
    }

    /// Parse row_delimiter to RecordTerminator
    pub fn record_terminator(&self) -> RecordTerminator {
        match self.row_delimiter.as_str() {
            "\r\n" => RecordTerminator::Crlf,
            "\n" => RecordTerminator::Byte(b'\n'),
            "\r" => RecordTerminator::Byte(b'\r'),
            _ => RecordTerminator::Byte(b'\n'),
        }
    }

    pub fn has_header(&self) -> bool {
        self.header_lines > 0
    }
}

fn parse_char_spec(s: &str) -> Option<u8> {
    if s.is_empty() {
        return None;
    }
    // Handle single character
    if s.len() == 1 {
        return Some(s.bytes().next().unwrap());
    }
    // Handle escape sequences
    match s {
        "\\t" => Some(b'\t'),
        "\\n" => Some(b'\n'),
        "\\r" => Some(b'\r'),
        _ => {
            // Handle hex notation like "0x3B"
            if s.starts_with("0x") || s.starts_with("0X") {
                u8::from_str_radix(&s[2..], 16).ok()
            } else {
                s.bytes().next()
            }
        }
    }
}

/// A single test case from the Pollock benchmark
#[derive(Debug)]
pub struct PollockTestCase {
    pub name: String,
    pub csv_path: PathBuf,
    pub clean_path: PathBuf,
    pub parameters_path: PathBuf,
    pub parameters: Option<PollockParameters>,
}

impl PollockTestCase {
    pub fn load_parameters(&mut self) -> Result<&PollockParameters, String> {
        if self.parameters.is_some() {
            return Ok(self.parameters.as_ref().unwrap());
        }

        let content = fs::read_to_string(&self.parameters_path)
            .map_err(|e| format!("Failed to read parameters: {}", e))?;

        let params: PollockParameters = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse parameters JSON: {}", e))?;

        self.parameters = Some(params);
        Ok(self.parameters.as_ref().unwrap())
    }

    pub fn load_csv(&self) -> Result<Vec<u8>, String> {
        fs::read(&self.csv_path)
            .map_err(|e| format!("Failed to read CSV: {}", e))
    }

    pub fn load_clean(&self) -> Result<Vec<u8>, String> {
        fs::read(&self.clean_path)
            .map_err(|e| format!("Failed to read clean file: {}", e))
    }
}

/// Result of evaluating a single test case
#[derive(Debug)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub duration: Duration,
    pub detected_dialect: Option<DetectedDialect>,
    pub expected_params: Option<PollockParameters>,
}

#[derive(Debug)]
pub enum TestStatus {
    /// Dialect detected and matches expected parameters
    Success,
    /// Dialect detected but doesn't match expected
    Mismatch { reason: String },
    /// No dialect could be detected
    NoDialect,
    /// Error during processing
    Error { message: String },
}

impl TestStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, TestStatus::Success)
    }

    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }
}

/// Simplified representation of detected dialect
#[derive(Debug, Clone)]
pub struct DetectedDialect {
    pub dialect_type: String,
    pub delimiter: Option<u8>,
    pub quotechar: Option<u8>,
    pub escapechar: Option<u8>,
    pub record_terminator: RecordTerminator,
    pub has_header: bool,
    pub n_columns: usize,
    pub total_rows: usize,
}

impl From<&Dialect> for DetectedDialect {
    fn from(d: &Dialect) -> Self {
        match d {
            Dialect::SingleByte(sb) => DetectedDialect {
                dialect_type: "SingleByte".into(),
                delimiter: Some(sb.field_separator),
                quotechar: sb.quote_char,
                escapechar: sb.escape_char,
                record_terminator: sb.record_terminator.clone(),
                has_header: sb.header.is_some(),
                n_columns: sb.empty_columns.len(),
                total_rows: sb.total_rows,
            },
            Dialect::KeyValue(kv) => DetectedDialect {
                dialect_type: "KeyValue".into(),
                delimiter: Some(kv.field_separator),
                quotechar: None,
                escapechar: None,
                record_terminator: RecordTerminator::Byte(b'\n'),
                has_header: false,
                n_columns: 2,
                total_rows: kv.total_rows,
            },
        }
    }
}

/// Aggregated benchmark statistics
#[derive(Debug, Default)]
pub struct BenchmarkStats {
    pub total: usize,
    pub success: usize,
    pub mismatch: usize,
    pub no_dialect: usize,
    pub errors: usize,
    pub total_duration: Duration,

    // Detailed tracking
    pub delimiter_correct: usize,
    pub quotechar_correct: usize,
    pub terminator_correct: usize,
    pub header_correct: usize,

    pub failures: Vec<(String, String)>, // (name, reason)
}

impl BenchmarkStats {
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 { 0.0 } else { (self.success as f64 / self.total as f64) * 100.0 }
    }

    pub fn detection_rate(&self) -> f64 {
        let detected = self.success + self.mismatch;
        if self.total == 0 { 0.0 } else { (detected as f64 / self.total as f64) * 100.0 }
    }

    pub fn add(&mut self, result: &TestResult) {
        self.total += 1;
        self.total_duration += result.duration;

        match &result.status {
            TestStatus::Success => {
                self.success += 1;
                self.delimiter_correct += 1;
                self.quotechar_correct += 1;
                self.terminator_correct += 1;
                self.header_correct += 1;
            }
            TestStatus::Mismatch { reason } => {
                self.mismatch += 1;
                self.failures.push((result.name.clone(), reason.clone()));

                // Partial credit tracking
                if let (Some(detected), Some(expected)) = (&result.detected_dialect, &result.expected_params) {
                    if detected.delimiter == expected.delimiter_byte() {
                        self.delimiter_correct += 1;
                    }
                    if detected.quotechar == expected.quotechar_byte() {
                        self.quotechar_correct += 1;
                    }
                    if detected.record_terminator == expected.record_terminator() {
                        self.terminator_correct += 1;
                    }
                    if detected.has_header == expected.has_header() {
                        self.header_correct += 1;
                    }
                }
            }
            TestStatus::NoDialect => {
                self.no_dialect += 1;
                self.failures.push((result.name.clone(), "no dialect detected".into()));
            }
            TestStatus::Error { message } => {
                self.errors += 1;
                self.failures.push((result.name.clone(), message.clone()));
            }
        }
    }

    pub fn print_summary(&self) {
        println!("\n{:=^60}", " Pollock Benchmark Results ");
        println!("Total test cases:  {}", self.total);
        println!("Success:           {} ({:.1}%)", self.success, self.success_rate());
        println!("Mismatch:          {}", self.mismatch);
        println!("No dialect:        {}", self.no_dialect);
        println!("Errors:            {}", self.errors);
        println!("Detection rate:    {:.1}%", self.detection_rate());
        println!("Total duration:    {:?}", self.total_duration);

        if self.total > 0 {
            let detected = self.success + self.mismatch;
            if detected > 0 {
                println!("\nParameter Accuracy (among detected):");
                println!("  Delimiter:   {}/{} ({:.1}%)", self.delimiter_correct, detected,
                    self.delimiter_correct as f64 / detected as f64 * 100.0);
                println!("  Quote char:  {}/{} ({:.1}%)", self.quotechar_correct, detected,
                    self.quotechar_correct as f64 / detected as f64 * 100.0);
                println!("  Terminator:  {}/{} ({:.1}%)", self.terminator_correct, detected,
                    self.terminator_correct as f64 / detected as f64 * 100.0);
                println!("  Header:      {}/{} ({:.1}%)", self.header_correct, detected,
                    self.header_correct as f64 / detected as f64 * 100.0);
            }
        }

        if !self.failures.is_empty() && self.failures.len() <= 20 {
            println!("\nFailures:");
            for (name, reason) in &self.failures {
                println!("  - {}: {}", name, reason);
            }
        } else if !self.failures.is_empty() {
            println!("\nFailures: {} (showing first 20)", self.failures.len());
            for (name, reason) in self.failures.iter().take(20) {
                println!("  - {}: {}", name, reason);
            }
        }
    }
}

/// Iterator over Pollock test cases
pub struct PollockIterator {
    cases: Vec<PollockTestCase>,
    index: usize,
}

impl PollockIterator {
    pub fn new(base_dir: &Path) -> Result<Self, String> {
        let csv_dir = base_dir.join("csv");
        let clean_dir = base_dir.join("clean");
        let params_dir = base_dir.join("parameters");

        if !csv_dir.exists() {
            return Err(format!("CSV directory not found: {:?}", csv_dir));
        }

        let mut cases = Vec::new();

        let entries = fs::read_dir(&csv_dir)
            .map_err(|e| format!("Failed to read csv dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let csv_path = entry.path();

            if let Some(ext) = csv_path.extension() {
                if ext.to_string_lossy().to_lowercase() != "csv" {
                    continue;
                }
            } else {
                continue;
            }

            let file_name = csv_path.file_name().unwrap().to_string_lossy();
            let clean_path = clean_dir.join(&*file_name);
            let params_path = params_dir.join(format!("{}_parameters.json", file_name));

            cases.push(PollockTestCase {
                name: file_name.to_string(),
                csv_path,
                clean_path,
                parameters_path: params_path,
                parameters: None,
            });
        }

        cases.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self { cases, index: 0 })
    }

    pub fn len(&self) -> usize {
        self.cases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

impl Iterator for PollockIterator {
    type Item = PollockTestCase;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.cases.len() {
            let case = std::mem::replace(
                &mut self.cases[self.index],
                PollockTestCase {
                    name: String::new(),
                    csv_path: PathBuf::new(),
                    clean_path: PathBuf::new(),
                    parameters_path: PathBuf::new(),
                    parameters: None,
                },
            );
            self.index += 1;
            Some(case)
        } else {
            None
        }
    }
}

/// Main benchmark executor
pub struct PollockBenchmark {
    base_dir: PathBuf,
}

impl PollockBenchmark {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Evaluate a single test case
    pub fn evaluate(&self, mut case: PollockTestCase) -> TestResult {
        let start = Instant::now();

        // Load expected parameters
        let params = match case.load_parameters() {
            Ok(p) => p.clone(),
            Err(e) => {
                return TestResult {
                    name: case.name,
                    status: TestStatus::Error { message: e },
                    duration: start.elapsed(),
                    detected_dialect: None,
                    expected_params: None,
                };
            }
        };

        // Load CSV data
        let data = match case.load_csv() {
            Ok(d) => d,
            Err(e) => {
                return TestResult {
                    name: case.name,
                    status: TestStatus::Error { message: e },
                    duration: start.elapsed(),
                    detected_dialect: None,
                    expected_params: Some(params),
                };
            }
        };

        // Detect dialect
        let mut sniffer = CsvSniffer::new(None);
        sniffer.process(&mut Cursor::new(&data));

        let dialects = sniffer.dialects();
        let best = dialects.into_iter().max();

        let detected = match best {
            Some(d) => DetectedDialect::from(&d),
            None => {
                return TestResult {
                    name: case.name,
                    status: TestStatus::NoDialect,
                    duration: start.elapsed(),
                    detected_dialect: None,
                    expected_params: Some(params),
                };
            }
        };

        // Compare detected vs expected
        let status = self.compare_dialect(&detected, &params);

        TestResult {
            name: case.name,
            status,
            duration: start.elapsed(),
            detected_dialect: Some(detected),
            expected_params: Some(params),
        }
    }

    fn compare_dialect(&self, detected: &DetectedDialect, expected: &PollockParameters) -> TestStatus {
        let mut mismatches = Vec::new();

        // Check delimiter
        if detected.delimiter != expected.delimiter_byte() {
            mismatches.push(format!(
                "delimiter: got {:?}, expected {:?}",
                detected.delimiter.map(|b| b as char),
                expected.delimiter_byte().map(|b| b as char)
            ));
        }

        // Check quote char
        if detected.quotechar != expected.quotechar_byte() {
            mismatches.push(format!(
                "quotechar: got {:?}, expected {:?}",
                detected.quotechar.map(|b| b as char),
                expected.quotechar_byte().map(|b| b as char)
            ));
        }

        // Check record terminator
        if detected.record_terminator != expected.record_terminator() {
            mismatches.push(format!(
                "terminator: got {:?}, expected {:?}",
                detected.record_terminator,
                expected.record_terminator()
            ));
        }

        // Check header
        if detected.has_header != expected.has_header() {
            mismatches.push(format!(
                "header: got {}, expected {}",
                detected.has_header,
                expected.has_header()
            ));
        }

        if mismatches.is_empty() {
            TestStatus::Success
        } else {
            TestStatus::Mismatch {
                reason: mismatches.join("; "),
            }
        }
    }

    /// Run benchmark on all test cases
    pub fn run(&self, verbose: bool) -> BenchmarkStats {
        let mut stats = BenchmarkStats::default();

        let iter = match PollockIterator::new(&self.base_dir) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Failed to initialize benchmark: {}", e);
                return stats;
            }
        };

        let total = iter.len();
        println!("Running Pollock benchmark: {} test cases", total);

        for case in iter {
            let name = case.name.clone();
            let result = self.evaluate(case);

            if verbose {
                let status_str = match &result.status {
                    TestStatus::Success => "[OK]  ",
                    TestStatus::Mismatch { .. } => "[MISS]",
                    TestStatus::NoDialect => "[NONE]",
                    TestStatus::Error { .. } => "[ERR] ",
                };
                println!("{} {:50} {:?}", status_str, truncate(&name, 50), result.duration);
            }

            stats.add(&result);
        }

        stats
    }

    /// Run and collect all results
    pub fn run_collect(&self) -> (Vec<TestResult>, BenchmarkStats) {
        let mut stats = BenchmarkStats::default();
        let mut results = Vec::new();

        let iter = match PollockIterator::new(&self.base_dir) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Failed to initialize benchmark: {}", e);
                return (results, stats);
            }
        };

        for case in iter {
            let result = self.evaluate(case);
            stats.add(&result);
            results.push(result);
        }

        (results, stats)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        format!("{:<width$}", s, width = max)
    } else {
        format!("{}...", &s[..max - 3])
    }
}

/// Get default Pollock benchmark directory
pub fn default_pollock_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("polluted_files")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_char_spec() {
        assert_eq!(parse_char_spec(","), Some(b','));
        assert_eq!(parse_char_spec(";"), Some(b';'));
        assert_eq!(parse_char_spec("\\t"), Some(b'\t'));
        assert_eq!(parse_char_spec("0x3B"), Some(b';'));
        assert_eq!(parse_char_spec("0x09"), Some(b'\t'));
        assert_eq!(parse_char_spec(""), None);
    }
}
