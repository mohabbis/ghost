//! Confidence scoring for a click/type target.

use crate::core::events::ElementInfo;

pub fn score(element: Option<&ElementInfo>) -> f32 {
    let Some(el) = element else {
        return 0.25;
    };

    let has_identifier = el.identifier.as_deref().is_some_and(|s| !s.is_empty());
    let has_name = !el.name.trim().is_empty();
    let has_role = !el.role.trim().is_empty();
    let has_app = !el.app.trim().is_empty();

    if has_identifier && has_app && has_role && has_name {
        0.95
    } else if has_name && has_role && has_app {
        0.80
    } else if has_app {
        0.55
    } else {
        0.25
    }
}
