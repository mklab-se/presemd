use std::time::Instant;

use eframe::egui::{self, Pos2};

use crate::parser::{Block, Slide};
use crate::render::image_cache::ImageCache;
use crate::render::text;
use crate::render::visualizations::{
    bar_chart, donut_chart, funnel_chart, kpi_cards, line_chart, org_chart, pie_chart,
    progress_bars, radar_chart, scatter_plot, stacked_bar, timeline, venn_diagram, word_cloud,
};
use crate::theme::Theme;

fn is_viz_block(block: &Block) -> bool {
    matches!(
        block,
        Block::WordCloud { .. }
            | Block::Timeline { .. }
            | Block::PieChart { .. }
            | Block::BarChart { .. }
            | Block::LineChart { .. }
            | Block::DonutChart { .. }
            | Block::KpiCards { .. }
            | Block::FunnelChart { .. }
            | Block::RadarChart { .. }
            | Block::StackedBar { .. }
            | Block::VennDiagram { .. }
            | Block::ProgressBars { .. }
            | Block::ScatterPlot { .. }
            | Block::OrgChart { .. }
    )
}

/// Visualization slide layout: heading at top, optional text blocks, visualization
/// filling remaining space.
#[allow(clippy::too_many_arguments)]
pub fn render(
    ui: &egui::Ui,
    slide: &Slide,
    theme: &Theme,
    rect: egui::Rect,
    opacity: f32,
    image_cache: &ImageCache,
    reveal_step: usize,
    reveal_timestamp: Option<Instant>,
    scale: f32,
) {
    let padding = 60.0 * scale;
    let content_width = rect.width() - padding * 2.0;
    let content_left = rect.left() + padding;
    let mut y = rect.top() + padding;

    // Separate blocks into: first heading, text blocks, first viz block
    let mut heading: Option<&Block> = None;
    let mut viz_block: Option<&Block> = None;
    let mut text_blocks: Vec<&Block> = Vec::new();

    for block in &slide.blocks {
        match block {
            Block::Heading { .. } if heading.is_none() => {
                heading = Some(block);
            }
            _ if is_viz_block(block) && viz_block.is_none() => {
                viz_block = Some(block);
            }
            _ if !is_viz_block(block) && !matches!(block, Block::Heading { .. }) => {
                text_blocks.push(block);
            }
            _ => {}
        }
    }

    // Draw heading if present
    if let Some(Block::Heading { level, inlines }) = heading {
        let h = text::draw_heading(
            ui,
            inlines,
            *level,
            theme,
            Pos2::new(content_left, y),
            content_width,
            opacity,
            scale,
        );
        y += h + 30.0 * scale;
    }

    // Draw any text blocks (paragraphs, lists, etc.) between heading and visualization
    let block_spacing = 16.0 * scale;
    for block in &text_blocks {
        let h = text::draw_block(
            ui,
            block,
            theme,
            Pos2::new(content_left, y),
            content_width,
            opacity,
            image_cache,
            reveal_step,
            scale,
        );
        y += h + block_spacing;
    }

    // Draw visualization filling the remaining vertical space
    if let Some(block) = viz_block {
        let remaining_height = rect.bottom() - y - padding;
        if remaining_height > 50.0 * scale {
            let viz_pos = Pos2::new(content_left, y);
            let ts = reveal_timestamp;
            match block {
                Block::WordCloud { content } => {
                    word_cloud::draw_word_cloud(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        scale,
                    );
                }
                Block::Timeline { content } => {
                    timeline::draw_timeline(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        scale,
                    );
                }
                Block::PieChart { content } => {
                    pie_chart::draw_pie_chart(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::BarChart { content } => {
                    bar_chart::draw_bar_chart(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::LineChart { content } => {
                    line_chart::draw_line_chart(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::DonutChart { content } => {
                    donut_chart::draw_donut_chart(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::KpiCards { content } => {
                    kpi_cards::draw_kpi_cards(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::FunnelChart { content } => {
                    funnel_chart::draw_funnel_chart(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::RadarChart { content } => {
                    radar_chart::draw_radar_chart(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::StackedBar { content } => {
                    stacked_bar::draw_stacked_bar(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::VennDiagram { content } => {
                    venn_diagram::draw_venn_diagram(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::ProgressBars { content } => {
                    progress_bars::draw_progress_bars(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::ScatterPlot { content } => {
                    scatter_plot::draw_scatter_plot(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                Block::OrgChart { content } => {
                    org_chart::draw_org_chart(
                        ui,
                        content,
                        theme,
                        viz_pos,
                        content_width,
                        remaining_height,
                        opacity,
                        reveal_step,
                        ts,
                        scale,
                    );
                }
                _ => {}
            }
        }
    }
}
