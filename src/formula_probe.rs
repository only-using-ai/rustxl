use std::cell::RefCell;
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Instant;

use super::Spreadsheet;

pub(super) const CHAIN_DEPTH: usize = 16;
const EXPECTED_VALUE: &str = "65536";
type BodyCounts = HashMap<(usize, usize), usize>;

thread_local! {
    static BODIES: RefCell<Option<BodyCounts>> = const { RefCell::new(None) };
}

pub(super) fn record_body(row: usize, col: usize) {
    BODIES.with_borrow_mut(|bodies| {
        if let Some(bodies) = bodies {
            *bodies.entry((row, col)).or_default() += 1;
        }
    });
}

pub(super) fn count_bodies<T>(evaluate: impl FnOnce() -> T) -> (T, BodyCounts) {
    BODIES.with_borrow_mut(|bodies| {
        assert!(bodies.is_none());
        *bodies = Some(HashMap::new());
    });
    let result = evaluate();
    let counts = BODIES.with_borrow_mut(|bodies| bodies.take().unwrap());
    (result, counts)
}

pub(super) fn depth_16_chain() -> Spreadsheet {
    let mut sheet = Spreadsheet::new();
    sheet.set_cell(0, 0, "1".into());
    for row in 1..=CHAIN_DEPTH {
        sheet.set_cell(row, 0, format!("=SUM(A{row},A{row})"));
    }
    sheet
}

#[test]
#[ignore = "run in release mode for the formula-cache comparison"]
fn release_depth_16() {
    const WARMUPS: usize = 5;
    const SAMPLES: usize = 31;
    let sheet = depth_16_chain();
    let evaluate = || black_box(&sheet).evaluate_cell(CHAIN_DEPTH, 0);

    for _ in 0..WARMUPS {
        assert_eq!(black_box(evaluate()), EXPECTED_VALUE);
    }

    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let value = black_box(evaluate());
        samples.push(start.elapsed());
        assert_eq!(value, EXPECTED_VALUE);
    }
    samples.sort_unstable();

    let (value, bodies) = count_bodies(evaluate);
    assert_eq!(value, EXPECTED_VALUE);
    let total: usize = bodies.values().sum();
    println!(
        "depth={CHAIN_DEPTH} warmups={WARMUPS} samples={SAMPLES} median_ns={} value={value} total_bodies={total} distinct_coordinates={}",
        samples[SAMPLES / 2].as_nanos(),
        bodies.len(),
    );
}
