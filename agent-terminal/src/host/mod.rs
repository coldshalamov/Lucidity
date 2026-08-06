//! Compile-safe host and lifecycle ownership boundary.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseIntent {
    StockClose,
    HideToTray,
    QuitOwned,
}
