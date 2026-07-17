//! Shared helpers for unit tests. Not compiled into release builds.

use std::sync::{Mutex, MutexGuard};

/// Serializes `std::env` mutations. Rust's environment is process-global; parallel
/// tests that call `set_var` / `remove_var` without this lock can corrupt heap
/// state on teardown (observed as intermittent SIGABRT/SIGSEGV after tests pass).
static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the process-wide env serialization lock for the caller's scope.
///
/// Read-only tests that assert on the *default* (unset) state of an env var
/// [`EnvVarGuard`] can toggle must hold this for their duration too — otherwise a
/// concurrent test that sets the var leaks its value into the reader's window.
/// Hold the returned guard for as long as the env-sensitive assertions run.
pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// RAII guard that sets/removes environment variables for the duration of a
/// test and restores the prior values on drop. Holds [`env_lock`] for its
/// entire lifetime, so no other lock-respecting test observes the mutation.
///
/// Because the guard holds the process-wide lock for its whole lifetime,
/// constructing a **second** guard while one is alive self-deadlocks (the
/// mutex is not reentrant). A test that needs to touch several variables must
/// use one [`EnvVarGuard::apply`] call instead of stacking guards.
pub struct EnvVarGuard {
    /// (key, previous value) pairs, restored in reverse order on drop.
    saved: Vec<(String, Option<String>)>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    pub fn set(key: &str, value: &str) -> Self {
        Self::apply(&[(key, Some(value))])
    }

    pub fn remove(key: &str) -> Self {
        Self::apply(&[(key, None)])
    }

    /// Set (`Some`) or remove (`None`) several variables under a single lock
    /// acquisition. This is the only safe way to touch more than one variable
    /// in a test scope — two live guards deadlock on the non-reentrant lock.
    pub fn apply(ops: &[(&str, Option<&str>)]) -> Self {
        let lock = env_lock();
        let mut saved = Vec::with_capacity(ops.len());
        for (key, value) in ops {
            let previous = std::env::var(key).ok();
            // Safe: env_lock() serializes all env mutation for the guard's lifetime.
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
            saved.push((key.to_string(), previous));
        }
        Self { saved, _lock: lock }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // We already hold ENV_TEST_LOCK (via self._lock) for our whole lifetime,
        // so restore in place — re-locking here would deadlock.
        for (key, previous) in self.saved.iter().rev() {
            match previous {
                // Safe: still holding env_lock() through self._lock.
                Some(value) => unsafe { std::env::set_var(key, value) },
                // Safe: still holding env_lock() through self._lock.
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}
