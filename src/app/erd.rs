//! Entity-Relationship Diagram rendering using iced's `canvas`.
//!
//! The diagram lays tables out on a grid, draws each as a titled box listing
//! its columns (with PK/FK markers), and connects foreign keys with lines. The
//! view supports panning (drag) and zooming (buttons) so large schemas remain
//! navigable.

use std::collections::HashMap;

use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{button, canvas as canvas_widget, column, container, row, text, Space};
use iced::{
    alignment, Alignment, Color, Element, Event, Length, Padding, Point, Rectangle, Renderer, Size,
    Theme, Vector,
};

use decentdb_studio::db::schema::Table;

use super::{style, Message, Studio};

const BOX_WIDTH: f32 = 230.0;
const HEADER_HEIGHT: f32 = 28.0;
const ROW_HEIGHT: f32 = 20.0;
const H_GAP: f32 = 95.0;
const V_GAP: f32 = 58.0;

/// Messages produced by the ERD canvas.
#[derive(Debug, Clone)]
pub enum ErdMessage {
    Pan(Vector),
    ZoomIn,
    ZoomOut,
    ResetView,
}

/// View-state for the ERD, stored on [`Studio`].
#[derive(Debug, Clone)]
pub struct ErdState {
    pub offset: Vector,
    pub zoom: f32,
}

impl Default for ErdState {
    fn default() -> Self {
        Self {
            offset: Vector::new(20.0, 20.0),
            zoom: 1.0,
        }
    }
}

/// Apply an [`ErdMessage`] to the studio state.
pub fn update(app: &mut Studio, message: ErdMessage) {
    match message {
        ErdMessage::Pan(delta) => {
            app.erd.offset = app.erd.offset + delta;
        }
        ErdMessage::ZoomIn => app.erd.zoom = (app.erd.zoom * 1.2).min(3.0),
        ErdMessage::ZoomOut => app.erd.zoom = (app.erd.zoom / 1.2).max(0.3),
        ErdMessage::ResetView => app.erd = ErdState::default(),
    }
}

/// Render the ERD panel (toolbar + canvas).
pub fn view(app: &Studio) -> Element<'_, Message> {
    if app.schema.tables.is_empty() {
        return container(
            text("No tables to diagram. Open a database with tables.").style(text::secondary),
        )
        .center_x(Length::Fill)
        .padding(20)
        .into();
    }

    let toolbar = row![
        text(format!(
            "Entity-Relationship Diagram · {} tables",
            app.schema.tables.len()
        ))
        .size(14),
        Space::new().width(Length::Fill),
        erd_button("Zoom +", ErdMessage::ZoomIn),
        erd_button("Zoom −", ErdMessage::ZoomOut),
        erd_button("Reset", ErdMessage::ResetView),
        text(format!("{:.0}%", app.erd.zoom * 100.0)).size(13),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let diagram = ErdCanvas {
        tables: &app.schema.tables,
        state: &app.erd,
    };

    let diagram_canvas = canvas_widget(diagram)
        .width(Length::Fill)
        .height(Length::Fill);

    column![
        container(toolbar).padding(8),
        container(diagram_canvas)
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

fn erd_button(label: &str, msg: ErdMessage) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding(Padding::from([4, 10]))
        .style(style::toolbar_button)
        .on_press(Message::ErdMessage(msg))
        .into()
}

/// The canvas program drawing the diagram.
struct ErdCanvas<'a> {
    tables: &'a [Table],
    state: &'a ErdState,
}

/// Interaction state for drag-panning.
#[derive(Default)]
struct Interaction {
    dragging: bool,
    last_cursor: Point,
}

impl<'a> canvas::Program<Message> for ErdCanvas<'a> {
    type State = Interaction;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cursor_pos = cursor.position_in(bounds)?;
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.dragging = true;
                state.last_cursor = cursor_pos;
                Some(canvas::Action::request_redraw())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                Some(canvas::Action::request_redraw())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let delta = Vector::new(
                    cursor_pos.x - state.last_cursor.x,
                    cursor_pos.y - state.last_cursor.y,
                );
                state.last_cursor = cursor_pos;
                Some(canvas::Action::publish(Message::ErdMessage(
                    ErdMessage::Pan(delta),
                )))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());

        // Background.
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), palette.background.base.color);
        draw_canvas_grid(
            &mut frame,
            bounds.size(),
            Color {
                a: 0.18,
                ..palette.background.strong.color
            },
        );

        let zoom = self.state.zoom;
        let layout = layout_tables(self.tables);

        // Draw relationship lines first (under the boxes).
        let box_centers: HashMap<&str, Point> = layout
            .iter()
            .map(|(name, pos)| {
                let height = table_height(table_by_name(self.tables, name));
                (
                    name.as_str(),
                    Point::new(pos.x + BOX_WIDTH / 2.0, pos.y + height / 2.0),
                )
            })
            .collect();

        for table in self.tables {
            for fk in &table.foreign_keys {
                let (Some(from), Some(to)) = (
                    box_centers.get(table.name.as_str()),
                    box_centers.get(fk.referenced_table.as_str()),
                ) else {
                    continue;
                };
                let from = transform(*from, self.state.offset, zoom);
                let to = transform(*to, self.state.offset, zoom);
                let mid_x = (from.x + to.x) / 2.0;
                let line = Path::new(|path| {
                    path.move_to(from);
                    path.line_to(Point::new(mid_x, from.y));
                    path.line_to(Point::new(mid_x, to.y));
                    path.line_to(to);
                });
                frame.stroke(
                    &line,
                    Stroke::default()
                        .with_width(1.5)
                        .with_color(palette.primary.strong.color),
                );
                // Arrow head dot at the referenced (parent) side.
                frame.fill(&Path::circle(to, 4.0), palette.primary.strong.color);
            }
        }

        // Draw table boxes.
        for table in self.tables {
            let Some(pos) = layout.get(&table.name) else {
                continue;
            };
            draw_table(&mut frame, table, *pos, self.state.offset, zoom, palette);
        }

        vec![frame.into_geometry()]
    }
}

fn draw_canvas_grid(frame: &mut Frame, size: Size, color: Color) {
    let spacing = 32.0;
    let mut x = 0.0;
    while x <= size.width {
        let path = Path::line(Point::new(x, 0.0), Point::new(x, size.height));
        frame.stroke(&path, Stroke::default().with_width(0.5).with_color(color));
        x += spacing;
    }

    let mut y = 0.0;
    while y <= size.height {
        let path = Path::line(Point::new(0.0, y), Point::new(size.width, y));
        frame.stroke(&path, Stroke::default().with_width(0.5).with_color(color));
        y += spacing;
    }
}

fn transform(p: Point, offset: Vector, zoom: f32) -> Point {
    Point::new(p.x * zoom + offset.x, p.y * zoom + offset.y)
}

fn table_by_name<'a>(tables: &'a [Table], name: &str) -> &'a Table {
    tables.iter().find(|t| t.name == name).unwrap_or(&tables[0])
}

fn table_height(table: &Table) -> f32 {
    HEADER_HEIGHT + table.columns.len() as f32 * ROW_HEIGHT
}

/// Lay tables out on a simple grid, sized so wide schemas wrap into rows.
fn layout_tables(tables: &[Table]) -> HashMap<String, Point> {
    let mut map = HashMap::new();
    let columns = (tables.len() as f32).sqrt().ceil().max(1.0) as usize;
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut row_max_height = 0.0f32;
    for (i, table) in tables.iter().enumerate() {
        if i > 0 && i % columns == 0 {
            x = 0.0;
            y += row_max_height + V_GAP;
            row_max_height = 0.0;
        }
        map.insert(table.name.clone(), Point::new(x, y));
        let h = table_height(table);
        row_max_height = row_max_height.max(h);
        x += BOX_WIDTH + H_GAP;
    }
    map
}

#[allow(clippy::too_many_arguments)]
fn draw_table(
    frame: &mut Frame,
    table: &Table,
    pos: Point,
    offset: Vector,
    zoom: f32,
    palette: &iced::theme::palette::Extended,
) {
    let origin = transform(pos, offset, zoom);
    let height = table_height(table) * zoom;
    let width = BOX_WIDTH * zoom;

    // Box background + border.
    let box_path = Path::rounded_rectangle(origin, Size::new(width, height), 6.0.into());
    frame.fill(&box_path, palette.background.weak.color);
    frame.stroke(
        &box_path,
        Stroke::default()
            .with_width(1.5)
            .with_color(palette.primary.strong.color),
    );

    // Header.
    let header_path =
        Path::rounded_rectangle(origin, Size::new(width, HEADER_HEIGHT * zoom), 6.0.into());
    frame.fill(&header_path, palette.primary.strong.color);
    frame.fill_rectangle(
        Point::new(origin.x, origin.y + (HEADER_HEIGHT * zoom / 2.0)),
        Size::new(width, HEADER_HEIGHT * zoom / 2.0),
        palette.primary.strong.color,
    );
    frame.fill_text(Text {
        content: table.name.clone(),
        position: Point::new(origin.x + 8.0 * zoom, origin.y + 6.0 * zoom),
        color: palette.primary.strong.text,
        size: (13.0 * zoom).into(),
        font: iced::Font::MONOSPACE,
        align_x: iced::widget::text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        ..Text::default()
    });

    // Columns.
    for (i, col) in table.columns.iter().enumerate() {
        let y = origin.y + (HEADER_HEIGHT + i as f32 * ROW_HEIGHT) * zoom;
        let marker = if col.primary_key {
            "PK "
        } else if col.references.is_some() {
            "FK "
        } else {
            "   "
        };
        let label = format!("{marker}{} : {}", col.name, col.type_name);
        frame.fill_text(Text {
            content: label,
            position: Point::new(origin.x + 6.0 * zoom, y + 3.0 * zoom),
            color: palette.background.base.text,
            size: (11.0 * zoom).into(),
            font: iced::Font::MONOSPACE,
            align_x: iced::widget::text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            ..Text::default()
        });
    }
}
