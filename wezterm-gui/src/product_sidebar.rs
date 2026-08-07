//! Product-neutral sidebar snapshots for applications embedding `wezterm-gui`.
//!
//! The GUI owns presentation, while the embedding product owns catalog and
//! runtime truth.  A provider returns an owned snapshot so renderer code never
//! holds a product lock while shaping text or allocating quads.

use std::fmt;
use std::sync::Arc;

/// Stable product identity carried by sidebar hit targets.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProductSidebarRowId(Arc<str>);

impl ProductSidebarRowId {
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProductSidebarRowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for ProductSidebarRowId {
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for ProductSidebarRowId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

/// The two durable organization sections presented by the product sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProductSidebarSection {
    Active,
    Settled,
}

impl ProductSidebarSection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Settled => "SETTLED",
        }
    }
}

/// Product-neutral runtime truth.  The renderer supplies the token and color;
/// the embedding application supplies only the semantic state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProductSidebarStatus {
    NotRunning,
    Starting,
    Working,
    WaitingForInput,
    AwaitingApproval,
    CompletedIdle,
    Failed,
    UnknownExternal,
}

impl ProductSidebarStatus {
    pub const fn token(self) -> &'static str {
        match self {
            Self::NotRunning => "IDLE",
            Self::Starting => "START",
            Self::Working => "WORK",
            Self::WaitingForInput => "NEEDS",
            Self::AwaitingApproval => "APPR",
            Self::CompletedIdle => "DONE",
            Self::Failed => "FAIL",
            Self::UnknownExternal => "EXT?",
        }
    }
}

/// One immutable row in a product sidebar snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSidebarRow {
    pub id: ProductSidebarRowId,
    pub title: String,
    /// Bounded, already-formatted secondary truth such as project and age.
    pub metadata: String,
    pub status: ProductSidebarStatus,
    /// True only for the row attached to the visible terminal.
    pub attached: bool,
}

/// An owned, render-ready view of the durable product catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductSidebarSnapshot {
    pub active: Vec<ProductSidebarRow>,
    pub settled: Vec<ProductSidebarRow>,
}

impl ProductSidebarSnapshot {
    pub fn rows_in_presentation_order(
        &self,
    ) -> impl Iterator<Item = (ProductSidebarSection, &ProductSidebarRow)> {
        self.active
            .iter()
            .map(|row| (ProductSidebarSection::Active, row))
            .chain(
                self.settled
                    .iter()
                    .map(|row| (ProductSidebarSection::Settled, row)),
            )
    }
}

type SnapshotCallback = dyn Fn() -> ProductSidebarSnapshot + Send + Sync + 'static;
type ActivateCallback = dyn Fn(&ProductSidebarRowId) + Send + Sync + 'static;

/// Thread-safe bridge between product state and the generic GUI.
///
/// Callbacks must be bounded and non-blocking.  In particular, `activate`
/// should enqueue or dispatch product work instead of waiting on a process.
#[derive(Clone)]
pub struct ProductSidebarProvider {
    snapshot: Arc<SnapshotCallback>,
    activate: Arc<ActivateCallback>,
}

impl ProductSidebarProvider {
    pub fn new<S, A>(snapshot: S, activate: A) -> Self
    where
        S: Fn() -> ProductSidebarSnapshot + Send + Sync + 'static,
        A: Fn(&ProductSidebarRowId) + Send + Sync + 'static,
    {
        Self {
            snapshot: Arc::new(snapshot),
            activate: Arc::new(activate),
        }
    }

    pub fn snapshot(&self) -> ProductSidebarSnapshot {
        (self.snapshot)()
    }

    pub fn activate(&self, id: &ProductSidebarRowId) {
        (self.activate)(id);
    }
}

impl fmt::Debug for ProductSidebarProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductSidebarProvider")
            .finish_non_exhaustive()
    }
}

impl PartialEq for ProductSidebarProvider {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.snapshot, &other.snapshot) && Arc::ptr_eq(&self.activate, &other.activate)
    }
}

impl Eq for ProductSidebarProvider {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, RwLock};

    fn row(id: &str, status: ProductSidebarStatus) -> ProductSidebarRow {
        ProductSidebarRow {
            id: id.into(),
            title: id.to_string(),
            metadata: String::new(),
            status,
            attached: false,
        }
    }

    #[test]
    fn snapshot_preserves_section_and_row_order() {
        let snapshot = ProductSidebarSnapshot {
            active: vec![row("active-a", ProductSidebarStatus::Working)],
            settled: vec![
                row("settled-a", ProductSidebarStatus::CompletedIdle),
                row("settled-b", ProductSidebarStatus::Failed),
            ],
        };

        let actual: Vec<_> = snapshot
            .rows_in_presentation_order()
            .map(|(section, row)| (section, row.id.as_str()))
            .collect();
        assert_eq!(
            actual,
            vec![
                (ProductSidebarSection::Active, "active-a"),
                (ProductSidebarSection::Settled, "settled-a"),
                (ProductSidebarSection::Settled, "settled-b"),
            ]
        );
    }

    #[test]
    fn semantic_states_have_stable_visible_tokens() {
        let tokens = [
            ProductSidebarStatus::NotRunning,
            ProductSidebarStatus::Starting,
            ProductSidebarStatus::Working,
            ProductSidebarStatus::WaitingForInput,
            ProductSidebarStatus::AwaitingApproval,
            ProductSidebarStatus::CompletedIdle,
            ProductSidebarStatus::Failed,
            ProductSidebarStatus::UnknownExternal,
        ]
        .map(ProductSidebarStatus::token);
        assert_eq!(
            tokens,
            ["IDLE", "START", "WORK", "NEEDS", "APPR", "DONE", "FAIL", "EXT?"]
        );
    }

    #[test]
    fn provider_returns_owned_snapshot_and_dispatches_stable_id() {
        let state = Arc::new(RwLock::new(ProductSidebarSnapshot {
            active: vec![row("conversation-7", ProductSidebarStatus::Working)],
            settled: vec![],
        }));
        let activated = Arc::new(Mutex::new(Vec::new()));
        let provider = ProductSidebarProvider::new(
            {
                let state = Arc::clone(&state);
                move || state.read().unwrap().clone()
            },
            {
                let activated = Arc::clone(&activated);
                move |id| activated.lock().unwrap().push(id.clone())
            },
        );

        let first = provider.snapshot();
        state.write().unwrap().active.clear();
        assert_eq!(first.active[0].id.as_str(), "conversation-7");
        assert!(provider.snapshot().active.is_empty());

        provider.activate(&first.active[0].id);
        assert_eq!(
            activated.lock().unwrap().as_slice(),
            &["conversation-7".into()]
        );
    }
}
