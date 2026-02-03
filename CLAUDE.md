# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build                        # Debug build (library only)
cargo build --features binary      # Build with csv2asv binary
cargo build --features progress    # Build binary with progress bar support
cargo build --release --features binary  # Release build with binary
cargo test                         # Run tests (no test suite exists yet)
cargo clippy                       # Lint
```

The `binary` feature gates the CLI (`csv2asv`), adding clap/clio dependencies. The `progress` feature extends `binary` with indicatif progress bars.

## Architecture

**brutal-csv** is a Rust library that detects CSV dialects by exhaustively validating files against all supported dialect combinations, then ranks survivors by heuristic quality. It also provides conversion to ASV (ASCII Separated Values) format.

### Core Flow

1. `CsvSniffer` (lib.rs) creates multiple `DialectGroupValidator` instances — one per candidate dialect combination (separator × quote char × escape char × line terminator × quoted-linebreak mode)
2. File data streams through all validators simultaneously in 1 MiB chunks, byte-by-byte via state machines
3. Validators that encounter inconsistencies (column count mismatch, constraint violations) are dropped
4. Surviving dialects are ranked via `PartialOrd` implementation (headers > numeric columns > fewer escape complexities > more rows > CRLF)
5. Best dialect can convert input to ASV via the `Normalize` trait

### Module Layout

- **`src/lib.rs`** — `CsvSniffer` entry point, orchestrates chunk-based streaming across all validators
- **`src/dialects/mod.rs`** — `Dialect` enum, `DialectGroupValidator` trait, `Normalize` trait
- **`src/dialects/single_byte/`** — Main CSV detection:
  - `mod.rs` — `SingleByteDialect` struct with `PartialOrd` for ranking, header detection heuristics (known header list, suffix matching, type/length mismatch)
  - `detector.rs` — `SingleByteDialectValidator` state machine: byte-level parsing tracking quote/escape state, column statistics (numeric, ASCII, length), enforces constraints (max 5000 columns, max 10 MiB cell, min 5 rows, max 256-char headers)
  - `normalizer.rs` — Converts to ASV format, drops empty columns
- **`src/dialects/key_value/`** — Detects simple key:value files (>50% single-separator rows)
- **`src/bin/csv2asv.rs`** — CLI binary, two-pass (detect then convert), requires seekable input

### Key Design Patterns

- **Multiple-validator strategy**: All dialect combinations tested in parallel against the same data; failures prune the search space
- **Streaming state machine**: Byte-by-byte processing with quote/escape toggle flags, processes arbitrarily large files in constant memory
- **Two-pass binary**: First pass detects dialect, stream rewinds, second pass normalizes to ASV
- **ASV output format**: Field delimiter `0x1f` (UNIT SEPARATOR), record terminator `0x1e` (RECORD SEPARATOR), no escaping needed, header always present (`__NO_HEADER__` placeholder if undetected)
