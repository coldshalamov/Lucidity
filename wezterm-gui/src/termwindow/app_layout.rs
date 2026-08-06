//! Pure physical-pixel layout for optional embedding-product chrome.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RectPhys {
    pub min_x: usize,
    pub min_y: usize,
    pub max_x: usize,
    pub max_y: usize,
}

impl RectPhys {
    pub const fn new(min_x: usize, min_y: usize, max_x: usize, max_y: usize) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    pub fn width(self) -> usize {
        self.max_x.saturating_sub(self.min_x)
    }

    pub fn height(self) -> usize {
        self.max_y.saturating_sub(self.min_y)
    }

    pub fn is_empty(self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn contains(self, x: usize, y: usize) -> bool {
        x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y
    }

    fn inset(self, left: usize, top: usize, right: usize, bottom: usize) -> Self {
        let min_x = self.min_x.saturating_add(left).min(self.max_x);
        let min_y = self.min_y.saturating_add(top).min(self.max_y);
        let max_x = self.max_x.saturating_sub(right).max(min_x);
        let max_y = self.max_y.saturating_sub(bottom).max(min_y);
        Self::new(min_x, min_y, max_x, max_y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarPreference {
    #[default]
    Auto,
    Shown,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductChromeConfig {
    pub sidebar: SidebarPreference,
    pub sidebar_width_logical: usize,
}

impl Default for ProductChromeConfig {
    fn default() -> Self {
        Self {
            sidebar: SidebarPreference::Auto,
            sidebar_width_logical: 264,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppLayout {
    pub client: RectPhys,
    pub title_bar: RectPhys,
    /// Includes the two-pixel seam so every pixel to the terminal edge is
    /// covered by a typed chrome hit target.
    pub sidebar: Option<RectPhys>,
    pub state_rail: Option<RectPhys>,
    pub sidebar_seam: Option<RectPhys>,
    pub sidebar_section: Option<RectPhys>,
    pub sidebar_action_bar: Option<RectPhys>,
    pub sidebar_restore: Option<RectPhys>,
    pub terminal_viewport: RectPhys,
    pub terminal_content: RectPhys,
    pub compact: bool,
    dpi: usize,
}

impl AppLayout {
    pub fn compute(
        client_width: usize,
        client_height: usize,
        dpi: usize,
        config: ProductChromeConfig,
    ) -> Self {
        let dpi = dpi.max(1);
        let client = RectPhys::new(0, 0, client_width, client_height);
        let title_height = physical(32, dpi).min(client_height);
        let title_bar = RectPhys::new(0, 0, client_width, title_height);
        let logical_width = client_width.saturating_mul(96) / dpi;

        let requested_sidebar = match config.sidebar {
            SidebarPreference::Hidden => 0,
            SidebarPreference::Shown => {
                if logical_width < 1_000 {
                    200
                } else {
                    config.sidebar_width_logical.clamp(200, 320)
                }
            }
            SidebarPreference::Auto if logical_width >= 1_000 => {
                config.sidebar_width_logical.clamp(200, 320)
            }
            SidebarPreference::Auto if logical_width >= 820 => 200,
            SidebarPreference::Auto => 0,
        };
        let compact = requested_sidebar == 200;
        let requested_sidebar = physical(requested_sidebar, dpi);
        let seam_width = usize::from(requested_sidebar > 0) * physical(2, dpi).max(1);
        let sidebar_width = requested_sidebar.min(client_width.saturating_sub(seam_width));
        let terminal_x = sidebar_width.saturating_add(seam_width).min(client_width);

        let sidebar = (sidebar_width > 0 && title_height < client_height)
            .then(|| RectPhys::new(0, title_height, terminal_x, client_height));
        let state_rail = sidebar.map(|rect| {
            RectPhys::new(
                rect.min_x,
                rect.min_y,
                rect.min_x.saturating_add(physical(3, dpi)).min(rect.max_x),
                rect.max_y,
            )
        });
        let sidebar_seam =
            sidebar.map(|rect| RectPhys::new(sidebar_width, rect.min_y, rect.max_x, rect.max_y));
        let sidebar_section = sidebar.map(|rect| {
            RectPhys::new(
                rect.min_x,
                rect.min_y,
                rect.max_x,
                rect.min_y.saturating_add(physical(24, dpi)).min(rect.max_y),
            )
        });
        let sidebar_action_bar = sidebar.map(|rect| {
            RectPhys::new(
                rect.min_x,
                rect.max_y.saturating_sub(physical(36, dpi)),
                rect.max_x,
                rect.max_y,
            )
        });

        let terminal_viewport =
            RectPhys::new(terminal_x, title_height, client_width, client_height);
        let terminal_content = terminal_viewport.inset(
            physical(8, dpi),
            physical(6, dpi),
            physical(8, dpi),
            physical(6, dpi),
        );

        let restore_size = physical(28, dpi).max(1);
        let restore_x = physical(12, dpi).min(client_width);
        let restore_y = title_height.saturating_sub(restore_size) / 2;
        let sidebar_restore = (sidebar.is_none() && !title_bar.is_empty()).then(|| {
            RectPhys::new(
                restore_x,
                restore_y,
                restore_x.saturating_add(restore_size).min(client_width),
                restore_y.saturating_add(restore_size).min(title_height),
            )
        });

        Self {
            client,
            title_bar,
            sidebar,
            state_rail,
            sidebar_seam,
            sidebar_section,
            sidebar_action_bar,
            sidebar_restore,
            terminal_viewport,
            terminal_content,
            compact,
            dpi,
        }
    }

    pub fn sidebar_row(self, index: usize) -> Option<RectPhys> {
        let sidebar = self.sidebar?;
        let section = self.sidebar_section?;
        let action = self.sidebar_action_bar?;
        let row_height = physical(if self.compact { 36 } else { 44 }, self.dpi);
        let min_y = section
            .max_y
            .saturating_add(row_height.saturating_mul(index));
        let max_y = min_y.saturating_add(row_height);
        (max_y <= action.min_y).then(|| RectPhys::new(sidebar.min_x, min_y, sidebar.max_x, max_y))
    }

    pub fn weld_for_row(self, row: RectPhys) -> Option<RectPhys> {
        let seam = self.sidebar_seam?;
        let width = physical(3, self.dpi).max(1);
        Some(RectPhys::new(
            seam.max_x.saturating_sub(width),
            row.min_y,
            seam.max_x,
            row.max_y,
        ))
    }

    pub fn chrome_contains(self, x: usize, y: usize) -> bool {
        self.title_bar.contains(x, y) || self.sidebar.is_some_and(|sidebar| sidebar.contains(x, y))
    }
}

pub fn physical(logical: usize, dpi: usize) -> usize {
    logical.saturating_mul(dpi.max(1)).saturating_add(48) / 96
}

impl super::TermWindow {
    pub fn app_layout_for_dimensions(&self, dimensions: &window::Dimensions) -> Option<AppLayout> {
        let mut chrome = crate::product_gui_config()?.chrome?;
        if self.sidebar_force_shown {
            chrome.sidebar = SidebarPreference::Shown;
        }
        Some(AppLayout::compute(
            dimensions.pixel_width,
            dimensions.pixel_height,
            dimensions.dpi,
            chrome,
        ))
    }

    pub fn app_layout(&self) -> Option<AppLayout> {
        self.app_layout_for_dimensions(&self.dimensions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::termwindow::{chrome_item, ChromeItem, UIItem};

    #[test]
    fn default_layout_at_96_dpi() {
        let layout = AppLayout::compute(1280, 800, 96, ProductChromeConfig::default());
        assert_eq!(layout.title_bar, RectPhys::new(0, 0, 1280, 32));
        assert_eq!(layout.sidebar, Some(RectPhys::new(0, 32, 266, 800)));
        assert_eq!(layout.state_rail, Some(RectPhys::new(0, 32, 3, 800)));
        assert_eq!(layout.terminal_content, RectPhys::new(274, 38, 1272, 794));
        assert!(!layout.compact);
    }

    #[test]
    fn auto_sidebar_is_compact_then_hidden() {
        let compact = AppLayout::compute(900, 600, 96, ProductChromeConfig::default());
        assert_eq!(compact.sidebar.unwrap().width(), 202);
        assert!(compact.compact);

        let hidden = AppLayout::compute(819, 600, 96, ProductChromeConfig::default());
        assert!(hidden.sidebar.is_none());
        assert_eq!(hidden.terminal_viewport, RectPhys::new(0, 32, 819, 600));
        assert!(hidden.sidebar_restore.is_some());
    }

    #[test]
    fn scales_logical_contract_at_144_dpi() {
        let layout = AppLayout::compute(1920, 1200, 144, ProductChromeConfig::default());
        assert_eq!(layout.title_bar.height(), 48);
        assert_eq!(layout.sidebar.unwrap().width(), 399);
        assert_eq!(layout.terminal_content.min_x, 411);
        assert_eq!(layout.terminal_content.min_y, 57);
    }

    #[test]
    fn explicit_sidebar_and_tiny_windows_saturate() {
        let shown = AppLayout::compute(
            600,
            320,
            96,
            ProductChromeConfig {
                sidebar: SidebarPreference::Shown,
                ..Default::default()
            },
        );
        assert_eq!(shown.sidebar.unwrap().width(), 202);
        assert!(shown.terminal_content.min_x <= shown.terminal_content.max_x);
        assert!(shown.terminal_content.min_y <= shown.terminal_content.max_y);

        let tiny = AppLayout::compute(1, 1, 192, ProductChromeConfig::default());
        assert!(tiny.title_bar.contains(0, 0));
        assert!(tiny.terminal_content.is_empty());
    }

    #[test]
    fn half_open_chrome_terminal_boundary_has_no_overlap() {
        let layout = AppLayout::compute(1280, 800, 96, ProductChromeConfig::default());
        let terminal_x = layout.terminal_viewport.min_x;
        assert!(layout.chrome_contains(terminal_x - 1, 500));
        assert!(!layout.chrome_contains(terminal_x, 500));
        assert!(layout.terminal_viewport.contains(terminal_x, 500));
    }

    #[test]
    fn typed_hit_items_cover_every_chrome_pixel() {
        let layout = AppLayout::compute(900, 600, 96, ProductChromeConfig::default());
        let mut items: Vec<UIItem> = Vec::new();
        items.push(chrome_item(layout.title_bar, ChromeItem::TitleBar).unwrap());
        items.push(chrome_item(layout.sidebar.unwrap(), ChromeItem::Sidebar).unwrap());
        items.push(chrome_item(layout.state_rail.unwrap(), ChromeItem::StateRail).unwrap());
        items.push(chrome_item(layout.sidebar_seam.unwrap(), ChromeItem::SidebarSeam).unwrap());
        items.push(
            chrome_item(layout.sidebar_section.unwrap(), ChromeItem::SidebarSection).unwrap(),
        );
        for (index, tab_id) in [41, 99].into_iter().enumerate() {
            items.push(
                chrome_item(
                    layout.sidebar_row(index).unwrap(),
                    ChromeItem::SidebarTab(tab_id),
                )
                .unwrap(),
            );
        }
        items.push(
            chrome_item(
                layout.sidebar_action_bar.unwrap(),
                ChromeItem::SidebarActionBar,
            )
            .unwrap(),
        );

        for y in 0..layout.client.max_y {
            for x in 0..layout.client.max_x {
                if layout.chrome_contains(x, y) {
                    assert!(
                        items
                            .iter()
                            .rev()
                            .any(|item| item.hit_test(x as isize, y as isize)),
                        "uncovered chrome pixel ({x}, {y})"
                    );
                }
            }
        }

        let terminal_x = layout.terminal_viewport.min_x;
        assert!(!items
            .iter()
            .any(|item| item.hit_test(terminal_x as isize, 300)));
    }
}
