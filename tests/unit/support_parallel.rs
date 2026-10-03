use dameng_cli::support::parallel;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

#[test]
fn worker_pool_bounds_concurrency_and_preserves_order() {
    let active = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let result = parallel::map((0..30).collect(), 3, |item| {
        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(running, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(5));
        active.fetch_sub(1, Ordering::SeqCst);
        item * 2
    })
    .unwrap();
    assert_eq!(result, (0..30).map(|item| item * 2).collect::<Vec<_>>());
    assert!((2..=3).contains(&peak.load(Ordering::SeqCst)));
    assert!(parallel::map(vec![1], 0, |item| item).is_err());
    assert!(parallel::map(vec![1], 1, |_| panic!("probe")).is_err());
    assert!(
        parallel::map(Vec::<u8>::new(), 4, |item| item)
            .unwrap()
            .is_empty()
    );
}
