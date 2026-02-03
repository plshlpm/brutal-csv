mod single_byte;
mod key_value;

use std::io::{Read, Write};
pub use single_byte::{SingleByteDialectValidator, SingleByteDialect, RecordTerminator};
pub use key_value::{KeyValueDialectValidator, KeyValueDialect};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Dialect {
    SingleByte(SingleByteDialect),
    KeyValue(KeyValueDialect)
}

pub trait DialectGroupValidator {
    fn try_process_chunk(&mut self, chunk: &[u8]) -> Result<(), String>;
    fn finalize(&mut self) -> Option<Dialect>;
}

trait Normalize {
    fn to_asv(
        &self,
        src: impl Read,
        dest: impl Write
    );
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
