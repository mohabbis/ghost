//! Scheduling gates for enterprise workflows.
//!
//! Scheduling is opt-in metadata. It does not create background observation or
//! unattended execution; a due schedule only indicates that planning may be
//! offered to the user for explicit review.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSchedule {
    pub schedule_id: String,
    pub playbook_id: String,
    pub next_review_at: DateTime<Utc>,
    pub enabled: bool,
}

impl ReviewSchedule {
    pub fn is_due(&self, now: DateTime<Utc>) -> bool {
        self.enabled && self.next_review_at <= now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn disabled_schedule_is_never_due() {
        let schedule = ReviewSchedule {
            schedule_id: "sched-1".into(),
            playbook_id: "pb-1".into(),
            next_review_at: Utc::now() - Duration::minutes(5),
            enabled: false,
        };
        assert!(!schedule.is_due(Utc::now()));
    }
}
