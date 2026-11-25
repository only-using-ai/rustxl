use std::{cell::Cell, fmt::Debug, hint::black_box, time::Instant};

use ratatui::{Terminal, backend::TestBackend};

use crate::{input::redraw_if_needed, spreadsheet::Spreadsheet};

#[derive(Clone, Copy)]
pub(crate) enum Work {
    FindCandidate,
    HighlightComparison,
    SelectionCandidate,
    Draw,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct WorkCounts {
    pub(crate) find_candidates: usize,
    pub(crate) highlight_comparisons: usize,
    pub(crate) selection_candidates: usize,
    pub(crate) draws: usize,
}

thread_local! {
    static WORK: Cell<Option<WorkCounts>> = const { Cell::new(None) };
}

pub(crate) fn record(work: Work) {
    WORK.with(|counts| {
        if let Some(mut captured) = counts.get() {
            match work {
                Work::FindCandidate => captured.find_candidates += 1,
                Work::HighlightComparison => captured.highlight_comparisons += 1,
                Work::SelectionCandidate => captured.selection_candidates += 1,
                Work::Draw => captured.draws += 1,
            }
            counts.set(Some(captured));
        }
    });
}

pub(crate) fn count_work<T>(run: impl FnOnce() -> T) -> (T, WorkCounts) {
    WORK.with(|counts| assert!(counts.replace(Some(WorkCounts::default())).is_none()));
    let result = run();
    let counts = WORK.with(|counts| counts.take().unwrap());
    (result, counts)
}

fn measure<T: PartialEq + Debug>(name: &str, mut run: impl FnMut() -> T, expected: T) {
    const WARMUPS: usize = 5;
    const SAMPLES: usize = 31;
    for _ in 0..WARMUPS {
        assert_eq!(black_box(run()), expected);
    }

    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let result = black_box(run());
        samples.push(start.elapsed());
        assert_eq!(result, expected);
    }
    samples.sort_unstable();

    let (result, counts) = count_work(run);
    assert_eq!(result, expected);
    println!(
        "{name} warmups={WARMUPS} samples={SAMPLES} median_ns={} result={result:?} find_candidates={} highlight_comparisons={} selection_candidates={} draws={}",
        samples[SAMPLES / 2].as_nanos(),
        counts.find_candidates,
        counts.highlight_comparisons,
        counts.selection_candidates,
        counts.draws,
    );
}

#[test]
#[ignore = "run in release mode for the sparse UI comparison"]
fn release_sparse_ui() {
    let mut sparse = Spreadsheet::new();
    sparse.num_rows = 1000;
    sparse.num_cols = 1000;
    sparse.set_cell(997, 997, "7".into());
    sparse.enter_find_mode();
    sparse.find_query = "7".into();
    measure(
        "sparse_find_1000x1000",
        || {
            black_box(&mut sparse).update_find_matches();
            sparse.find_matches.len()
        },
        1,
    );

    sparse.exit_find_mode();
    sparse.selection_anchor = Some((0, 0));
    sparse.cursor_row = 999;
    sparse.cursor_col = 999;
    measure(
        "sparse_stats_1000x1000",
        || black_box(&sparse).get_selection_stats(),
        Some((1, 1, 1, 7.0)),
    );

    let mut many = Spreadsheet::new();
    many.num_rows = 1000;
    many.num_cols = 1000;
    for row in 0..100 {
        for col in 0..100 {
            many.set_cell(row, col, "match".into());
        }
    }
    many.enter_find_mode();
    many.find_query = "match".into();
    measure(
        "many_find_10000_matches",
        || {
            black_box(&mut many).update_find_matches();
            many.find_matches.len()
        },
        10_000,
    );
    measure(
        "many_highlights_1200_checks",
        || {
            let many = black_box(&many);
            let mut matches = 0;
            for row in 85..115 {
                for col in 80..120 {
                    matches += usize::from(many.is_find_match(row, col));
                }
            }
            matches
        },
        300,
    );

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut sheet = Spreadsheet::new();
    sheet.set_cell(0, 0, "7".into());
    measure(
        "initial_frame_and_20_idle_polls",
        || {
            let mut redraw = true;
            for _ in 0..=20 {
                redraw_if_needed(&mut terminal, &mut sheet, &mut redraw).unwrap();
            }
        },
        (),
    );
}
