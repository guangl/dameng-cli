//! A fixed worker pool with ordered results and no thread per item.
use anyhow::{Result, ensure};
use std::sync::Mutex;

pub fn map<T: Send, U: Send>(
    items: Vec<T>,
    workers: usize,
    operation: impl Fn(T) -> U + Sync,
) -> Result<Vec<U>> {
    ensure!(workers > 0, "Worker count must be positive");
    let count = workers.min(items.len());
    let queue = Mutex::new(items.into_iter().enumerate());
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(count);
        for _ in 0..count {
            let queue = &queue;
            let operation = &operation;
            handles.push(scope.spawn(move || {
                let mut completed = Vec::new();
                loop {
                    let Some((index, item)) = queue.lock().unwrap().next() else {
                        break;
                    };
                    completed.push((index, operation(item)));
                }
                completed
            }));
        }
        let mut results = Vec::new();
        let mut panicked = false;
        for handle in handles {
            match handle.join() {
                Ok(completed) => results.extend(completed),
                Err(_) => panicked = true,
            }
        }
        ensure!(!panicked, "Update check worker panicked");
        results.sort_unstable_by_key(|(index, _)| *index);
        Ok(results.into_iter().map(|(_, value)| value).collect())
    })
}
