//! Ignored micro-benchmark for the per-token macro context query
//! (`macro_context_at`), the indexed replacement for the removed
//! per-token text gate.
//!
//! `collect_file` consults the macro context for every name-like token before
//! falling back to plain syntax resolution. The old gate scanned the file
//! text backwards from each token offset (quadratic in file size); the
//! coverage index should make the per-token cost constant.
//!
//! Run with:
//!
//! ```text
//! cargo test -p ide --release -- --ignored --nocapture index_benchmarks
//! ```

use std::time::Instant;

use preproc_expand::context::macro_context_at;

use super::*;

fn context_scan_all_name_tokens(db: &dyn PreprocDb, text: &str) -> (std::time::Duration, usize) {
    let bytes = text.as_bytes();
    let mut total = std::time::Duration::ZERO;
    let mut token_count = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
            {
                i += 1;
            }
            let offset = TextSize::from(start as u32);
            let start_time = Instant::now();
            std::hint::black_box(macro_context_at(db, FileId::from_raw(0), offset));
            total += start_time.elapsed();
            token_count += 1;
        } else {
            i += 1;
        }
    }
    (total, token_count)
}

fn bench_context_text(modules: u32) -> String {
    (0..modules)
        .map(|name| {
            format!(
                "module m{name}(input logic clk);\n  logic a{name}, b{name};\n  assign a{name} = b{name} ^ clk;\n  always_ff @(posedge clk) b{name} <= a{name};\nendmodule\n\n"
            )
        })
        .collect()
}

#[test]
#[ignore]
fn index_benchmarks_macro_context_scales_with_offset() {
    let modules = [64u32, 128, 256, 512, 1024, 2048];
    println!("\n== B1: per-token macro context cost vs file size (release) ==");
    println!("{:<10} {:<10} {:<12} {:<16}", "modules", "bytes", "tokens", "total");
    for count in modules {
        let text = bench_context_text(count);
        let (host, file_id) = crate::test_utils::setup_with_path(&text, "/bench.sv");
        let db = host.raw_db();
        // Warm the coverage query once; the scan measures lookup cost only.
        std::hint::black_box(macro_context_at(db, file_id, TextSize::from(0)));
        let (total, tokens) = context_scan_all_name_tokens(db, &text);
        let per_token =
            std::time::Duration::from_nanos(total.as_nanos() as u64 / tokens.max(1) as u64);
        println!(
            "{:<10} {:<10} {:<12} {:<12?} {per_token:?}/tok",
            count,
            text.len(),
            tokens,
            total
        );
    }
}
