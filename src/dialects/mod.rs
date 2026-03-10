mod single_byte;
mod key_value;

use std::cmp::min;
use std::io::{Read, Write};
pub use single_byte::{SingleByteDialectValidator, SingleByteDialect};
pub use key_value::{KeyValueDialectValidator, KeyValueDialect};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Dialect {
    SingleByte(SingleByteDialect),
    KeyValue(KeyValueDialect)
}

pub trait DialectGroupValidator {
    fn try_process_chunk(&mut self, chunk: &[u8]) -> Result<(), String>;
    fn finalize(&mut self) -> Option<Dialect>;
    fn describe(&self) -> String;
}

trait Normalize {
    fn to_asv(
        &self,
        src: impl Read,
        dest: impl Write
    );
}

fn format_error(desc: &'static str, buffer: &[u8], pos: usize, current_row: usize, current_col: usize, current_byte: usize) -> String {
    const CONTEXT_SIZE: usize = 256;

    let ctx_min = pos.saturating_sub(CONTEXT_SIZE);
    let ctx_max = min(buffer.len(), pos + CONTEXT_SIZE);
    let context = String::from_utf8_lossy(&buffer[ctx_min..ctx_max]);

    let lines_before = buffer[ctx_min..pos].iter().filter(|&&b| b == b'\n').count();
    let line_start = current_row.saturating_sub(lines_before) + 1;

    let span_start = pos - ctx_min;
    let span_end = (span_start + 1).min(context.len());

    let label = format!("col {}, byte {}", current_col, current_byte);

    #[cfg(feature = "debug")]
    {
        use miette::{MietteDiagnostic, LabeledSpan, NamedSource, Report, GraphicalReportHandler, GraphicalTheme};

        let diag = MietteDiagnostic::new(desc)
            .with_labels(vec![
                LabeledSpan::at(span_start..span_end, &label)
            ]);

        let report = Report::new(diag)
            .with_source_code(NamedSource::new("csv", context.into_owned()));

        let mut out = String::new();
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode())
            .render_report(&mut out, report.as_ref())
            .unwrap();
        out
    }

    // #[cfg(feature = "debug")]
    // {
    //     use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};
    //
    //     let report = [Level::ERROR
    //         .primary_title(desc)
    //         .element(
    //             Snippet::source(context.as_ref())
    //                 .path("csv")
    //                 .line_start(line_start)
    //                 .fold(true)
    //                 .annotation(
    //                     AnnotationKind::Primary
    //                         .span(span_start..span_end)
    //                         .label(label.as_str())
    //                 )
    //         )
    //     ];
    //
    //     Renderer::styled().render(&report).to_string()
    // }

    #[cfg(not(feature = "debug"))]
    {
        let _ = (line_start, span_start, span_end);
        format!("{desc} at row {current_row}, {label}")
    }
}

impl Dialect {
    pub fn to_asv(&self, src: impl Read, dest: impl Write) {
        match self {
            Dialect::SingleByte(sb) => {
                sb.to_asv(src, dest)
            }
            Dialect::KeyValue(kv) => {
                kv.to_asv(src, dest)
            }
        }
    }
}
