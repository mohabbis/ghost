//! Shared helpers for unit tests. Not compiled into release builds.

use std::sync::Mutex;

/// Serializes `std::env` mutations. Rust's environment is process-global; parallel
/// tests that call `set_var` / `remove_var` without this lock can corrupt heap
/// state on teardown (observed as intermittent SIGABRT/SIGSEGV after tests pass).
static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that sets an environment variable for the duration of a test and
/// restores the prior value (or removes the key) on drop.
pub struct EnvVarGuard {
    key: String,
    previous: Option<String>,
}

impl EnvVarGuard {
    pub fn set(key: &str, value: &str) -> Self {
        let _lock = ENV_TEST_LOCK.lock().expect("env test lock poisoned");
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            previous,
        }
    }

    pub fn remove(key: &str) -> Self {
        let _lock = ENV_TEST_LOCK.lock().expect("env test lock poisoned");
        let previous = std::env::var(key).ok();
        std::env::remove_var(key);
        Self {
            key: key.to_string(),
            previous,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        let _lock = ENV_TEST_LOCK.lock().expect("env test lock poisoned");
        match &self.previous {
            Some(value) => std::env::set_var(&self.key, value),
            None => std::env::remove_var(&self.key),
        }
    }
}
