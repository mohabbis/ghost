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

/// RAII guard that sets an environment variable for the duration of a test and
/// restores the prior value (or removes the key) on drop. Holds [`env_lock`] for
/// its entire lifetime, so no other lock-respecting test observes the mutation.
pub struct EnvVarGuard {
    key: String,
    previous: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    pub fn set(key: &str, value: &str) -> Self {
        let lock = env_lock();
        let previous = std::env::var(key).ok();
        // Safe: env_lock() serializes all env mutation for the guard's lifetime.
        unsafe { std::env::set_var(key, value) };
        Self {
            key: key.to_string(),
            previous,
            _lock: lock,
        }
    }

    pub fn remove(key: &str) -> Self {
        let lock = env_lock();
        let previous = std::env::var(key).ok();
        // Safe: env_lock() serializes all env mutation for the guard's lifetime.
        unsafe { std::env::remove_var(key) };
        Self {
            key: key.to_string(),
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // We already hold ENV_TEST_LOCK (via self._lock) for our whole lifetime,
        // so restore in place — re-locking here would deadlock.
        match &self.previous {
            // Safe: still holding env_lock() through self._lock.
            Some(value) => unsafe { std::env::set_var(&self.key, value) },
            // Safe: still holding env_lock() through self._lock.
            None => unsafe { std::env::remove_var(&self.key) },
        }
    }
}
