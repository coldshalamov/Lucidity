use crate::quad::TripleLayerQuadAllocator;
use crate::termwindow::app_layout::{physical, RectPhys};
use crate::termwindow::render::RenderScreenLineParams;
use crate::termwindow::{chrome_item, ChromeItem};
use crate::utilsprites::RenderMetrics;
use crate::{
    ProductSidebarRow, ProductSidebarSection, ProductSidebarSnapshot, ProductSidebarStatus,
};
use anyhow::Context;
use mux::renderable::RenderableDimensions;
use termwiz::cell::CellAttributes;
use termwiz::surface::Line;
use window::color::LinearRgba;

const SURFACE_WINDOW: LinearRgba = LinearRgba::with_components(0.0037, 0.0044, 0.0057, 1.0);
const SURFACE_SIDEBAR: LinearRgba = LinearRgba::with_components(0.0052, 0.0065, 0.0085, 1.0);
const SURFACE_SELECTED: LinearRgba = LinearRgba::with_components(0.0116, 0.0160, 0.0231, 1.0);
const SURFACE_TRACK: LinearRgba = LinearRgba::with_components(0.0168, 0.0232, 0.0331, 1.0);
const TEXT_PRIMARY: LinearRgba = LinearRgba::with_components(0.7913, 0.8232, 0.8714, 1.0);
const TEXT_TERTIARY: LinearRgba = LinearRgba::with_components(0.2016, 0.2462, 0.3005, 1.0);
const BORDER_SEAM: LinearRgba = LinearRgba::with_components(0.1356, 0.1701, 0.2195, 1.0);

impl crate::TermWindow {
    fn paint_product_rect(
        &self,
        layers: &mut TripleLayerQuadAllocator,
        rect: RectPhys,
        color: LinearRgba,
    ) -> anyhow::Result<()> {
        if rect.is_empty() {
            return Ok(());
        }
        self.filled_rectangle(
            layers,
            2,
            euclid::rect(
                rect.min_x as f32,
                rect.min_y as f32,
                rect.width() as f32,
                rect.height() as f32,
            ),
            color,
        )
        .context("paint product chrome rectangle")?;
        Ok(())
    }

    pub fn paint_product_chrome(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let Some(layout) = self.app_layout() else {
            return Ok(());
        };

        self.paint_product_rect(layers, layout.title_bar, SURFACE_WINDOW)?;
        if let Some(item) = chrome_item(layout.title_bar, ChromeItem::TitleBar) {
            self.ui_items.push(item);
        }

        if let Some(sidebar) = layout.sidebar {
            self.paint_product_rect(layers, sidebar, SURFACE_SIDEBAR)?;
            if let Some(item) = chrome_item(sidebar, ChromeItem::Sidebar) {
                self.ui_items.push(item);
            }

            if let Some(rail) = layout.state_rail {
                self.paint_product_rect(layers, rail, SURFACE_TRACK)?;
                if let Some(item) = chrome_item(rail, ChromeItem::StateRail) {
                    self.ui_items.push(item);
                }
            }
            if let Some(seam) = layout.sidebar_seam {
                self.paint_product_rect(layers, seam, BORDER_SEAM)?;
                if let Some(item) = chrome_item(seam, ChromeItem::SidebarSeam) {
                    self.ui_items.push(item);
                }
            }
            let provider = crate::product_gui_config()
                .and_then(|product| product.chrome)
                .and_then(|chrome| chrome.sidebar_provider);
            if let Some(provider) = provider {
                let snapshot = provider.snapshot();
                self.paint_product_sidebar_snapshot(layers, layout, &snapshot)?;
            } else {
                self.paint_mux_sidebar_fallback(layers, layout)?;
            }

            if let Some(action_bar) = layout.sidebar_action_bar {
                self.paint_product_rect(layers, action_bar, SURFACE_WINDOW)?;
                if let Some(item) = chrome_item(action_bar, ChromeItem::SidebarActionBar) {
                    self.ui_items.push(item);
                }
            }
        }

        self.paint_product_mark(layers, layout.sidebar_restore.is_some())?;
        let identity_x = physical(
            if layout.sidebar_restore.is_some() {
                76
            } else {
                36
            },
            self.dimensions.dpi,
        );
        self.paint_product_text(
            layers,
            "LUCIDITY",
            identity_x,
            physical(6, self.dimensions.dpi),
            physical(112, self.dimensions.dpi),
            (0xE6, 0xEA, 0xF0),
        )?;
        if let Some(product) = crate::product_gui_config() {
            self.paint_product_text(
                layers,
                &format!("AGENT  {}  v{}", self.mux_window_id, product.version),
                identity_x.saturating_add(physical(124, self.dimensions.dpi)),
                physical(6, self.dimensions.dpi),
                layout
                    .title_bar
                    .max_x
                    .saturating_sub(identity_x)
                    .saturating_sub(physical(148, self.dimensions.dpi)),
                (0x7C, 0x88, 0x95),
            )?;
        }
        if let Some(restore) = layout.sidebar_restore {
            self.paint_product_rect(layers, restore, SURFACE_TRACK)?;
            let stroke = physical(2, self.dimensions.dpi).max(1);
            let glyph = RectPhys::new(
                restore.min_x.saturating_add(stroke * 3).min(restore.max_x),
                restore.min_y.saturating_add(stroke * 3).min(restore.max_y),
                restore.max_x.saturating_sub(stroke * 3),
                restore.max_y.saturating_sub(stroke * 3),
            );
            self.paint_product_rect(layers, glyph, BORDER_SEAM)?;
            if let Some(item) = chrome_item(restore, ChromeItem::SidebarRestore) {
                self.ui_items.push(item);
            }
        }

        Ok(())
    }

    fn paint_product_sidebar_section(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        rect: RectPhys,
        section: ProductSidebarSection,
    ) -> anyhow::Result<()> {
        self.paint_product_rect(layers, rect, SURFACE_WINDOW)?;
        self.paint_product_text(
            layers,
            section.label(),
            rect.min_x.saturating_add(physical(12, self.dimensions.dpi)),
            rect.min_y.saturating_add(physical(3, self.dimensions.dpi)),
            rect.width()
                .saturating_sub(physical(24, self.dimensions.dpi)),
            (0xA6, 0xB0, 0xBD),
        )?;
        if let Some(item) = chrome_item(rect, ChromeItem::SidebarSection(section)) {
            self.ui_items.push(item);
        }
        Ok(())
    }

    fn paint_product_sidebar_snapshot(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        layout: crate::AppLayout,
        snapshot: &ProductSidebarSnapshot,
    ) -> anyhow::Result<()> {
        for item in layout_product_sidebar_snapshot(layout, snapshot) {
            match item.kind {
                ProductSidebarLayoutItemKind::Section(section) => {
                    self.paint_product_sidebar_section(layers, item.rect, section)?;
                }
                ProductSidebarLayoutItemKind::Row { section, row } => {
                    self.paint_product_sidebar_row(layers, layout, item.rect, section, row)?;
                    if let Some(hit) =
                        chrome_item(item.rect, ChromeItem::SidebarConversation(row.id.clone()))
                    {
                        self.ui_items.push(hit);
                    }
                }
            }
        }
        Ok(())
    }

    fn paint_product_sidebar_row(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        layout: crate::AppLayout,
        rect: RectPhys,
        section: ProductSidebarSection,
        row: &ProductSidebarRow,
    ) -> anyhow::Result<()> {
        if row.attached {
            self.paint_product_rect(layers, rect, SURFACE_SELECTED)?;
        }

        let (status_text, status_color) = product_status_colors(row.status);
        if let Some(rail) = layout.state_rail {
            let cap = RectPhys::new(rail.min_x, rect.min_y, rail.max_x, rect.max_y);
            self.paint_product_rect(layers, cap, linear_srgb(status_color))?;
        }
        if row.attached {
            if let Some(weld) = layout.weld_for_row(rect) {
                self.paint_product_rect(layers, weld, TEXT_PRIMARY)?;
            }
        }

        let dpi = self.dimensions.dpi;
        let title_x = rect.min_x.saturating_add(physical(12, dpi));
        let token_width = physical(52, dpi);
        let text_width = rect
            .max_x
            .saturating_sub(title_x)
            .saturating_sub(token_width);
        let title_y = rect.min_y.saturating_add(physical(
            if section == ProductSidebarSection::Active {
                3
            } else {
                4
            },
            dpi,
        ));
        self.paint_product_text(
            layers,
            &row.title,
            title_x,
            title_y,
            text_width,
            if row.attached {
                (0xE6, 0xEA, 0xF0)
            } else {
                (0xA6, 0xB0, 0xBD)
            },
        )?;
        if section == ProductSidebarSection::Active && !row.metadata.is_empty() {
            self.paint_product_text(
                layers,
                &row.metadata,
                title_x,
                rect.min_y.saturating_add(physical(22, dpi)),
                text_width,
                (0x7C, 0x88, 0x95),
            )?;
        }
        self.paint_product_text(
            layers,
            row.status.token(),
            rect.max_x.saturating_sub(token_width),
            rect.min_y.saturating_add(physical(
                if section == ProductSidebarSection::Active {
                    22
                } else {
                    4
                },
                dpi,
            )),
            token_width.saturating_sub(physical(8, dpi)),
            status_text,
        )?;
        Ok(())
    }

    fn paint_mux_sidebar_fallback(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        layout: crate::AppLayout,
    ) -> anyhow::Result<()> {
        if let Some(section) = layout.sidebar_section {
            self.paint_product_sidebar_section(layers, section, ProductSidebarSection::Active)?;
        }

        for (row_index, tab) in self.get_tab_information().into_iter().enumerate() {
            let Some(row) = layout.sidebar_row(row_index) else {
                break;
            };
            if tab.is_active {
                self.paint_product_rect(layers, row, SURFACE_SELECTED)?;
            }
            if let Some(rail) = layout.state_rail {
                let cap = RectPhys::new(rail.min_x, row.min_y, rail.max_x, row.max_y);
                self.paint_product_rect(layers, cap, TEXT_TERTIARY)?;
            }
            if tab.is_active {
                if let Some(weld) = layout.weld_for_row(row) {
                    self.paint_product_rect(layers, weld, TEXT_PRIMARY)?;
                }
            }
            let title_x = row.min_x.saturating_add(physical(12, self.dimensions.dpi));
            let token_width = physical(44, self.dimensions.dpi);
            self.paint_product_text(
                layers,
                &tab.tab_title,
                title_x,
                row.min_y.saturating_add(physical(6, self.dimensions.dpi)),
                row.max_x
                    .saturating_sub(title_x)
                    .saturating_sub(token_width),
                if tab.is_active {
                    (0xE6, 0xEA, 0xF0)
                } else {
                    (0xA6, 0xB0, 0xBD)
                },
            )?;
            self.paint_product_text(
                layers,
                "EXT?",
                row.max_x.saturating_sub(token_width),
                row.min_y.saturating_add(physical(6, self.dimensions.dpi)),
                token_width.saturating_sub(physical(8, self.dimensions.dpi)),
                (0x9B, 0xA8, 0xB7),
            )?;
            if let Some(item) = chrome_item(row, ChromeItem::SidebarTab(tab.tab_id)) {
                self.ui_items.push(item);
            }
        }
        Ok(())
    }

    fn paint_product_mark(
        &self,
        layers: &mut TripleLayerQuadAllocator,
        after_restore: bool,
    ) -> anyhow::Result<()> {
        let dpi = self.dimensions.dpi;
        let x = physical(if after_restore { 52 } else { 12 }, dpi);
        let y = physical(8, dpi);
        let rect = |x0: usize, y0: usize, x1: usize, y1: usize| {
            RectPhys::new(
                x + physical(x0, dpi),
                y + physical(y0, dpi),
                x + physical(x1, dpi),
                y + physical(y1, dpi),
            )
        };
        for part in [
            rect(2, 2, 5, 14),
            rect(5, 2, 12, 5),
            rect(5, 11, 12, 14),
            rect(7, 6, 9, 10),
            rect(11, 6, 14, 10),
        ] {
            self.paint_product_rect(layers, part, TEXT_PRIMARY)?;
        }
        Ok(())
    }

    fn paint_product_text(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        text: &str,
        x: usize,
        y: usize,
        pixel_width: usize,
        foreground: (u8, u8, u8),
    ) -> anyhow::Result<()> {
        if text.is_empty() || pixel_width == 0 {
            return Ok(());
        }
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let advance = metrics.cell_size.width.max(1) as usize;
        let available_chars = pixel_width.saturating_add(advance.saturating_sub(1)) / advance;
        let rendered = truncate_tail(text, available_chars.saturating_sub(2));
        if rendered.is_empty() {
            return Ok(());
        }
        let line = Line::from_text(&rendered, &CellAttributes::default(), 0, None);
        let config = self.config.clone();
        let mut palette = self.palette().clone();
        palette.foreground = foreground.into();
        palette.background = (0x0C, 0x0E, 0x11).into();
        let gl_state = self.render_state.as_ref().unwrap();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();

        self.render_screen_line(
            RenderScreenLineParams {
                top_pixel_y: y as f32,
                left_pixel_x: x as f32,
                pixel_width: pixel_width as f32,
                stable_line_idx: None,
                line: &line,
                selection: 0..0,
                cursor: &Default::default(),
                palette: &palette,
                dims: &RenderableDimensions {
                    cols: line.len(),
                    physical_top: 0,
                    scrollback_rows: 0,
                    scrollback_top: 0,
                    viewport_rows: 1,
                    dpi: self.terminal_size.dpi,
                    pixel_height: metrics.cell_size.height as usize,
                    pixel_width,
                    reverse_video: false,
                },
                config: &config,
                cursor_border_color: LinearRgba::TRANSPARENT,
                foreground: palette.foreground.to_linear(),
                pane: None,
                is_active: true,
                selection_fg: LinearRgba::TRANSPARENT,
                selection_bg: LinearRgba::TRANSPARENT,
                cursor_fg: LinearRgba::TRANSPARENT,
                cursor_bg: LinearRgba::TRANSPARENT,
                cursor_is_default_color: true,
                white_space,
                filled_box,
                window_is_transparent: false,
                default_bg: SURFACE_WINDOW,
                style: config.window_frame.font.as_ref(),
                font: Some(font),
                use_pixel_positioning: true,
                render_metrics: metrics,
                shape_key: None,
                password_input: false,
            },
            layers,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProductSidebarLayoutItem<'a> {
    rect: RectPhys,
    kind: ProductSidebarLayoutItemKind<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProductSidebarLayoutItemKind<'a> {
    Section(ProductSidebarSection),
    Row {
        section: ProductSidebarSection,
        row: &'a ProductSidebarRow,
    },
}

/// Produce complete visible bands only.  The settled header is reserved before
/// active rows so both organization axes stay discoverable in bounded space.
fn layout_product_sidebar_snapshot<'a>(
    layout: crate::AppLayout,
    snapshot: &'a ProductSidebarSnapshot,
) -> Vec<ProductSidebarLayoutItem<'a>> {
    let Some(active_header) = layout.sidebar_section else {
        return Vec::new();
    };
    let Some(action_bar) = layout.sidebar_action_bar else {
        return Vec::new();
    };

    let mut items = vec![ProductSidebarLayoutItem {
        rect: active_header,
        kind: ProductSidebarLayoutItemKind::Section(ProductSidebarSection::Active),
    }];
    let mut cursor = active_header.max_y;
    let settled_header_height = layout
        .sidebar_band(cursor, 24)
        .map(RectPhys::height)
        .unwrap_or(active_header.height());
    let active_limit = action_bar.min_y.saturating_sub(settled_header_height);

    for row in &snapshot.active {
        let Some(rect) = layout.sidebar_band(cursor, 44) else {
            break;
        };
        if rect.max_y > active_limit {
            break;
        }
        items.push(ProductSidebarLayoutItem {
            rect,
            kind: ProductSidebarLayoutItemKind::Row {
                section: ProductSidebarSection::Active,
                row,
            },
        });
        cursor = rect.max_y;
    }

    if let Some(rect) = layout.sidebar_band(cursor, 24) {
        items.push(ProductSidebarLayoutItem {
            rect,
            kind: ProductSidebarLayoutItemKind::Section(ProductSidebarSection::Settled),
        });
        cursor = rect.max_y;
    }

    for row in &snapshot.settled {
        let Some(rect) = layout.sidebar_band(cursor, 28) else {
            break;
        };
        items.push(ProductSidebarLayoutItem {
            rect,
            kind: ProductSidebarLayoutItemKind::Row {
                section: ProductSidebarSection::Settled,
                row,
            },
        });
        cursor = rect.max_y;
    }

    items
}

fn product_status_colors(status: ProductSidebarStatus) -> ((u8, u8, u8), (u8, u8, u8)) {
    let color = match status {
        ProductSidebarStatus::NotRunning => (0x7C, 0x88, 0x95),
        ProductSidebarStatus::Starting | ProductSidebarStatus::Working => (0x58, 0xA6, 0xFF),
        ProductSidebarStatus::WaitingForInput => (0xE3, 0xB3, 0x41),
        ProductSidebarStatus::AwaitingApproval => (0xFF, 0x8A, 0x4C),
        ProductSidebarStatus::CompletedIdle => (0x56, 0xD3, 0x64),
        ProductSidebarStatus::Failed => (0xF8, 0x51, 0x49),
        ProductSidebarStatus::UnknownExternal => (0x9B, 0xA8, 0xB7),
    };
    (color, color)
}

fn linear_srgb((red, green, blue): (u8, u8, u8)) -> LinearRgba {
    fn component(value: u8) -> f32 {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    LinearRgba::with_components(component(red), component(green), component(blue), 1.0)
}

fn truncate_tail(text: &str, budget: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= budget {
        return text.to_string();
    }
    match budget {
        0 => String::new(),
        1 => "…".to_string(),
        _ => {
            let mut truncated: String = text.chars().take(budget - 1).collect();
            truncated.push('…');
            truncated
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppLayout, ProductChromeConfig, ProductSidebarRowId};

    fn row(id: &str, status: ProductSidebarStatus) -> ProductSidebarRow {
        ProductSidebarRow {
            id: id.into(),
            title: id.to_string(),
            metadata: "repo · now".to_string(),
            status,
            attached: false,
        }
    }

    #[test]
    fn tail_truncation_includes_ellipsis_in_budget() {
        assert_eq!(truncate_tail("abcdef", 4), "abc…");
        assert_eq!(truncate_tail("abcdef", 1), "…");
        assert_eq!(truncate_tail("abcdef", 0), "");
        assert_eq!(truncate_tail("abc", 4), "abc");
    }

    #[test]
    fn snapshot_layout_emits_active_then_settled_with_stable_row_hits() {
        let layout = AppLayout::compute(1_280, 320, 96, ProductChromeConfig::default());
        let snapshot = ProductSidebarSnapshot {
            active: vec![row("active-1", ProductSidebarStatus::Working)],
            settled: vec![row("settled-1", ProductSidebarStatus::CompletedIdle)],
        };
        let items = layout_product_sidebar_snapshot(layout, &snapshot);

        assert!(matches!(
            items[0].kind,
            ProductSidebarLayoutItemKind::Section(ProductSidebarSection::Active)
        ));
        assert!(matches!(
            items[1].kind,
            ProductSidebarLayoutItemKind::Row {
                section: ProductSidebarSection::Active,
                row
            } if row.id.as_str() == "active-1"
        ));
        assert!(matches!(
            items[2].kind,
            ProductSidebarLayoutItemKind::Section(ProductSidebarSection::Settled)
        ));
        let settled = items[3];
        let id = match settled.kind {
            ProductSidebarLayoutItemKind::Row { section, row } => {
                assert_eq!(section, ProductSidebarSection::Settled);
                row.id.clone()
            }
            _ => panic!("expected settled row"),
        };
        let hit = chrome_item(settled.rect, ChromeItem::SidebarConversation(id.clone())).unwrap();
        assert_eq!(
            hit.item_type,
            crate::termwindow::UIItemType::Chrome(ChromeItem::SidebarConversation(
                ProductSidebarRowId::from("settled-1")
            ))
        );
        assert_eq!(id.as_str(), "settled-1");
    }

    #[test]
    fn snapshot_layout_reserves_both_headers_and_never_emits_partial_rows() {
        let layout = AppLayout::compute(900, 140, 96, ProductChromeConfig::default());
        let snapshot = ProductSidebarSnapshot {
            active: vec![row("active-1", ProductSidebarStatus::Working)],
            settled: vec![row("settled-1", ProductSidebarStatus::Failed)],
        };
        let items = layout_product_sidebar_snapshot(layout, &snapshot);

        assert_eq!(items.len(), 2);
        assert!(matches!(
            items[0].kind,
            ProductSidebarLayoutItemKind::Section(ProductSidebarSection::Active)
        ));
        assert!(matches!(
            items[1].kind,
            ProductSidebarLayoutItemKind::Section(ProductSidebarSection::Settled)
        ));
        assert!(items
            .iter()
            .all(|item| item.rect.max_y <= layout.sidebar_action_bar.unwrap().min_y));
    }
}
