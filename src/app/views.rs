//! Rendering for the DecentDB Studio UI.
//!
//! Composes the top toolbar, the schema sidebar, the central work area (which
//! switches on [`Panel`]), the status bar, and the modal conversion dialog.

use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_editor, text_input,
    Space,
};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding, Theme};

use decentdb_studio::db::{value, ResultSet};
use decentdb_studio::export::Format;
use decentdb_studio::theme::AppTheme;

use super::{erd, Message, Panel, SidebarGroup, Studio};

const MONO: Font = Font::MONOSPACE;

/// A flexible horizontal spacer that fills available width.
fn horizontal_space() -> Space {
    Space::new().width(Length::Fill)
}

/// A horizontal divider line.
fn horizontal_rule<'a>(height: u16) -> Element<'a, Message> {
    rule::horizontal(height as f32).into()
}

/// A vertical divider line.
fn vertical_rule<'a>(width: u16) -> Element<'a, Message> {
    rule::vertical(width as f32).into()
}

/// A fixed-height vertical spacer.
fn vspace<'a>(height: f32) -> Element<'a, Message> {
    Space::new().height(Length::Fixed(height)).into()
}

/// A fixed-width horizontal spacer.
fn hspace<'a>(width: f32) -> Element<'a, Message> {
    Space::new().width(Length::Fixed(width)).into()
}
const TABLE_ICON: &str = "▦";
const VIEW_ICON: &str = "◫";
const INDEX_ICON: &str = "⌗";
const TRIGGER_ICON: &str = "⚡";

/// Build the entire window contents.
pub fn root(app: &Studio) -> Element<'_, Message> {
    let body = if app.convert.open {
        convert_dialog(app)
    } else {
        main_layout(app)
    };

    column![toolbar(app), horizontal_rule(1), body, status_bar(app)]
        .spacing(0)
        .into()
}

// ----------------------------------------------------------------------------
// Toolbar
// ----------------------------------------------------------------------------

fn toolbar(app: &Studio) -> Element<'_, Message> {
    let connected = app.connection.is_some();

    let recent: Vec<RecentEntry> = app
        .settings
        .recent_files
        .iter()
        .map(|p| RecentEntry(p.clone()))
        .collect();

    let recent_picker = pick_list(recent, None::<RecentEntry>, |entry| {
        Message::OpenRecent(entry.0)
    })
    .placeholder("Recent…")
    .width(Length::Fixed(150.0));

    let theme_picker = pick_list(AppTheme::ALL.to_vec(), Some(app.settings.theme), Message::ThemeChanged)
        .width(Length::Fixed(160.0));

    let mut left = row![
        tool_button("Open", Message::OpenDatabaseDialog),
        tool_button("New", Message::NewDatabaseDialog),
        tool_button("Memory", Message::OpenInMemory),
        recent_picker,
        vertical_rule(1),
        tool_button("Convert SQLite", Message::OpenConvertDialog),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    if connected {
        left = left.push(vertical_rule(1));
        left = left.push(accent_button("Run  ▶", Message::RunQuery));
        left = left.push(tool_button("Explain", Message::ExplainCurrent));
        left = left.push(tool_button("Format", Message::FormatSql));
        left = left.push(tool_button("Checkpoint", Message::Checkpoint));
        left = left.push(tool_button("Close", Message::CloseDatabase));
    }

    container(
        row![
            left,
            horizontal_space(),
            text("Theme").size(13),
            theme_picker,
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([6, 10]))
    .width(Length::Fill)
    .into()
}

fn tool_button(label: &str, msg: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding(Padding::from([5, 10]))
        .style(button::secondary)
        .on_press(msg)
        .into()
}

fn accent_button(label: &str, msg: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding(Padding::from([5, 12]))
        .style(button::primary)
        .on_press(msg)
        .into()
}

// ----------------------------------------------------------------------------
// Main layout: sidebar + work area
// ----------------------------------------------------------------------------

fn main_layout(app: &Studio) -> Element<'_, Message> {
    if app.connection.is_none() {
        return welcome(app);
    }

    row![
        sidebar(app),
        vertical_rule(1),
        work_area(app),
    ]
    .height(Length::Fill)
    .into()
}

fn welcome(app: &Studio) -> Element<'_, Message> {
    let recents: Element<Message> = if app.settings.recent_files.is_empty() {
        text("No recent databases").size(14).into()
    } else {
        let mut col = column![text("Recent databases").size(16)].spacing(6);
        for path in &app.settings.recent_files {
            col = col.push(
                button(text(path).size(13).font(MONO))
                    .style(button::text)
                    .on_press(Message::OpenRecent(path.clone())),
            );
        }
        col.into()
    };

    container(
        column![
            text("DecentDB Studio").size(34),
            text("A cross-platform client & administration tool for DecentDB").size(15),
            text(format!("Engine: DecentDB v{}", decentdb::version())).size(13),
            vspace(16.0),
            row![
                accent_button("Open database", Message::OpenDatabaseDialog),
                tool_button("Create new", Message::NewDatabaseDialog),
                tool_button("In-memory", Message::OpenInMemory),
                tool_button("Convert SQLite → DecentDB", Message::OpenConvertDialog),
            ]
            .spacing(10),
            vspace(24.0),
            recents,
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

// ----------------------------------------------------------------------------
// Sidebar
// ----------------------------------------------------------------------------

fn sidebar(app: &Studio) -> Element<'_, Message> {
    let schema = &app.schema;
    let filter = app.sidebar_filter.to_lowercase();
    let matches = |name: &str| filter.is_empty() || name.to_lowercase().contains(&filter);

    let mut tree = column![].spacing(1).width(Length::Fill);

    // Tables
    tree = tree.push(group_header(
        TABLE_ICON,
        "Tables",
        schema.tables.len(),
        app.expanded.tables,
        SidebarGroup::Tables,
    ));
    if app.expanded.tables {
        for t in &schema.tables {
            if matches(&t.name) {
                let selected = app.selected_object.as_deref() == Some(t.name.as_str());
                tree = tree.push(object_row(
                    &t.name,
                    &format!("{} cols · {} rows", t.columns.len(), t.row_count),
                    selected,
                ));
            }
        }
    }

    // Views
    tree = tree.push(group_header(
        VIEW_ICON,
        "Views",
        schema.views.len(),
        app.expanded.views,
        SidebarGroup::Views,
    ));
    if app.expanded.views {
        for v in &schema.views {
            if matches(&v.name) {
                let selected = app.selected_object.as_deref() == Some(v.name.as_str());
                tree = tree.push(object_row(&v.name, "view", selected));
            }
        }
    }

    // Indexes
    tree = tree.push(group_header(
        INDEX_ICON,
        "Indexes",
        schema.indexes.len(),
        app.expanded.indexes,
        SidebarGroup::Indexes,
    ));
    if app.expanded.indexes {
        for i in &schema.indexes {
            if matches(&i.name) {
                tree = tree.push(info_row(&i.name, &format!("on {}", i.table_name)));
            }
        }
    }

    // Triggers
    tree = tree.push(group_header(
        TRIGGER_ICON,
        "Triggers",
        schema.triggers.len(),
        app.expanded.triggers,
        SidebarGroup::Triggers,
    ));
    if app.expanded.triggers {
        for t in &schema.triggers {
            if matches(&t.name) {
                tree = tree.push(info_row(&t.name, &t.target));
            }
        }
    }

    let filter_box = text_input("Filter objects…", &app.sidebar_filter)
        .on_input(Message::SidebarFilterChanged)
        .size(13)
        .padding(6);

    container(
        column![
            filter_box,
            scrollable(tree).height(Length::Fill).width(Length::Fill),
        ]
        .spacing(6),
    )
    .padding(8)
    .width(Length::Fixed(260.0))
    .height(Length::Fill)
    .into()
}

fn group_header<'a>(
    icon: &'a str,
    label: &'a str,
    count: usize,
    expanded: bool,
    group: SidebarGroup,
) -> Element<'a, Message> {
    let chevron = if expanded { "▾" } else { "▸" };
    button(
        row![
            text(chevron).size(11),
            text(icon).size(13),
            text(label).size(13),
            horizontal_space(),
            text(count.to_string()).size(12),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(Padding::from([4, 6]))
    .style(button::text)
    .on_press(Message::ToggleGroup(group))
    .into()
}

fn object_row<'a>(name: &'a str, detail: &str, selected: bool) -> Element<'a, Message> {
    let label = column![
        text(name.to_string()).size(13),
        text(detail.to_string()).size(10).style(text::secondary),
    ]
    .spacing(1);

    let style = if selected {
        button::primary
    } else {
        button::text
    };

    button(row![hspace(14.0), label].spacing(4))
        .width(Length::Fill)
        .padding(Padding::from([3, 6]))
        .style(style)
        .on_press(Message::SelectObject(name.to_string()))
        .into()
}

fn info_row<'a>(name: &'a str, detail: &str) -> Element<'a, Message> {
    container(
        row![
            hspace(20.0),
            text(name.to_string()).size(12),
            horizontal_space(),
            text(detail.to_string()).size(10).style(text::secondary),
        ]
        .spacing(4),
    )
    .padding(Padding::from([3, 6]))
    .width(Length::Fill)
    .into()
}

// ----------------------------------------------------------------------------
// Work area (panel switch)
// ----------------------------------------------------------------------------

fn work_area(app: &Studio) -> Element<'_, Message> {
    column![panel_tabs(app), horizontal_rule(1), panel_body(app)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn panel_tabs(app: &Studio) -> Element<'_, Message> {
    let mut tabs = row![].spacing(2).align_y(Alignment::Center);
    for panel in Panel::ALL {
        let active = app.panel == *panel;
        let style = if active {
            button::primary
        } else {
            button::text
        };
        tabs = tabs.push(
            button(text(panel.label()).size(13))
                .padding(Padding::from([5, 12]))
                .style(style)
                .on_press(Message::SelectPanel(*panel)),
        );
    }
    container(tabs).padding(Padding::from([4, 8])).into()
}

fn panel_body(app: &Studio) -> Element<'_, Message> {
    match app.panel {
        Panel::Query => query_panel(app),
        Panel::Data => data_panel(app),
        Panel::Explain => explain_panel(app),
        Panel::Erd => erd::view(app),
        Panel::Structure => structure_panel(app),
        Panel::Dashboard => dashboard_panel(app),
    }
}

// ----------------------------------------------------------------------------
// Query panel: editor + results
// ----------------------------------------------------------------------------

fn query_panel(app: &Studio) -> Element<'_, Message> {
    let editor = text_editor(&app.editor)
        .placeholder("Write SQL here… (Run with the toolbar button)")
        .on_action(Message::EditorAction)
        .font(MONO)
        .size(app.settings.editor_font_size as f32)
        .height(Length::FillPortion(2))
        .padding(10)
        .highlight("sql", app.settings.theme.highlighter_theme());

    // Build a "SELECT * FROM <selected>" snippet when a table is selected.
    let select_snippet = app
        .selected_object
        .as_ref()
        .and_then(|name| app.schema.table(name))
        .map(|t| {
            format!(
                "SELECT *\nFROM {}\nLIMIT 100;",
                decentdb_studio::db::quote_ident(&t.name)
            )
        });

    let mut editor_bar = row![
        accent_button("Run  ▶", Message::RunQuery),
        tool_button("Explain", Message::ExplainCurrent),
        tool_button("Format", Message::FormatSql),
        tool_button("Clear", Message::ClearEditor),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    if let Some(snippet) = select_snippet {
        editor_bar = editor_bar.push(tool_button("Select template", Message::InsertSnippet(snippet)));
    }

    let editor_bar = editor_bar
        .push(horizontal_space())
        .push(text("Ctrl+Enter to run").size(11).style(text::secondary));

    let results = results_view(app);

    column![
        container(editor).padding(6).height(Length::FillPortion(2)),
        completion_bar(app),
        container(editor_bar).padding(Padding::from([0, 6])),
        horizontal_rule(1),
        container(results)
            .padding(6)
            .height(Length::FillPortion(3))
            .width(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

/// A small live hint of completion candidates for the current word.
/// Interactive autocompletion bar: clickable chips that insert the chosen
/// identifier/keyword into the editor at the cursor.
fn completion_bar(app: &Studio) -> Element<'_, Message> {
    let candidates = app.completion_candidates();
    if candidates.is_empty() {
        return Space::new().into();
    }
    let mut chips = row![text("⌨").size(12).style(text::secondary)]
        .spacing(4)
        .align_y(Alignment::Center);
    for cand in candidates {
        chips = chips.push(
            button(text(cand.clone()).size(12).font(MONO))
                .padding(Padding::from([2, 8]))
                .style(button::secondary)
                .on_press(Message::ApplyCompletion(cand)),
        );
    }
    container(scrollable(chips).direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::default(),
    )))
    .padding(Padding::from([2, 6]))
    .width(Length::Fill)
    .into()
}

fn results_view(app: &Studio) -> Element<'_, Message> {
    if app.results.is_empty() {
        return container(text("Run a query to see results").style(text::secondary))
            .center_x(Length::Fill)
            .padding(20)
            .into();
    }

    // Result tabs (one per statement).
    let mut tabs = row![].spacing(2);
    for (i, rs) in app.results.iter().enumerate() {
        let active = i == app.active_result;
        let label = if rs.is_query {
            format!("Result {} ({} rows)", i + 1, rs.rows.len())
        } else {
            format!("Result {} ({} affected)", i + 1, rs.affected_rows)
        };
        tabs = tabs.push(
            button(text(label).size(12))
                .padding(Padding::from([3, 8]))
                .style(if active { button::primary } else { button::text })
                .on_press(Message::SelectResultTab(i)),
        );
    }

    let active = &app.results[app.active_result];
    let export_row = row![
        text(format!("{:.2} ms", active.elapsed_ms)).size(12).style(text::secondary),
        horizontal_space(),
        pick_list(Format::ALL.to_vec(), Some(app.export_format), Message::ExportFormatChanged),
        tool_button("Export", Message::ExportResults),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let grid: Element<Message> = if active.is_query {
        result_grid(active)
    } else {
        container(
            text(format!("{} row(s) affected", active.affected_rows)).size(14),
        )
        .padding(16)
        .into()
    };

    column![tabs, export_row, horizontal_rule(1), grid]
        .spacing(6)
        .height(Length::Fill)
        .into()
}

// ----------------------------------------------------------------------------
// Result grid
// ----------------------------------------------------------------------------

/// Render a result set as a scrollable grid with a sticky header.
fn result_grid(rs: &ResultSet) -> Element<'_, Message> {
    if rs.columns.is_empty() {
        return text("No columns").into();
    }
    let col_width = Length::Fixed(160.0);

    // Header row.
    let mut header = row![row_number_cell("#", true)].spacing(0);
    for col in &rs.columns {
        header = header.push(grid_header_cell(col, col_width));
    }

    // Data rows.
    let mut body = column![].spacing(0);
    for (ri, r) in rs.rows.iter().enumerate() {
        let mut line = row![row_number_cell(&(ri + 1).to_string(), false)].spacing(0);
        for cell in r {
            let is_null = value::is_null(cell);
            let display = if is_null {
                "NULL".to_string()
            } else {
                let s = value::display(cell);
                truncate(&s, 200)
            };
            line = line.push(grid_data_cell(display, is_null, col_width, ri));
        }
        body = body.push(line);
    }

    let table = column![
        container(header),
        scrollable(body).height(Length::Fill).width(Length::Fill),
    ];

    scrollable(table)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn grid_header_cell(label: &str, width: Length) -> Element<'_, Message> {
    container(text(label.to_string()).size(12).font(MONO))
        .padding(Padding::from([5, 8]))
        .width(width)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(palette.background.strong.color)),
                text_color: Some(palette.background.strong.text),
                border: Border {
                    color: palette.background.weak.color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

fn grid_data_cell<'a>(
    content: String,
    is_null: bool,
    width: Length,
    row_index: usize,
) -> Element<'a, Message> {
    let mut label = text(content).size(12).font(MONO);
    if is_null {
        label = label.style(text::secondary);
    }
    container(label)
        .padding(Padding::from([4, 8]))
        .width(width)
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            let bg = if row_index % 2 == 0 {
                palette.background.base.color
            } else {
                palette.background.weak.color
            };
            container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: palette.background.weak.color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

fn row_number_cell<'a>(label: &str, header: bool) -> Element<'a, Message> {
    let t = text(label.to_string()).size(11).font(MONO);
    container(t)
        .padding(Padding::from([4, 6]))
        .width(Length::Fixed(46.0))
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            let bg = if header {
                palette.background.strong.color
            } else {
                palette.background.weak.color
            };
            container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(palette.background.strong.text),
                border: Border {
                    color: palette.background.weak.color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

// ----------------------------------------------------------------------------
// Data browser panel
// ----------------------------------------------------------------------------

fn data_panel(app: &Studio) -> Element<'_, Message> {
    let Some(table) = &app.selected_object else {
        return container(text("Select a table in the sidebar to browse its data").style(text::secondary))
            .center_x(Length::Fill)
            .padding(20)
            .into();
    };

    let editable = app
        .schema
        .table(table)
        .map(|t| !t.primary_key_columns.is_empty())
        .unwrap_or(false);

    let mut header = row![
        text(format!("{TABLE_ICON} {table}")).size(16),
        horizontal_space(),
        tool_button("Add row", Message::BeginAddRow),
        tool_button("Refresh", Message::RefreshData),
        tool_button("◀ Prev", Message::BrowsePrevPage),
        text(format!("Page {}", app.browse_page + 1)).size(13),
        tool_button("Next ▶", Message::BrowseNextPage),
        pick_list(Format::ALL.to_vec(), Some(app.export_format), Message::ExportFormatChanged),
        tool_button("Export", Message::ExportResults),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if !editable {
        header = header.push(
            text("read-only (no PK)").size(11).style(text::secondary),
        );
    }

    let grid: Element<Message> = match &app.browse {
        Some(rs) if rs.is_query => editable_grid(app, rs, editable),
        Some(_) => text("No rows").into(),
        None => text("Loading…").style(text::secondary).into(),
    };

    let mut col = column![container(header).padding(8), horizontal_rule(1)];

    // New-row draft editor, if active.
    if let Some(draft) = &app.new_row {
        col = col.push(new_row_editor(draft));
        col = col.push(horizontal_rule(1));
    }

    col = col.push(container(grid).padding(6).height(Length::Fill));
    col.height(Length::Fill).into()
}

/// Editor strip for adding a new row.
fn new_row_editor(draft: &[(String, String)]) -> Element<'_, Message> {
    let mut fields = row![text("New row:").size(13)].spacing(6).align_y(Alignment::Center);
    for (i, (name, value)) in draft.iter().enumerate() {
        fields = fields.push(
            text_input(name, value)
                .on_input(move |t| Message::NewRowChanged(i, t))
                .size(12)
                .width(Length::Fixed(120.0))
                .padding(4),
        );
    }
    fields = fields.push(horizontal_space());
    fields = fields.push(accent_button("Insert", Message::CommitNewRow));
    fields = fields.push(tool_button("Cancel", Message::CancelNewRow));
    container(fields)
        .padding(8)
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(palette.background.weak.color)),
                ..container::Style::default()
            }
        })
        .into()
}

/// Render an editable data grid for the data browser.
fn editable_grid<'a>(app: &'a Studio, rs: &'a ResultSet, editable: bool) -> Element<'a, Message> {
    if rs.columns.is_empty() {
        return text("No columns").into();
    }
    let col_width = Length::Fixed(160.0);

    let mut header = row![row_number_cell("#", true)].spacing(0);
    if editable {
        header = header.push(grid_header_cell("", Length::Fixed(60.0)));
    }
    for col in &rs.columns {
        header = header.push(grid_header_cell(col, col_width));
    }

    let mut body = column![].spacing(0);
    for (ri, r) in rs.rows.iter().enumerate() {
        let mut line = row![row_number_cell(&(ri + 1).to_string(), false)].spacing(0);
        if editable {
            line = line.push(
                container(
                    button(text("✕").size(11))
                        .style(button::danger)
                        .padding(Padding::from([2, 6]))
                        .on_press(Message::DeleteRow(ri)),
                )
                .padding(Padding::from([2, 4]))
                .width(Length::Fixed(60.0)),
            );
        }
        for (ci, cell) in r.iter().enumerate() {
            let editing_here = app
                .editing
                .as_ref()
                .map(|e| e.row == ri && e.col == ci)
                .unwrap_or(false);

            if editing_here {
                let draft = app.editing.as_ref().map(|e| e.draft.clone()).unwrap_or_default();
                line = line.push(
                    container(
                        text_input("", &draft)
                            .on_input(Message::EditChanged)
                            .on_submit(Message::CommitEdit)
                            .size(12)
                            .padding(3),
                    )
                    .width(col_width)
                    .padding(1),
                );
            } else {
                let is_null = value::is_null(cell);
                let display = if is_null {
                    "NULL".to_string()
                } else {
                    truncate(&value::display(cell), 200)
                };
                let raw = if is_null { String::new() } else { value::display(cell) };
                if editable {
                    line = line.push(editable_cell(display, is_null, raw, col_width, ri, ci));
                } else {
                    line = line.push(grid_data_cell(display, is_null, col_width, ri));
                }
            }
        }
        body = body.push(line);
    }

    let table = column![container(header), scrollable(body).height(Length::Fill).width(Length::Fill)];

    scrollable(table)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// A clickable data cell that begins editing when pressed.
fn editable_cell<'a>(
    content: String,
    is_null: bool,
    raw: String,
    width: Length,
    row_index: usize,
    col_index: usize,
) -> Element<'a, Message> {
    let mut label = text(content).size(12).font(MONO);
    if is_null {
        label = label.style(text::secondary);
    }
    button(label)
        .style(move |theme: &Theme, _status| {
            let palette = theme.extended_palette();
            let bg = if row_index % 2 == 0 {
                palette.background.base.color
            } else {
                palette.background.weak.color
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: palette.background.base.text,
                border: Border {
                    color: palette.background.weak.color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..button::Style::default()
            }
        })
        .padding(Padding::from([4, 8]))
        .width(width)
        .on_press(Message::BeginEdit(row_index, col_index, raw))
        .into()
}

// ----------------------------------------------------------------------------
// Explain panel
// ----------------------------------------------------------------------------

fn explain_panel(app: &Studio) -> Element<'_, Message> {
    if app.explain_lines.is_empty() {
        return container(
            text("Run \"Explain\" on a statement to see its query plan").style(text::secondary),
        )
        .center_x(Length::Fill)
        .padding(20)
        .into();
    }

    let mut col = column![text("Query plan").size(16)].spacing(2);
    for line in &app.explain_lines {
        // Indent visually based on leading whitespace already present.
        col = col.push(
            container(text(line.clone()).size(13).font(MONO))
                .padding(Padding::from([2, 8]))
                .width(Length::Fill),
        );
    }

    scrollable(container(col).padding(12))
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

// ----------------------------------------------------------------------------
// Structure panel (DDL + columns)
// ----------------------------------------------------------------------------

fn structure_panel(app: &Studio) -> Element<'_, Message> {
    let Some(name) = &app.selected_object else {
        return container(text("Select an object to inspect its structure").style(text::secondary))
            .center_x(Length::Fill)
            .padding(20)
            .into();
    };

    if let Some(table) = app.schema.table(name) {
        let mut cols = column![
            row![
                struct_head("Column", 200.0),
                struct_head("Type", 140.0),
                struct_head("Null", 60.0),
                struct_head("Key", 60.0),
                struct_head("Default", 160.0),
            ]
            .spacing(0)
        ]
        .spacing(0);

        for c in &table.columns {
            let key = if c.primary_key {
                "PK"
            } else if c.references.is_some() {
                "FK"
            } else if c.unique {
                "UQ"
            } else {
                ""
            };
            cols = cols.push(
                row![
                    struct_cell(&c.name, 200.0),
                    struct_cell(&c.type_name, 140.0),
                    struct_cell(if c.nullable { "YES" } else { "NO" }, 60.0),
                    struct_cell(key, 60.0),
                    struct_cell(c.default_sql.as_deref().unwrap_or(""), 160.0),
                ]
                .spacing(0),
            );
        }

        let fks: Element<Message> = if table.foreign_keys.is_empty() {
            text("None").size(13).style(text::secondary).into()
        } else {
            let mut fk_col = column![].spacing(2);
            for fk in &table.foreign_keys {
                fk_col = fk_col.push(
                    text(format!(
                        "{} → {} ({})",
                        fk.columns.join(", "),
                        fk.referenced_table,
                        fk.referenced_columns.join(", ")
                    ))
                    .size(13)
                    .font(MONO),
                );
            }
            fk_col.into()
        };

        return scrollable(
            container(
                column![
                    text(format!("{TABLE_ICON} {}", table.name)).size(18),
                    text(format!("{} rows", table.row_count)).size(13).style(text::secondary),
                    vspace(8.0),
                    text("Columns").size(15),
                    cols,
                    vspace(12.0),
                    text("Foreign keys").size(15),
                    fks,
                    vspace(12.0),
                    text("DDL").size(15),
                    ddl_box(&table.ddl),
                ]
                .spacing(6),
            )
            .padding(14),
        )
        .height(Length::Fill)
        .into();
    }

    if let Some(view) = app.schema.views.iter().find(|v| &v.name == name) {
        return scrollable(
            container(
                column![
                    text(format!("{VIEW_ICON} {}", view.name)).size(18),
                    text(format!("Columns: {}", view.columns.join(", "))).size(13),
                    vspace(8.0),
                    text("Definition").size(15),
                    ddl_box(&view.sql_text),
                ]
                .spacing(6),
            )
            .padding(14),
        )
        .height(Length::Fill)
        .into();
    }

    container(text("Object not found").style(text::secondary))
        .padding(20)
        .into()
}

fn struct_head(label: &str, width: f32) -> Element<'_, Message> {
    container(text(label.to_string()).size(12))
        .padding(Padding::from([4, 8]))
        .width(Length::Fixed(width))
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(palette.background.strong.color)),
                text_color: Some(palette.background.strong.text),
                ..container::Style::default()
            }
        })
        .into()
}

fn struct_cell<'a>(label: &str, width: f32) -> Element<'a, Message> {
    container(text(label.to_string()).size(12).font(MONO))
        .padding(Padding::from([3, 8]))
        .width(Length::Fixed(width))
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                border: Border {
                    color: palette.background.weak.color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

fn ddl_box(ddl: &str) -> Element<'_, Message> {
    container(text(ddl.to_string()).size(12).font(MONO))
        .padding(10)
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(palette.background.weak.color)),
                border: Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

// ----------------------------------------------------------------------------
// Dashboard panel
// ----------------------------------------------------------------------------

fn dashboard_panel(app: &Studio) -> Element<'_, Message> {
    let Some(conn) = &app.connection else {
        return text("No database open").into();
    };

    let mut cards = column![].spacing(10);
    cards = cards.push(text(format!("Database: {}", conn.display_name())).size(20));
    cards = cards.push(
        text(format!("Path: {}", conn.path().display()))
            .size(13)
            .font(MONO)
            .style(text::secondary),
    );
    cards = cards.push(text(format!("DecentDB engine v{}", decentdb::version())).size(13));

    let stats: Element<Message> = match conn.storage_info() {
        Ok(info) => {
            column![
                stat_card("Format version", info.format_version.to_string()),
                stat_card("Page size", format!("{} bytes", info.page_size)),
                stat_card("Page count", info.page_count.to_string()),
                stat_card("Cache size", format!("{} MB", info.cache_size_mb)),
                stat_card("WAL file size", format!("{} bytes", info.wal_file_size)),
                stat_card("Last checkpoint LSN", info.last_checkpoint_lsn.to_string()),
                stat_card("Active readers", info.active_readers.to_string()),
            ]
            .spacing(6)
            .into()
        }
        Err(e) => text(format!("Storage info unavailable: {e}"))
            .style(text::danger)
            .into(),
    };

    let counts = row![
        count_card(TABLE_ICON, "Tables", app.schema.tables.len()),
        count_card(VIEW_ICON, "Views", app.schema.views.len()),
        count_card(INDEX_ICON, "Indexes", app.schema.indexes.len()),
        count_card(TRIGGER_ICON, "Triggers", app.schema.triggers.len()),
    ]
    .spacing(10);

    let migrate = column![
        text("Migrate / export database").size(15),
        row![
            tool_button("Export → SQLite", Message::ExportDatabaseSqlite),
            tool_button("Export → SQL dump", Message::ExportDatabaseDump),
            tool_button("Checkpoint WAL", Message::Checkpoint),
        ]
        .spacing(8),
    ]
    .spacing(6);

    scrollable(
        container(
            column![cards, vspace(10.0), counts, vspace(10.0), migrate, vspace(10.0), stats]
                .spacing(8),
        )
        .padding(16),
    )
    .height(Length::Fill)
    .into()
}

fn stat_card(label: &str, value: String) -> Element<'_, Message> {
    row![
        text(label.to_string()).size(13).width(Length::Fixed(180.0)),
        text(value).size(13).font(MONO),
    ]
    .spacing(8)
    .into()
}

fn count_card<'a>(icon: &'a str, label: &'a str, count: usize) -> Element<'a, Message> {
    container(
        column![
            text(format!("{icon} {count}")).size(24),
            text(label.to_string()).size(13).style(text::secondary),
        ]
        .align_x(Alignment::Center)
        .spacing(4),
    )
    .padding(16)
    .width(Length::Fixed(120.0))
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style {
            background: Some(Background::Color(palette.background.weak.color)),
            border: Border {
                color: palette.primary.weak.color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        }
    })
    .into()
}

// ----------------------------------------------------------------------------
// Conversion dialog
// ----------------------------------------------------------------------------

fn convert_dialog(app: &Studio) -> Element<'_, Message> {
    let cv = &app.convert;

    let source_label = cv
        .source
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "No file selected".to_string());
    let target_label = cv
        .target
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "No file selected".to_string());

    let mut log = column![].spacing(2);
    for line in &cv.log {
        log = log.push(text(line.clone()).size(12).font(MONO));
    }

    let run_button: Element<Message> = if cv.running {
        button(text("Converting…").size(14))
            .padding(Padding::from([6, 16]))
            .style(button::secondary)
            .into()
    } else {
        accent_button("Start conversion", Message::RunConversion)
    };

    let body = column![
        text("Convert SQLite → DecentDB").size(22),
        text(
            "Reads a SQLite database and rebuilds it in DecentDB, mapping SQLite \
             affinities onto DecentDB native types (BOOL, TIMESTAMP, UUID, DATE, \
             DECIMAL, BLOB, …) and recreating indexes."
        )
        .size(13)
        .style(text::secondary),
        vspace(8.0),
        row![
            text("Source SQLite:").size(13).width(Length::Fixed(120.0)),
            text(source_label).size(13).font(MONO).width(Length::Fill),
            tool_button("Choose…", Message::PickConvertSource),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            text("Target DecentDB:").size(13).width(Length::Fixed(120.0)),
            text(target_label).size(13).font(MONO).width(Length::Fill),
            tool_button("Choose…", Message::PickConvertTarget),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        vspace(8.0),
        row![run_button, tool_button("Close", Message::CloseConvertDialog)].spacing(10),
        vspace(8.0),
        text("Log").size(15),
        container(scrollable(log).height(Length::Fill))
            .padding(10)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(Background::Color(palette.background.weak.color)),
                    border: Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                }
            }),
    ]
    .spacing(8);

    container(body)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ----------------------------------------------------------------------------
// Status bar
// ----------------------------------------------------------------------------

fn status_bar(app: &Studio) -> Element<'_, Message> {
    let (msg, danger) = match &app.status {
        Some(s) => (s.text.clone(), s.error),
        None => (
            match &app.connection {
                Some(c) => format!("Connected: {}", c.display_name()),
                None => "Ready".to_string(),
            },
            false,
        ),
    };

    let label = if danger {
        text(msg).size(12).style(text::danger)
    } else {
        text(msg).size(12)
    };

    // The message area is a borderless button so clicking it dismisses the
    // current status notification.
    let dismissible = button(label)
        .style(button::text)
        .padding(0)
        .on_press(Message::DismissStatus);

    container(
        row![
            dismissible,
            horizontal_space(),
            text(format!("DecentDB Studio · engine v{}", decentdb::version()))
                .size(11)
                .style(text::secondary),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([4, 10]))
    .width(Length::Fill)
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style {
            background: Some(Background::Color(palette.background.weak.color)),
            ..container::Style::default()
        }
    })
    .into()
}

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

/// Wrapper so recent-file paths can be used in a `pick_list`.
#[derive(Debug, Clone, PartialEq)]
struct RecentEntry(String);

impl std::fmt::Display for RecentEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = std::path::Path::new(&self.0)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.0.clone());
        write!(f, "{name}")
    }
}

fn truncate(s: &str, max: usize) -> String {
    let one_line = s.replace('\n', " ").replace('\r', "");
    if one_line.chars().count() > max {
        let truncated: String = one_line.chars().take(max).collect();
        format!("{truncated}…")
    } else {
        one_line
    }
}




