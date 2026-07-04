use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use terminal_core::Terminal;
use terminal_fixtures::{first_snapshot_difference, m4_synthetic_workload, snapshot_terminal};

fn m4_public_proof_benchmarks(criterion: &mut Criterion) {
    let input = m4_synthetic_workload(2_000);
    let expected_snapshot = snapshot_from_input(&input);
    let mut snapshot_terminal_state = Terminal::with_default_dimensions();
    snapshot_terminal_state.advance_bytes(&input);
    let mut group = criterion.benchmark_group("m4_public_proof");
    group.throughput(Throughput::Bytes(input.len() as u64));

    group.bench_function(BenchmarkId::new("byte_ingest", "2000_lines"), |bencher| {
        bencher.iter(|| {
            let mut terminal = Terminal::with_default_dimensions();
            terminal.advance_bytes(black_box(&input));
            black_box(terminal.scrollback_len());
        });
    });

    group.bench_function(
        BenchmarkId::new("snapshot_generation", "2000_lines"),
        |bencher| {
            bencher.iter(|| {
                black_box(snapshot_terminal(&mut snapshot_terminal_state));
            });
        },
    );

    group.bench_function(BenchmarkId::new("replay", "2000_lines"), |bencher| {
        bencher.iter(|| {
            let actual = snapshot_from_input(black_box(&input));
            black_box(first_snapshot_difference(&expected_snapshot, &actual));
        });
    });

    group.bench_function(
        BenchmarkId::new("snapshot_comparison", "2000_lines"),
        |bencher| {
            bencher.iter(|| {
                black_box(first_snapshot_difference(
                    &expected_snapshot,
                    &expected_snapshot,
                ));
            });
        },
    );

    group.finish();
}

fn snapshot_from_input(input: &[u8]) -> terminal_fixtures::TerminalSnapshot {
    let mut terminal = Terminal::with_default_dimensions();
    terminal.advance_bytes(input);
    snapshot_terminal(&mut terminal)
}

criterion_group!(benches, m4_public_proof_benchmarks);
criterion_main!(benches);
