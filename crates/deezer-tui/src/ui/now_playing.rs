use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use ratatui_image::{Resize, StatefulImage};

use deezer_core::player::state::{PlaybackStatus, RepeatMode};

use crate::client::{Overlay, RowsKind, ViewState};
use crate::i18n::t;
use crate::theme::Theme;
use crate::ui::common::{track_status, STATUS_WIDTH};

const MAX_ART_WIDTH: u16 = 36;

/// Draw the full-page playback view. It remains part of the main layout so the
/// global player controls stay visible and usable at the bottom of the screen.
pub fn draw(frame: &mut Frame, view: &mut ViewState, area: Rect) {
    let selected = std::iter::once(view.overlay.as_ref())
        .chain(view.overlay_stack.iter().rev().map(Some))
        .find_map(|overlay| match overlay {
            Some(Overlay::NowPlaying { selected }) => Some(*selected),
            _ => None,
        })
        .unwrap_or(view.queue_index)
        .min(view.queue.len().saturating_sub(1));

    let Some(track) = view.current_track.clone() else {
        let empty = Paragraph::new(vec![
            Line::from(Span::styled(
                t().now_playing,
                Style::default()
                    .fg(Theme::primary())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(t().queue_empty_subtitle, Theme::dim())),
        ])
        .alignment(Alignment::Center);
        let centered = Rect {
            y: area.y + area.height.saturating_sub(3) / 2,
            height: 3.min(area.height),
            ..area
        };
        frame.render_widget(empty, centered);
        return;
    };

    // Keep both panels useful in compact terminals by stacking them when the
    // horizontal layout would make either side too narrow.
    if area.width >= 82 {
        let columns = Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(area);
        draw_track_panel(frame, view, &track, columns[0], false);
        draw_queue(frame, view, selected, columns[1]);
    } else {
        let rows =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).split(area);
        draw_track_panel(frame, view, &track, rows[0], true);
        draw_queue(frame, view, selected, rows[1]);
    }
}

fn draw_track_panel(
    frame: &mut Frame,
    view: &mut ViewState,
    track: &deezer_core::api::models::TrackData,
    area: Rect,
    metadata_beside_art: bool,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::border())
        .padding(ratatui::widgets::Padding::new(2, 2, 1, 1))
        .title(format!(" {} ", t().now_playing.to_uppercase()))
        .title_style(Theme::title());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visualizer_height = inner.height.clamp(3, 6);
    let info_height = if inner.height >= 12 { 4 } else { 3 };
    let sections = Layout::vertical([
        Constraint::Min(info_height),
        Constraint::Length(1),
        Constraint::Length(visualizer_height),
    ])
    .split(inner);

    if metadata_beside_art {
        let hero = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Length(3),
            Constraint::Percentage(50),
        ])
        .split(sections[0]);
        let art_area = draw_art(frame, view, hero[0], true);
        let beside_art = Rect {
            y: art_area.y,
            height: art_area.height,
            ..hero[2]
        };
        let info_area = vertically_centered(beside_art, info_height);
        draw_track_info(frame, view, track, info_area, Alignment::Left);
    } else {
        let art_bounds = Rect {
            height: sections[0].height.saturating_sub(info_height),
            ..sections[0]
        };
        let art_area = draw_art(frame, view, art_bounds, false);
        let info_area = Rect {
            y: art_area.bottom(),
            height: info_height.min(sections[0].bottom().saturating_sub(art_area.bottom())),
            ..sections[0]
        };
        draw_track_info(frame, view, track, info_area, Alignment::Center);
    }

    draw_visualizer(frame, view, sections[2]);
}

fn draw_track_info(
    frame: &mut Frame,
    view: &ViewState,
    track: &deezer_core::api::models::TrackData,
    area: Rect,
    alignment: Alignment,
) {
    let favorite = view
        .favorites
        .iter()
        .any(|item| item.track_id == track.track_id);
    let title = if favorite {
        format!("{}  ♥", track.title)
    } else {
        track.title.clone()
    };
    let info = Paragraph::new(vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(Theme::text_color())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            &track.artist,
            Style::default().fg(Theme::primary()),
        )),
        Line::from(Span::styled(&track.album, Theme::dim())),
        Line::from(Span::styled(playback_badges(view), Theme::dim())),
    ])
    .alignment(alignment)
    .wrap(Wrap { trim: true });
    frame.render_widget(info, area);
}

fn playback_badges(view: &ViewState) -> String {
    let status = match view.status {
        PlaybackStatus::Playing => "PLAYING",
        PlaybackStatus::Paused => "PAUSED",
        PlaybackStatus::Loading => "LOADING",
        PlaybackStatus::Stopped => "STOPPED",
    };
    let repeat = match view.repeat {
        RepeatMode::Off => "",
        RepeatMode::Queue => "  REPEAT ALL",
        RepeatMode::Track => "  REPEAT ONE",
    };
    let shuffle = if view.shuffle { "  SHUFFLE" } else { "" };
    format!(
        "{status}{shuffle}{repeat}  {}",
        view.quality.as_api_format()
    )
}

fn draw_art(frame: &mut Frame, view: &mut ViewState, bounds: Rect, align_right: bool) -> Rect {
    let bounds = art_render_bounds(bounds, align_right);
    if let Some(ref mut proto) = view.cover_image {
        let resize = Resize::Scale(None);
        let sized = proto.size_for(resize.clone(), bounds);
        let image_area = Rect {
            x: if align_right {
                bounds.right().saturating_sub(sized.width)
            } else {
                bounds.x + bounds.width.saturating_sub(sized.width) / 2
            },
            y: bounds.y,
            width: sized.width,
            height: sized.height,
        };
        let widget =
            StatefulImage::<ratatui_image::protocol::StatefulProtocol>::default().resize(resize);
        frame.render_stateful_widget(widget, image_area, proto);
        view.cover_image_area = Some(image_area);
        image_area
    } else {
        let image_area = draw_art_placeholder(frame, bounds, align_right);
        view.cover_image_area = None;
        image_area
    }
}

fn art_render_bounds(bounds: Rect, align_right: bool) -> Rect {
    let width = bounds.width.min(MAX_ART_WIDTH);
    Rect {
        x: if align_right {
            bounds.right().saturating_sub(width)
        } else {
            bounds.x + bounds.width.saturating_sub(width) / 2
        },
        width,
        ..bounds
    }
}

fn draw_art_placeholder(frame: &mut Frame, area: Rect, align_right: bool) -> Rect {
    let height = area.height.min(area.width / 2).max(1);
    let width = (height * 2).min(area.width);
    let art = Rect {
        x: if align_right {
            area.right().saturating_sub(width)
        } else {
            area.x + area.width.saturating_sub(width) / 2
        },
        y: area.y,
        width,
        height,
    };
    frame.render_widget(
        Block::default()
            .style(Style::default().bg(Theme::surface()))
            .borders(Borders::ALL)
            .border_style(Theme::border()),
        art,
    );
    if height > 1 {
        let note = Paragraph::new("♪")
            .style(
                Style::default()
                    .fg(Theme::primary())
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        frame.render_widget(note, art.inner(Margin::new(1, height / 2)));
    }
    art
}

fn vertically_centered(area: Rect, height: u16) -> Rect {
    let height = height.min(area.height);
    Rect {
        y: area.y + area.height.saturating_sub(height) / 2,
        height,
        ..area
    }
}

/// Draw frequency levels captured from the PipeWire/PulseAudio output monitor.
fn draw_visualizer(frame: &mut Frame, view: &ViewState, area: Rect) {
    if area.is_empty() {
        return;
    }
    let inner = area;
    if inner.is_empty() {
        return;
    }

    let heights = display_heights(&view.spectrum_levels, inner.width, inner.height);

    let lines = (0..inner.height)
        .map(|row| {
            let level = inner.height - row;
            let cells: String = heights
                .iter()
                .map(|height| if *height >= level { '█' } else { ' ' })
                .collect();
            let gradient_position = if inner.height <= 1 {
                1.0
            } else {
                f32::from(level - 1) / f32::from(inner.height - 1)
            };
            Line::from(Span::styled(
                cells,
                Style::default().fg(Theme::visualizer_gradient(gradient_position)),
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn display_heights(levels: &[f32], width: u16, height: u16) -> Vec<u16> {
    if levels.is_empty() || width == 0 {
        return Vec::new();
    }
    // Always reserve one separator column between bars. On narrow terminals
    // this reduces the number of displayed bands instead of letting bars touch.
    let spaced = width >= 2;
    let visible_columns = if spaced {
        usize::from(width).div_ceil(2)
    } else {
        usize::from(width)
    };
    (0..width)
        .map(|column| {
            if spaced && column % 2 == 1 {
                return 0;
            }
            let visible_column = usize::from(if spaced { column / 2 } else { column });
            let band = (visible_column * levels.len() / visible_columns).min(levels.len() - 1);
            (levels[band].clamp(0.0, 1.0) * f32::from(height)).round() as u16
        })
        .collect()
}

fn draw_queue(frame: &mut Frame, view: &mut ViewState, selected: usize, area: Rect) {
    let s = t();
    let rows_area = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::border())
        .title(s.waiting_list_title(view.queue.len()))
        .title_style(Theme::title());

    let header = Row::new(vec![
        Cell::from(Span::raw("")),
        Cell::from(Span::styled(s.header_title, Theme::dim())),
        Cell::from(Span::styled(s.header_artist, Theme::dim())),
        Cell::from(Span::styled(s.header_duration, Theme::dim())),
    ]);
    let rows = view.queue.iter().enumerate().map(|(index, track)| {
        let duration = track.duration_secs();
        Row::new(vec![
            Cell::from(track_status(
                index == selected,
                index == view.queue_index,
                view.is_track_favorite(&track.track_id),
                view.status,
            )),
            Cell::from(Span::styled(&track.title, Theme::text())),
            Cell::from(Span::styled(
                &track.artist,
                Style::default().fg(Theme::primary()),
            )),
            Cell::from(Span::styled(
                format!("{}:{:02}", duration / 60, duration % 60),
                Theme::dim(),
            )),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(STATUS_WIDTH),
            Constraint::Percentage(48),
            Constraint::Percentage(37),
            Constraint::Length(6),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(Theme::highlight())
    .highlight_symbol("");

    let mut state = view.table_state(RowsKind::NowPlayingQueue, selected);
    frame.render_stateful_widget(table, rows_area[0], &mut state);
    view.record_rows(
        rows_area[0],
        2,
        state.offset(),
        view.queue.len(),
        RowsKind::NowPlayingQueue,
    );

    let hints = Line::from(vec![
        Span::styled("Enter", Theme::shortcut_key()),
        Span::styled(s.hint_play, Theme::dim()),
        Span::styled("d", Theme::shortcut_key()),
        Span::styled(s.hint_remove, Theme::dim()),
        Span::styled("f", Theme::shortcut_key()),
        Span::styled(s.hint_favorite, Theme::dim()),
        Span::styled("x", Theme::shortcut_key()),
        Span::styled(s.hint_menu, Theme::dim()),
        Span::styled("Esc", Theme::shortcut_key()),
        Span::styled(s.hint_close, Theme::dim()),
    ]);
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        rows_area[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DaemonSnapshot;
    use ratatui::backend::TestBackend;

    #[test]
    fn visualizer_stays_within_requested_height() {
        let levels = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let heights = display_heights(&levels, 40, 5);
        assert_eq!(heights.len(), 40);
        assert!(heights.iter().all(|height| *height <= 5));
        assert!(heights.contains(&0));
        assert!(heights.contains(&5));
    }

    #[test]
    fn visualizer_keeps_a_gap_between_bars_when_narrow() {
        let heights = display_heights(&vec![1.0; 32], 20, 5);
        assert!(heights.windows(2).all(|pair| pair[0] == 0 || pair[1] == 0));
        assert!(heights.iter().any(|height| *height > 0));
    }

    #[test]
    fn artwork_width_is_capped_without_changing_alignment() {
        let bounds = Rect {
            x: 10,
            y: 2,
            width: 80,
            height: 30,
        };
        let centered = art_render_bounds(bounds, false);
        assert_eq!(centered.width, MAX_ART_WIDTH);
        assert_eq!(centered.x, 32);

        let right_aligned = art_render_bounds(bounds, true);
        assert_eq!(right_aligned.width, MAX_ART_WIDTH);
        assert_eq!(right_aligned.right(), bounds.right());
    }

    #[test]
    fn renders_in_wide_and_compact_terminals() {
        let track: deezer_core::api::models::TrackData =
            serde_json::from_value(serde_json::json!({
                "SNG_ID": "42",
                "SNG_TITLE": "A very good song",
                "ART_NAME": "The Artist",
                "ALB_TITLE": "The Album",
                "DURATION": "240"
            }))
            .unwrap();
        let snapshot = DaemonSnapshot {
            current_track: Some(track.clone()),
            queue: vec![track],
            ..DaemonSnapshot::default()
        };
        let mut view = ViewState::from_snapshot(&snapshot);
        view.overlay = Some(Overlay::NowPlaying { selected: 0 });

        for (width, height) in [(120, 32), (60, 20)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = ratatui::Terminal::new(backend).unwrap();
            terminal
                .draw(|frame| draw(frame, &mut view, frame.area()))
                .unwrap();
        }
    }
}
