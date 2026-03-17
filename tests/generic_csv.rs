use std::collections::HashMap;
use std::env;
use dotenv::dotenv;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use owo_colors::OwoColorize;
use s3::creds::Credentials;
use s3::{Bucket, Region};

type State = HashMap<String, Vec<String>>;

#[derive(Debug)]
struct Config {
    cache_dir: String,
    state_path: String,
    err_cache_size: usize,
    bucket: Option<Box<Bucket>>,
    s3_path: String,
    s3_state_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_dir: "tests/cache/".to_string(),
            state_path: "tests/state.json".to_string(),
            err_cache_size: 10,
            bucket: None,
            s3_path: "examples".to_string(),
            s3_state_path: "state.json".to_string(),
        }
    }
}

impl Config {
    fn from_env() -> Self {
        let defaults = Self::default();
        let cache_dir = env::var("CACHE_DIR").unwrap_or(defaults.cache_dir);
        let state_path = env::var("STATE_PATH").unwrap_or(defaults.state_path);
        let err_cache_size: usize = env::var("ERR_CACHE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.err_cache_size);
        let s3_path = env::var("S3_PATH").unwrap_or(defaults.s3_path);
        let s3_state_path = env::var("S3_STATE_PATH").unwrap_or(defaults.s3_state_path);
        let bucket = match (
            env::var("S3_ENDPOINT"),
            env::var("S3_BUCKET"),
            env::var("S3_ACCESS_KEY"),
            env::var("S3_SECRET_KEY"),
        ) {
            (Ok(endpoint), Ok(bucket_name), Ok(access_key), Ok(secret_key)) => {
                let region = Region::Custom { region: String::new(), endpoint };
                let credentials = Credentials::new(
                    Some(&access_key), Some(&secret_key),
                    None, None, None,
                ).expect("failed to create S3 credentials");

                Some(
                    Bucket::new(&bucket_name, region, credentials)
                        .expect("failed to connect to S3 bucket")
                        .with_path_style()
                )
            }
            _ => None
        };

        Self { cache_dir, state_path, err_cache_size, bucket, s3_path, s3_state_path }
    }
}

fn empty_state(cfg: &Config) -> State {
    let mut state = State::new();
    let files_list: Vec<String> = fs::read_dir(&cfg.cache_dir).unwrap()
        .map(|entry| entry
            .map(|file|
                file.path().file_name().unwrap().to_str().unwrap().to_string()
            )
            .unwrap()
        )
        .collect();
    state.insert("Error".to_string(), files_list);
    state
}

fn load_state(config: &Config) -> State {
    let path = Path::new(&config.state_path);

    if path.exists() {
        let content = fs::read_to_string(path).unwrap();
        return serde_json::from_str(&content).unwrap_or_default();
    }

    if let Some(bucket) = &config.bucket {

        if let Ok(response) = bucket.get_object(&config.s3_state_path) {
            let mut content = String::from_utf8(response.bytes().to_vec()).unwrap();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            if content.is_empty() {
                let state = empty_state(&config);
                content = serde_json::to_string_pretty(&state).unwrap();
            }
            println!("here");
            fs::write(path, &content).unwrap();
            return serde_json::from_str(&content).unwrap_or_default();
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let state = empty_state(&config);
    let content = serde_json::to_string_pretty(&state).unwrap();
    fs::write(path, &content).unwrap();
    state
}

fn save_state(config: &Config, state: &State) {
    let content = serde_json::to_string_pretty(state).unwrap();
    let state_path = Path::new(&config.state_path);
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let stem = state_path.file_stem().unwrap().to_string_lossy();
    let out_path = state_path.with_file_name(format!("{stem}.{timestamp}.json"));

    fs::write(&out_path, content).unwrap();
    println!("state saved to {}", out_path.display());
}

fn download_file(config: &Config, filename: &str) -> Option<PathBuf> {
    let local_path = PathBuf::from(&config.cache_dir).join(filename);
    if local_path.exists() {
        return Some(local_path);
    }

    let bucket = config.bucket.as_ref()?;
    let s3_key = if config.s3_path.is_empty() {
        filename.to_string()
    } else {
        format!("{}/{}", config.s3_path, filename)
    };

    match bucket.get_object(&s3_key) {
        Ok(response) => {
            fs::write(&local_path, response.bytes()).unwrap();
            println!("  downloaded {}", filename);
            Some(local_path)
        }
        Err(e) => {
            println!("  failed to download {}: {}", filename, e);
            None
        }
    }
}

fn test_file(path: &Path) -> Option<String> {
    let filename = path.file_name().unwrap().to_string_lossy();
    let data = fs::read(path).unwrap();

    let mut sniffer = brutal_csv::CsvSniffer::new(None);
    sniffer.process(&mut data.as_slice());

    let debug = sniffer.debug();
    let mut dialects = sniffer.dialects();

    if dialects.is_empty() {
        println!("{}: no valid dialects found", filename.red());
        for (dialect, error) in &debug {
            if error.contains("Only one column found") {
                println!("  {dialect} -> {}", "Only one column found".red());
            } else {
                println!("  {dialect}\n\t{error}");
            }
        }
        None
    } else {
        dialects.sort();
        let best = dialects.last().unwrap();
        let desc = best.describe();
        println!("{}: {}", filename.green(), desc);
        Some(desc)
    }
}

#[test]
fn generic_csv() {
    dotenv().ok();
    let config = Config::from_env();
    println!("{:?}", config);
    fs::create_dir_all(&config.cache_dir);
    let state = load_state(&config);
    println!("{:#?}", state);
    let mut test_files: Vec<(PathBuf, String)> = Vec::new();

    // Download 1 file per dialect (regression testing)
    for (dialect, files) in &state {
        if dialect == "Error" { continue; }
        if let Some(filename) = files.first() {
            if let Some(path) = download_file(&config, filename) {
                test_files.push((path, dialect.clone()));
            }
        }
    }

    // Download ERR_CACHE_SIZE error files
    if let Some(error_files) = state.get("Error") {
        for filename in error_files.iter().take(config.err_cache_size) {
            if let Some(path) = download_file(&config, filename) {
                // skips download if file exists in cache
                test_files.push((path, "Error".to_string()));
            }
        }
    }

    if test_files.is_empty() {
        println!("no test files in state (state.json is empty or missing)");
        return;
    }

    // Build new state: start with untested files from old state
    let tested_filenames: Vec<String> = test_files.iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    let mut new_state: State = HashMap::new();
    for (key, files) in &state {
        for filename in files {
            if !tested_filenames.contains(filename) {
                new_state.entry(key.clone()).or_default().push(filename.clone());
            }
        }
    }

    // Run tests
    let mut regressions = 0;
    for (path, origin_key) in &test_files {
        let filename = path.file_name().unwrap().to_string_lossy().to_string();

        match test_file(&path) {
            Some(dialect_key) => {
                if origin_key == "Error" {
                    println!("  {} {}", "IMPROVEMENT:".green(), filename);
                }
                new_state.entry(dialect_key).or_default().push(filename);
            }
            None => {
                if origin_key != "Error" {
                    println!("  {} {} was {}", "REGRESSION:".red(), filename, origin_key);
                    regressions += 1;
                }
                new_state.entry("Error".to_string()).or_default().push(filename);
            }
        }
    }

    save_state(&config, &new_state);

    assert_eq!(regressions, 0, "{regressions} regression(s) detected");
}
