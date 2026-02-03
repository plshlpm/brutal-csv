use brutal_csv::CsvSniffer;
use brutal_csv::dialects::{Dialect, SingleByteDialect, RecordTerminator};

pub fn sniff(data: &[u8]) -> Vec<Dialect> {
    let mut sniffer = CsvSniffer::new(None);
    sniffer.process(&mut &data[..]);
    let mut dialects = sniffer.dialects();
    dialects.sort();
    dialects
}

pub fn sniff_with_headers(data: &[u8], has_headers: Option<bool>) -> Vec<Dialect> {
    let mut sniffer = CsvSniffer::new(has_headers);
    sniffer.process(&mut &data[..]);
    let mut dialects = sniffer.dialects();
    dialects.sort();
    dialects
}

pub fn best_dialect(data: &[u8]) -> Dialect {
    let dialects = sniff(data);
    dialects.into_iter().last().expect("no valid dialect found")
}

pub fn assert_single_byte(
    dialect: &Dialect,
    separator: u8,
    quote: Option<u8>,
    escape: Option<u8>,
    terminator: &RecordTerminator,
) {
    match dialect {
        Dialect::SingleByte(sb) => {
            assert_eq!(sb.field_separator, separator, "separator mismatch");
            assert_eq!(sb.quote_char, quote, "quote_char mismatch");
            assert_eq!(sb.escape_char, escape, "escape_char mismatch");
            assert_eq!(&sb.record_terminator, terminator, "record_terminator mismatch");
        }
        other => panic!("expected SingleByte dialect, got {:?}", other),
    }
}

pub fn to_asv(data: &[u8]) -> Vec<u8> {
    let dialect = best_dialect(data);
    let mut output = Vec::new();
    dialect.to_asv(&mut &data[..], &mut output);
    output
}
