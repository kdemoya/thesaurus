//! Pure state transitions shared by the opt-in review queue and its unit tests.
//!
//! Run without the web runtime:
//! rustc --edition 2024 --test apps/ui/src/bragbook_review_state.rs -o /tmp/thesaurus-review-state-tests
//! /tmp/thesaurus-review-state-tests

use std::collections::HashMap;

/// Only editable content belongs in reconciliation. A server acknowledgment can
/// change the version while leaving these fields identical to the local draft.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DraftContent<'a> {
    pub title: &'a str,
    pub contribution: &'a str,
    pub impact: &'a str,
    pub date: &'a str,
}

impl DraftContent<'_> {
    fn normalized(self) -> Self {
        Self {
            title: self.title.trim(),
            contribution: self.contribution.trim(),
            impact: self.impact.trim(),
            // The API validates an exact canonical date; it does not trim it.
            date: self.date,
        }
    }
}

pub(crate) trait ReviewDraft: Clone {
    fn content(&self) -> DraftContent<'_>;
}

/// Accept the current server baseline only when it already contains all edits.
/// A differing draft retains its original version, allowing the UI to present
/// a conflict rather than silently rebase it onto somebody else's changes.
pub(crate) fn reconcile<D: ReviewDraft>(draft: D, saved: &D) -> D {
    // Match the service's normalization before comparing a draft with an SSE
    // refresh that can arrive before its own successful PATCH acknowledgment.
    if draft.content().normalized() == saved.content().normalized() {
        saved.clone()
    } else {
        draft
    }
}

/// An in-flight request owns its submitted snapshot, never edits made later.
/// This remains valid when the editor that submitted the request has unmounted.
pub(crate) fn clear_submitted<D: PartialEq>(
    drafts: &mut HashMap<String, D>,
    entry_id: &str,
    submitted: &D,
) -> bool {
    if drafts.get(entry_id) == Some(submitted) {
        drafts.remove(entry_id);
        true
    } else {
        false
    }
}

/// Keep the active candidate through background insertion/reordering. If it was
/// reviewed away, its former position selects the next candidate, or the last
/// remaining candidate when the page has shortened. Empty pages use index zero.
pub(crate) fn queue_index<T: AsRef<str>>(
    active_id: Option<&str>,
    item_ids: &[T],
    fallback_index: usize,
) -> usize {
    active_id
        .and_then(|id| item_ids.iter().position(|item| item.as_ref() == id))
        .unwrap_or(fallback_index)
        .min(item_ids.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Draft {
        version: i64,
        title: String,
        contribution: String,
        impact: String,
        date: String,
    }

    impl ReviewDraft for Draft {
        fn content(&self) -> DraftContent<'_> {
            DraftContent {
                title: &self.title,
                contribution: &self.contribution,
                impact: &self.impact,
                date: &self.date,
            }
        }
    }

    fn draft() -> Draft {
        Draft {
            version: 1,
            title: "Documented import recovery".into(),
            contribution: "Wrote and tested the recovery steps.".into(),
            impact: "Teammates can resume an interrupted import.".into(),
            date: "2026-10-29".into(),
        }
    }

    #[test]
    fn new_server_version_before_patch_response_acknowledges_matching_edits() {
        let submitted = draft();
        let saved = Draft {
            version: 2,
            ..submitted.clone()
        };

        // A watermark refresh can reconstruct the editor before PATCH resolves.
        let restored = reconcile(submitted.clone(), &saved);
        assert_eq!(restored, saved);
        assert_ne!(restored.version, submitted.version);
        // Subsequent reconstruction remains clean instead of reviving conflict.
        assert_eq!(reconcile(restored, &saved), saved);
    }

    #[test]
    fn server_trim_before_patch_response_acknowledges_all_submitted_text_fields() {
        let saved = Draft {
            version: 2,
            ..draft()
        };
        let submitted = Draft {
            title: format!(" \t{}\n", saved.title),
            contribution: format!("\n{}  ", saved.contribution),
            impact: format!("\t{}\n ", saved.impact),
            ..draft()
        };

        // The refreshed version contains exactly what the service saves, even
        // though it no longer equals the literal request's padded strings.
        let restored = reconcile(submitted, &saved);
        assert_eq!(restored, saved);
        assert_eq!(reconcile(restored, &saved), saved);
    }

    #[test]
    fn normalization_preserves_internal_text_edits_and_noncanonical_dates() {
        let saved = Draft {
            version: 2,
            ..draft()
        };
        let changed_text = Draft {
            title: "Documented  import recovery".into(),
            ..draft()
        };
        let changed_date = Draft {
            date: " 2026-10-29 ".into(),
            ..draft()
        };

        assert_eq!(reconcile(changed_text.clone(), &saved), changed_text);
        assert_eq!(reconcile(changed_date.clone(), &saved), changed_date);
    }

    #[test]
    fn any_differing_editable_field_preserves_the_draft_and_its_conflicting_version() {
        let local = draft();
        for field in ["title", "contribution", "impact", "date"] {
            let mut saved = Draft {
                version: 2,
                ..local.clone()
            };
            match field {
                "title" => saved.title = "Another title".into(),
                "contribution" => saved.contribution = "Another contributor's wording".into(),
                "impact" => saved.impact = "A different outcome".into(),
                "date" => saved.date = "2026-10-30".into(),
                _ => unreachable!(),
            }
            assert_eq!(reconcile(local.clone(), &saved), local, "{field}");
        }
    }

    #[test]
    fn successful_request_removes_only_its_exact_submitted_draft() {
        let submitted = draft();
        let other = Draft {
            title: "Another accomplishment".into(),
            ..draft()
        };
        let mut drafts = HashMap::from([
            ("entry-a".into(), submitted.clone()),
            ("entry-b".into(), other.clone()),
        ]);
        assert!(clear_submitted(&mut drafts, "entry-a", &submitted));
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts.get("entry-b"), Some(&other));
        assert!(!clear_submitted(&mut drafts, "entry-a", &submitted));
    }

    #[test]
    fn late_response_preserves_edits_made_after_navigation_and_return() {
        let submitted = draft();
        let newer = Draft {
            contribution: "Wrote, tested and shared the recovery steps.".into(),
            ..submitted.clone()
        };
        let mut drafts = HashMap::from([("entry-a".into(), newer.clone())]);
        assert!(!clear_submitted(&mut drafts, "entry-a", &submitted));
        assert_eq!(drafts.get("entry-a"), Some(&newer));
    }

    #[test]
    fn late_response_cannot_remove_a_draft_based_on_a_newer_server_version() {
        let submitted = draft();
        let newer = Draft {
            version: 3,
            ..submitted.clone()
        };
        let mut drafts = HashMap::from([("entry-a".into(), newer.clone())]);
        assert!(!clear_submitted(&mut drafts, "entry-a", &submitted));
        assert_eq!(drafts.get("entry-a"), Some(&newer));
    }

    #[test]
    fn refreshed_queue_keeps_the_active_entry_after_insertion_or_reordering() {
        assert_eq!(queue_index(Some("b"), &["new", "a", "b", "c"], 1), 2);
        assert_eq!(queue_index(Some("b"), &["b", "c", "a"], 1), 0);
    }

    #[test]
    fn reviewed_entry_advances_to_its_successor_or_last_remaining_candidate() {
        assert_eq!(queue_index(Some("b"), &["a", "c", "d"], 1), 1);
        assert_eq!(queue_index(Some("d"), &["a", "b", "c"], 3), 2);
        assert_eq!(queue_index(Some("only"), &[] as &[&str], 0), 0);
    }

    #[test]
    fn page_navigation_without_an_active_id_uses_a_bounded_requested_position() {
        let full_page: Vec<String> = (0..25).map(|i| format!("entry-{i}")).collect();
        assert_eq!(queue_index(None, &full_page, 0), 0);
        assert_eq!(queue_index(None, &full_page, 24), 24);
        assert_eq!(queue_index(None, &["one", "two"], 24), 1);
    }
}
