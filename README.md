# brutal-csv

Reliably detect CSV dialect. Dialect is detected by validating whole file against
each supported dialect (so why it's called `brutal-csv`, but 
actually it's not as slow as you may think).

## As a library
It provides `CsvSniffer` struct that can validate CSV file and 
return list of dialects for which file is valid.

```rust,ignore
use std::fs::File;
let mut sniffer = brutal_csv::CsvSniffer::new();
let mut reader = File::open("/etc/group").unwrap(); // that's also CSV-like file

sniffer.process(&mut reader);
let dialects = sniffer.dialects();
assert!(dialects.len() > 0);
for dialect in dialects {
  println!("{:?}", dialect);
}
```

## As a binary (`csv2asv`)

Library also provides a way to transform CSV files, but only 
implemented destination format is very specific (and way simpler than CSV):
  - Header row is always present, using `__NO_HEADER__` 
    as a placeholder if original file did not contain header
  - Field delimiter is `0x1f` (`UNIT SEPARATOR`)
  - Row terminator is `0x1e` (`RECORD SEPARATOR`)
  - No escaping or quoting, parsing is simply splitting

CSV output is not implemented because it will require either 
duplication of normalizer code or introducing an abstraction 
that will lead to performance loss.


## Testing
Test generic_csv read files from s3 bucket
```bash
cargo test
```


