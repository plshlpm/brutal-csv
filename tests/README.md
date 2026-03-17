# Integration tests

## Overview

The `generic_csv` test is a data-driven regression pipeline. It downloads CSV samples from S3 (MinIO), runs dialect detection via `brutal_csv::CsvSniffer`, and tracks results in a state file.

## State file

`state.json` is a `HashMap<String, Vec<String>>` where keys are dialect descriptions (output of `describe()`) and values are lists of filenames. Files that failed detection go under the `"Error"` key.

Each test run produces a timestamped snapshot: `state.{unix_timestamp}.json`.

## Test pipeline

1. Load config from `.env` (see `.env.example`)
2. Create `cache/` directory for downloaded files
3. Load `state.json` — from local file, S3, or create empty
4. Download one file per known dialect for regression testing
5. Download up to `ERR_CACHE_SIZE` error files to retry detection
6. Run `CsvSniffer` on each file
7. Build new state from results:
   - If an Error file now detects a dialect — logged as **IMPROVEMENT**
   - If a previously good file fails — logged as **REGRESSION**
8. Save new state as `state.{timestamp}.json`
9. Assert zero regressions

## Configuration

Copy `.env.example` to `.env` and fill in values:

| Variable | Default | Description |
|---|---|---|
| `CACHE_DIR` | `tests/cache/` | Local directory for downloaded samples |
| `STATE_PATH` | `tests/state.json` | Path to state file |
| `ERR_CACHE_SIZE` | `10` | Max error files to download per run |
| `S3_ENDPOINT` | — | S3-compatible endpoint URL |
| `S3_BUCKET` | — | Bucket name |
| `S3_PATH` | `examples` | Prefix for sample files in the bucket |
| `S3_ACCESS_KEY` | — | Access key |
| `S3_SECRET_KEY` | — | Secret key |

S3 parameters are optional. Without them the test works with local cache only.

## Just recipes

| Command | Description |
|---|---|
| `just test` | Run tests with stdout visible |
| `just clean` | Remove `cache/`, `state.json` and snapshots |
| `just sync` | Upload latest state snapshot to MinIO |
| `just empty-state` | Build initial state from S3 bucket file listing |
