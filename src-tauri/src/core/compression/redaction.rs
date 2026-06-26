//! Redaction policy for compressed typing.

use crate::core::events::ElementInfo;
use crate::core::guard::is_sensitive_element;

pub const REDACT_BY_DEFAULT: bool = true;

pub fn is_secure_target(element: Option<&ElementInfo>) -> bool {
    element.is_some_and(is_sensitive_element)
}

pub fn resolve(typed: &str, secure_field: bool, keep_text: bool) -> (bool, Option<String>) {
    if keep_text && !secure_field {
        (false, Some(typed.to_string()))
    } else {
        (true, None)
    }
}
