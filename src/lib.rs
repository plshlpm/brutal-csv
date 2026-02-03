#![doc = include_str!("../README.md")]

use std::io::Read;
use crate::dialects::{Dialect, DialectGroupValidator, KeyValueDialectValidator, SingleByteDialectValidator};

pub mod dialects;

#[derive(Default)]
pub struct CsvSniffer {
    validators: Vec<Box<dyn DialectGroupValidator>>
}

impl CsvSniffer {

    /// None = unknown (default)
    /// Some(true) = assume with headers 
    /// Some(false) = assume without headers   
    pub fn new(has_headers: Option<bool>) -> Self {
        let mut validators = vec![];

        validators.extend(SingleByteDialectValidator::make(has_headers)
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn DialectGroupValidator>)
        );

        validators.extend(KeyValueDialectValidator::make()
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn DialectGroupValidator>)
        );

        Self {
            validators
        }
    }

    /// Validates file against each CSV dialect.
    ///
    /// You must pass whole file into it, otherwise behaviour is undefined.
    pub fn process<T: Read>(&mut self, reader: &mut T) {
        let mut buffer = [b'0'; 1024*1024]; // 1 MiB chunks

        loop {
            let chunk_size = reader.read(&mut buffer).unwrap();
            if chunk_size == 0 {
                break
            }

            self.process_chunk(&buffer[0..chunk_size]);
            if self.validators.is_empty() {
                break
            }
        }
    }

    #[inline]
    fn process_chunk(&mut self, chunk: &[u8]) {
        self.validators.retain_mut(|c| {
            let res = c.try_process_chunk(chunk);
            // if let Err(e) = &res {
            //     eprintln!("{}", e)
            // }
        
            res.is_ok()
        });
    }

    /// Returns valid dialects for processed file.
    pub fn dialects(self) -> Vec<Dialect> {
        self.validators
            .into_iter()
            .filter_map(|mut x| x.finalize())
            .collect()
    }
}



