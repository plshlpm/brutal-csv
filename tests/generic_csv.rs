use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use owo_colors::OwoColorize;
use s3::creds::Credentials;
use s3::{Bucket, Region};

fn prepare_cache() -> (String, String) {
    let cache_error = env::var("CACHE_ERROR").unwrap_or_else(|_| "./tests/cache/Error/".to_string());
    let cache_ok = env::var("CACHE_OK").unwrap_or_else(|_| "./tests/cache/Ok/".to_string());
    let cache_size: usize = env::var("CACHE_SIZE").ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    fs::create_dir_all(&cache_error).unwrap();
    fs::create_dir_all(&cache_ok).unwrap();

    // If S3 is configured, download missing samples
    if let (Ok(endpoint), Ok(bucket_name), Ok(access_key), Ok(secret_key)) = (
        env::var("S3_ENDPOINT"),
        env::var("S3_BUCKET"),
        env::var("S3_ACCESS_KEY"),
        env::var("S3_SECRET_KEY"),
    ) {
        let region = Region::Custom { region: String::new(), endpoint };
        let credentials = Credentials::new(
            Some(&access_key), Some(&secret_key),
            None, None, None,
        ).expect("failed to create S3 credentials");

        let bucket = Bucket::new(&bucket_name, region, credentials)
            .expect("failed to create S3 bucket")
            .with_path_style();

        download_samples(&bucket, "Error", &cache_error, cache_size);
        download_samples(&bucket, "Ok", &cache_ok, cache_size);
    }

    (cache_error, cache_ok)
}

fn download_samples(bucket: &Bucket, s3_prefix: &str, local_dir: &str, cache_size: usize) {
    let existing: usize = fs::read_dir(local_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .count();

    if existing >= cache_size {
        return;
    }

    let remaining = cache_size - existing;

    let results = bucket.list(format!("{s3_prefix}/"), Some("/".to_string()))
        .expect("failed to list S3 objects");

    let mut downloaded = 0;
    for result in &results {
        for obj in &result.contents {
            if downloaded >= remaining {
                return;
            }

            let key = &obj.key;
            let filename = match key.rsplit('/').next() {
                Some(f) if !f.is_empty() => f,
                _ => continue,
            };

            let local_path = format!("{local_dir}{filename}");
            if Path::new(&local_path).exists() {
                continue;
            }

            let response = bucket.get_object(key)
                .expect(&format!("failed to download {key}"));

            fs::write(&local_path, response.bytes()).unwrap();
            println!("  downloaded {key}");
            downloaded += 1;
        }
    }
}

fn test_file(path: &Path) -> bool {
    let filename = path.file_name().unwrap().to_string_lossy();
    let data = fs::read(path).unwrap();

    let mut sniffer = brutal_csv::CsvSniffer::new(None);
    sniffer.process(&mut data.as_slice());

    let debug = sniffer.debug();
    let dialects = sniffer.dialects();

    if dialects.is_empty() {
        println!("{}: no valid dialects found", filename.red());
        for (dialect, error) in &debug {
            if error.contains("Only one column found") {
                println!("  {dialect} -> {}", "Only one column found".red());
            } else {
                println!("  {dialect}\n\t{error}");
            }
        }
        return false;
    }

    println!("{}: {} dialect(s) detected", filename.green(), dialects.len());
    true
}

#[test]
fn generic_csv() {
    let (cache_error, cache_ok) = prepare_cache();

    // Collect all files with their source dir, then test, then move
    // (to avoid testing a file twice when it moves between dirs mid-run)
    let mut files: HashMap<PathBuf, &str> = HashMap::new();

    for (dir, origin) in [(&cache_error, "error"), (&cache_ok, "ok")] {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    files.insert(path, origin);
                }
            }
        }
    }

    if files.is_empty() {
        println!("no test files found (S3 may be unreachable)");
        return;
    }

    // Run tests, record results
    let results: Vec<(PathBuf, &str, bool)> = files
        .into_iter()
        .map(|(path, origin)| {
            let passed = test_file(&path);
            (path, origin, passed)
        })
        .collect();

    // Move files based on results
    let mut failures = 0;
    for (path, origin, passed) in &results {
        let filename = path.file_name().unwrap().to_string_lossy();
        match (origin, passed) {
            (&"error", true) => {
                let dest = format!("{cache_ok}{filename}");
                fs::rename(path, &dest).unwrap();
                println!("  -> moved to Ok/");
            }
            (&"ok", false) => {
                let dest = format!("{cache_error}{filename}");
                fs::rename(path, &dest).unwrap();
                println!("  -> moved to Error/ (regression)");
                failures += 1;
            }
            (&"error", false) => {
                failures += 1;
            }
            _ => {}
        }
    }

    assert_eq!(failures, 0, "{failures} file(s) failed out of {}", results.len());
}
