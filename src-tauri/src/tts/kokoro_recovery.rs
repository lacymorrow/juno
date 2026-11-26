//! Getting Kokoro speaking again after its engine breaks.
//!
//! An engine can break for good. candle's Metal kernels `.unwrap()` the result
//! of compiling a shader while holding a write lock, so one compile failure
//! (macOS's AMD shader compiler crashing, LAC-9074) panics, poisons the lock,
//! and every later sentence on that engine fails with `poisoned lock`. Only a
//! new engine has a new lock, so a broken one is thrown away and the next is
//! built in its place:
//!
//! - A panic or a compute error means the engine is broken. Anything else
//!   (unknown voice, a voice file that could not be fetched) is the request's
//!   fault and keeps the engine.
//! - The sentence that hit the broken engine is retried on the new one.
//! - After [`MAX_GPU_FAILURES`] broken GPU engines, the GPU is left alone
//!   until relaunch and Kokoro speaks on the CPU. A GPU that cannot compile a
//!   kernel fails the same way every time.
//! - A sentence that breaks the CPU engine too was never the GPU's fault, so
//!   it costs the GPU nothing.
//! - Nothing is remembered about a broken CPU engine. It is rebuilt every time.

use super::kokoro::ModelCell;
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::warn;

/// Broken GPU engines before Kokoro moves to the CPU. Two, so a one-off GPU
/// error gets a fresh engine on the same GPU first.
pub const MAX_GPU_FAILURES: usize = 2;

/// Enough to use up the GPU's chances and still try the CPU.
const ATTEMPTS: usize = MAX_GPU_FAILURES + 1;

/// A model and the hardware it was built for.
pub struct Engine<T> {
    pub model: T,
    pub on_gpu: bool,
}

/// Why a job on an engine failed.
#[derive(Debug)]
pub enum Fault {
    /// The engine is broken and must be replaced.
    Broken(String),
    /// This request failed; the engine is fine.
    Request(String),
}

/// How the GPU has been doing since launch.
#[derive(Default)]
pub struct GpuHealth {
    failures: AtomicUsize,
}

impl GpuHealth {
    pub const fn new() -> Self {
        Self {
            failures: AtomicUsize::new(0),
        }
    }

    pub fn usable(&self) -> bool {
        self.failures.load(Ordering::SeqCst) < MAX_GPU_FAILURES
    }

    pub fn failed(&self) {
        let failures = self.failures.fetch_add(1, Ordering::SeqCst) + 1;
        if failures == MAX_GPU_FAILURES {
            warn!("[Kokoro] The GPU failed {failures} times; using the CPU until Juno relaunches");
        }
    }

    /// Take back failures that turned out not to be the GPU's.
    fn forgive(&self, failures: usize) {
        self.failures.fetch_sub(failures, Ordering::SeqCst);
    }
}

fn panic_text(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message")
}

/// Run engine code, turning a panic into [`Fault::Broken`] with its message.
/// The message is the only record of the first failure: every later one just
/// says the lock is poisoned.
pub fn guard<R>(job: impl FnOnce() -> Result<R, Fault>) -> Result<R, Fault> {
    catch_unwind(AssertUnwindSafe(job)).unwrap_or_else(|payload| {
        Err(Fault::Broken(format!(
            "panicked: {}",
            panic_text(payload.as_ref())
        )))
    })
}

/// Run `job` on the cell's engine, replacing the engine and trying again if it
/// turns out to be broken. Blocking: call it from a blocking thread.
///
/// `load` builds a new engine and is expected to consult `gpu`.
pub fn with_recovery<T, R>(
    cell: &ModelCell<Arc<Engine<T>>>,
    gpu: &GpuHealth,
    load: impl Fn() -> Result<Arc<Engine<T>>, String>,
    job: impl Fn(&T) -> Result<R, Fault>,
) -> Result<R, String> {
    let mut last = String::new();
    let mut blamed_on_gpu = 0;
    let mut broke_the_cpu = false;
    for attempt in 1..=ATTEMPTS {
        let engine = cell.get_or_load(&load)?;
        match guard(|| job(&engine.model)) {
            Ok(value) => return Ok(value),
            Err(Fault::Request(e)) => return Err(e),
            Err(Fault::Broken(e)) => {
                warn!(
                    "[Kokoro] Engine on the {} broke (attempt {attempt} of {ATTEMPTS}): {e}",
                    if engine.on_gpu { "GPU" } else { "CPU" }
                );
                // Several sentences can fail on one broken engine. Only the
                // one that actually removes it counts against the GPU.
                if cell.discard(|current| Arc::ptr_eq(current, &engine)) && engine.on_gpu {
                    gpu.failed();
                    blamed_on_gpu += 1;
                }
                broke_the_cpu |= !engine.on_gpu;
                last = e;
            }
        }
    }
    if broke_the_cpu {
        gpu.forgive(blamed_on_gpu);
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Barrier, Mutex, RwLock};

    /// An engine shaped like the real failure: its first GPU job panics while
    /// holding a write lock, and every later job on it finds the lock poisoned.
    struct Fake {
        kernels: RwLock<()>,
        compiles: bool,
    }

    impl Fake {
        fn speak(&self) -> Result<(), Fault> {
            let _kernels = self
                .kernels
                .write()
                .map_err(|e| Fault::Broken(format!("poisoned lock: {e}")))?;
            assert!(self.compiles, "shader compiler crashed");
            Ok(())
        }
    }

    /// Builds engines the way `load_kokoro` does, recording where each went.
    struct Factory {
        cell: ModelCell<Arc<Engine<Fake>>>,
        gpu: GpuHealth,
        built_on_gpu: Mutex<Vec<bool>>,
        /// Whether the nth engine built can compile its kernels.
        compiles: fn(usize, bool) -> bool,
    }

    impl Factory {
        fn new(compiles: fn(usize, bool) -> bool) -> Self {
            Self {
                cell: ModelCell::new(),
                gpu: GpuHealth::new(),
                built_on_gpu: Mutex::new(Vec::new()),
                compiles,
            }
        }

        fn load(&self) -> Result<Arc<Engine<Fake>>, String> {
            let on_gpu = self.gpu.usable();
            let mut built = self.built_on_gpu.lock().map_err(|e| e.to_string())?;
            let compiles = (self.compiles)(built.len(), on_gpu);
            built.push(on_gpu);
            Ok(Arc::new(Engine {
                model: Fake {
                    kernels: RwLock::new(()),
                    compiles,
                },
                on_gpu,
            }))
        }

        fn speak(&self) -> Result<(), String> {
            with_recovery(&self.cell, &self.gpu, || self.load(), Fake::speak)
        }

        fn built(&self) -> Vec<bool> {
            self.built_on_gpu
                .lock()
                .map(|b| b.clone())
                .unwrap_or_default()
        }
    }

    /// LAC-9074: a GPU that can never compile the kernel. The first sentence
    /// is still spoken, on the CPU, and the next reuses that engine.
    #[test]
    fn a_gpu_that_cannot_compile_ends_up_speaking_on_the_cpu() {
        let factory = Factory::new(|_, on_gpu| !on_gpu);
        assert_eq!(factory.speak(), Ok(()));
        assert_eq!(factory.speak(), Ok(()));
        assert_eq!(factory.built(), vec![true, true, false]);
    }

    #[test]
    fn a_one_off_failure_rebuilds_on_the_gpu() {
        let factory = Factory::new(|n, _| n > 0);
        assert_eq!(factory.speak(), Ok(()));
        assert_eq!(factory.built(), vec![true, true]);
        assert!(factory.gpu.usable());
    }

    /// Recovery is not a one-time thing: an engine that breaks after the GPU
    /// was already given up on is replaced as well.
    #[test]
    fn a_second_break_later_is_recovered_too() {
        // Engines 0 and 2 break.
        let factory = Factory::new(|n, _| n != 0 && n != 2);
        assert_eq!(factory.speak(), Ok(()));
        // The next sentence needs a new engine, and that one breaks too.
        factory.cell.release();
        assert_eq!(factory.speak(), Ok(()));
        assert_eq!(factory.built(), vec![true, true, true, false]);
    }

    /// A sentence no engine can say must not cost a healthy GPU its place.
    #[test]
    fn a_sentence_that_breaks_every_engine_does_not_blame_the_gpu() {
        let factory = Factory::new(|_, _| true);
        let result: Result<(), String> = with_recovery(
            &factory.cell,
            &factory.gpu,
            || factory.load(),
            |_| Err(Fault::Broken("shape mismatch".into())),
        );
        assert_eq!(result, Err("shape mismatch".to_string()));
        assert!(factory.gpu.usable());
        assert_eq!(factory.speak(), Ok(()));
        assert_eq!(factory.built(), vec![true, true, false, true]);
    }

    /// When every attempt breaks, it fails now and a new engine is built for
    /// the next sentence.
    #[test]
    fn an_engine_that_keeps_breaking_reports_why_and_is_rebuilt_next_time() {
        let factory = Factory::new(|n, _| n >= 3);
        let error = factory.speak().expect_err("every attempt broke");
        assert!(error.contains("shader compiler crashed"), "got {error}");
        assert!(!factory.cell.is_ready(), "the broken engine is gone");
        assert_eq!(factory.speak(), Ok(()));
        // The CPU broke too, so the GPU was not to blame and is tried again.
        assert_eq!(factory.built(), vec![true, true, false, true]);
    }

    #[test]
    fn a_refused_request_keeps_the_engine() {
        let factory = Factory::new(|_, _| true);
        let result: Result<(), String> = with_recovery(
            &factory.cell,
            &factory.gpu,
            || factory.load(),
            |_| Err(Fault::Request("unknown voice".into())),
        );
        assert_eq!(result, Err("unknown voice".to_string()));
        assert!(factory.cell.is_ready());
        assert_eq!(factory.built(), vec![true]);
    }

    /// The greeting log: several sentences hit the same engine at once. One
    /// broken engine is one strike, and every sentence gets spoken.
    #[test]
    fn sentences_failing_together_count_one_broken_engine_once() {
        let factory = Factory::new(|n, _| n > 0);
        assert!(factory.cell.begin_preload());
        assert!(factory.cell.finish_preload(factory.load()).is_ok());
        let together = Barrier::new(4);
        std::thread::scope(|scope| {
            let speakers: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        let first = std::sync::atomic::AtomicBool::new(true);
                        with_recovery(
                            &factory.cell,
                            &factory.gpu,
                            || factory.load(),
                            |model: &Fake| {
                                // All four hold the broken engine before any fails.
                                if first.swap(false, Ordering::SeqCst) {
                                    together.wait();
                                }
                                model.speak()
                            },
                        )
                    })
                })
                .collect();
            for speaker in speakers {
                assert!(matches!(speaker.join(), Ok(Ok(()))));
            }
        });
        assert_eq!(factory.built(), vec![true, true]);
        assert!(factory.gpu.usable());
    }
}
