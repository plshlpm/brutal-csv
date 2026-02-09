//! Pollock Benchmark Integration Test
//!
//! Runs dialect detection against the standard Pollock benchmark files:
//! - tests/polluted_files/csv/       - Input CSV files
//! - tests/polluted_files/clean/     - Expected clean output
//! - tests/polluted_files/parameters/ - Expected dialect parameters
//!
//! Reference: https://github.com/HPI-Information-Systems/Pollock

mod common;

use common::{PollockBenchmark, default_pollock_dir};

/// Run full Pollock benchmark with detailed output
#[test]
fn pollock_benchmark_verbose() {
    // https://github.com/HPI-Information-Systems/Pollock.git
    let pollock_dir = default_pollock_dir();

    if !pollock_dir.exists() {
        println!("Pollock benchmark directory not found at {:?}", pollock_dir);
        return;
    }

    let benchmark = PollockBenchmark::new(&pollock_dir);
    let stats = benchmark.run(true);

    stats.print_summary();
}

// /// Run benchmark silently and check minimum success rate
// #[test]
// fn pollock_benchmark_threshold() {
//     let pollock_dir = default_pollock_dir();
//
//     if !pollock_dir.exists() {
//         return;
//     }
//
//     let benchmark = PollockBenchmark::new(&pollock_dir);
//     let (_, stats) = benchmark.run_collect();
//
//     // Detection rate should be reasonable
//     let min_detection_rate = 70.0;
//     assert!(
//         stats.detection_rate() >= min_detection_rate,
//         "Detection rate {:.1}% below threshold {:.1}%",
//         stats.detection_rate(),
//         min_detection_rate
//     );
// }

// /// Ensure no panics on any test file
// #[test]
// fn pollock_benchmark_no_panic() {
//     let pollock_dir = default_pollock_dir();
//
//     if !pollock_dir.exists() {
//         return;
//     }
//
//     let benchmark = PollockBenchmark::new(&pollock_dir);
//     let (results, stats) = benchmark.run_collect();
//
//     // If we got here without panic, the test passes
//     println!("Processed {} files without panic", results.len());
//     println!(
//         "Success: {}, Mismatch: {}, NoDialect: {}, Errors: {}",
//         stats.success, stats.mismatch, stats.no_dialect, stats.errors
//     );
// }
