//! The DecentDB Studio iced application.
//!
//! This module owns the top-level [`Studio`] state, the [`Message`] enum, and
//! the `update`/`view` orchestration. Rendering of individual panels lives in
//! submodules ([`views`], [`erd`]).

mod erd;
mod views;

use std::path::PathBuf;

use iced::widget::text_editor;
use iced::{Element, Task, Theme};

use decentdb_studio::convert::{self, ConvertOptions, ConvertReport};
use decentdb_studio::db::{Connection, ResultSet, Schema};
use decentdb_studio::export::Format;
use decentdb_studio::settings::Settings;
use decentdb_studio::theme::AppTheme;

/// The central panel currently shown in the main work area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// SQL editor + results.
    Query,
    /// Tabular data browser for the selected table.
    Data,
    /// EXPLAIN plan for the current statement.
    Explain,
    /// Entity-relationship diagram.
    Erd,
    /// Object details / DDL for the selected object.
    Structure,
    /// Database dashboard (storage stats, engine info).
    Dashboard,
}

impl Panel {
    pub const ALL: &'static [Panel] = &[
        Panel::Query,
        Panel::Data,
        Panel::Explain,
        Panel::Structure,
        Panel::Erd,
        Panel::Dashboard,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Panel::Query => "Query",
            Panel::Data => "Data",
            Panel::Explain => "Explain",
            Panel::Erd => "Diagram",
            Panel::Structure => "Structure",
            Panel::Dashboard => "Dashboard",
        }
    }
}

/// A transient toast/status message with a severity.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub error: bool,
}

/// SQL keywords offered by the editor's autocompletion and used by the syntax
/// chips. Kept in one place so the editor and completion logic stay in sync.
pub(crate) const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "DELETE", "CREATE", "TABLE",
    "INDEX", "VIEW", "TRIGGER", "DROP", "ALTER", "JOIN", "INNER", "LEFT", "RIGHT", "OUTER", "ON",
    "GROUP", "ORDER", "BY", "HAVING", "LIMIT", "OFFSET", "DISTINCT", "AS", "AND", "OR", "NOT",
    "NULL", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "UNIQUE", "DEFAULT", "EXPLAIN", "BEGIN",
    "COMMIT", "ROLLBACK", "INT64", "TEXT", "BOOL", "FLOAT64", "TIMESTAMP", "UUID", "DECIMAL",
    "DATE", "BLOB", "CASCADE", "BETWEEN", "LIKE", "IN", "IS", "CASE", "WHEN", "THEN", "ELSE", "END",
    "COUNT", "SUM", "AVG", "MIN", "MAX", "COALESCE",
];

/// The state of an in-flight or completed SQLite conversion.
#[derive(Debug, Clone, Default)]
pub struct ConvertState {
    /// Source SQLite path chosen by the user.
    pub source: Option<PathBuf>,
    /// Target DecentDB path chosen by the user.
    pub target: Option<PathBuf>,
    /// Whether a conversion is running.
    pub running: bool,
    /// Log lines emitted by the conversion.
    pub log: Vec<String>,
    /// The final report, if finished.
    pub report: Option<ConvertReport>,
    /// Whether the convert dialog is open.
    pub open: bool,
}

/// Top-level application state.
pub struct Studio {
    /// Persisted settings (theme, recents, editor prefs).
    pub settings: Settings,
    /// The active database connection, if any.
    pub connection: Option<Connection>,
    /// Cached schema for the open database.
    pub schema: Schema,
    /// The SQL editor content + cursor state.
    pub editor: text_editor::Content,
    /// The most recent query results (one tab per statement).
    pub results: Vec<ResultSet>,
    /// Index of the active result tab.
    pub active_result: usize,
    /// EXPLAIN plan lines for the current statement.
    pub explain_lines: Vec<String>,
    /// Which central panel is shown.
    pub panel: Panel,
    /// Currently selected schema object (table/view name).
    pub selected_object: Option<String>,
    /// Rows for the data browser.
    pub browse: Option<ResultSet>,
    /// Current data-browser page (0-based).
    pub browse_page: usize,
    /// Status bar message.
    pub status: Option<StatusMessage>,
    /// Set of expanded sidebar groups.
    pub expanded: SidebarExpansion,
    /// Conversion dialog/runner state.
    pub convert: ConvertState,
    /// Export format selected in the export menu.
    pub export_format: Format,
    /// Whether a query is currently executing.
    pub running: bool,
    /// Free-text filter for the sidebar object list.
    pub sidebar_filter: String,
    /// Entity-relationship diagram view state (pan/zoom).
    pub erd: erd::ErdState,
    /// In-progress inline cell edit in the data browser, if any.
    pub editing: Option<CellEdit>,
    /// In-progress "add row" draft values keyed by column name.
    pub new_row: Option<Vec<(String, String)>>,
}

/// State for an inline cell edit in the data browser.
#[derive(Debug, Clone)]
pub struct CellEdit {
    /// Row index within the current browse page.
    pub row: usize,
    /// Column index within the current browse page.
    pub col: usize,
    /// The current draft text in the edit box.
    pub draft: String,
}

/// Tracks which sidebar groups are expanded.
#[derive(Debug, Clone)]
pub struct SidebarExpansion {
    pub tables: bool,
    pub views: bool,
    pub indexes: bool,
    pub triggers: bool,
}

impl Default for SidebarExpansion {
    fn default() -> Self {
        Self {
            tables: true,
            views: true,
            indexes: false,
            triggers: false,
        }
    }
}

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    // ----- Connection lifecycle -----
    OpenDatabaseDialog,
    NewDatabaseDialog,
    DatabasePicked(Option<PathBuf>),
    DatabaseCreatePicked(Option<PathBuf>),
    OpenRecent(String),
    OpenInMemory,
    CloseDatabase,
    SchemaLoaded(Result<Schema, String>),

    // ----- Editor / query -----
    EditorAction(text_editor::Action),
    RunQuery,
    RunSelectionOrAll,
    QueryFinished(Result<Vec<ResultSet>, String>),
    ExplainCurrent,
    ExplainFinished(Result<Vec<String>, String>),
    FormatSql,
    ClearEditor,
    InsertSnippet(String),
    SelectResultTab(usize),
    /// Replace the partial word at the cursor with the given completion.
    ApplyCompletion(String),

    // ----- Navigation -----
    SelectPanel(Panel),
    SelectObject(String),
    ToggleGroup(SidebarGroup),
    SidebarFilterChanged(String),

    // ----- Data browser -----
    BrowseLoaded(Result<ResultSet, String>),
    BrowseNextPage,
    BrowsePrevPage,
    RefreshData,

    // ----- Data editing -----
    /// Begin editing the cell at (row, col) with its current text.
    BeginEdit(usize, usize, String),
    /// Update the in-progress edit draft text.
    EditChanged(String),
    /// Commit the in-progress edit (writes an UPDATE).
    CommitEdit,
    /// Cancel the in-progress edit.
    CancelEdit,
    /// Delete the row at the given page-row index.
    DeleteRow(usize),
    /// Begin adding a new row (open the draft editor).
    BeginAddRow,
    /// Update a draft column value for the new row.
    NewRowChanged(usize, String),
    /// Commit the new row (writes an INSERT).
    CommitNewRow,
    /// Cancel adding a new row.
    CancelNewRow,
    /// Result of a data-modifying operation (affected rows or error).
    EditApplied(Result<u64, String>),

    // ----- ERD -----
    ErdMessage(erd::ErdMessage),

    // ----- Export -----
    ExportFormatChanged(Format),
    ExportResults,
    ExportPicked(Option<PathBuf>, String),
    ExportDatabaseSqlite,
    ExportDatabaseSqlitePicked(Option<PathBuf>),
    ExportDatabaseDump,
    ExportDatabaseDumpPicked(Option<PathBuf>),

    // ----- Conversion -----
    OpenConvertDialog,
    CloseConvertDialog,
    PickConvertSource,
    ConvertSourcePicked(Option<PathBuf>),
    PickConvertTarget,
    ConvertTargetPicked(Option<PathBuf>),
    RunConversion,
    ConversionFinished(Result<ConvertReport, String>),

    // ----- Misc -----
    ThemeChanged(AppTheme),
    DismissStatus,
    Checkpoint,
    CheckpointDone(Result<(), String>),
    /// No-op (used for ignored keyboard events).
    Noop,
}

/// Identifies a collapsible sidebar group.
#[derive(Debug, Clone, Copy)]
pub enum SidebarGroup {
    Tables,
    Views,
    Indexes,
    Triggers,
}

impl Studio {
    /// Construct the initial application state.
    pub fn new() -> (Self, Task<Message>) {
        let settings = Settings::load();
        let studio = Self {
            settings,
            connection: None,
            schema: Schema::default(),
            editor: text_editor::Content::with_text(
                "-- Welcome to DecentDB Studio\n-- Open or create a database to begin.\n\nSELECT 1 AS hello;",
            ),
            results: Vec::new(),
            active_result: 0,
            explain_lines: Vec::new(),
            panel: Panel::Query,
            selected_object: None,
            browse: None,
            browse_page: 0,
            status: None,
            expanded: SidebarExpansion::default(),
            convert: ConvertState::default(),
            export_format: Format::Csv,
            running: false,
            sidebar_filter: String::new(),
            erd: erd::ErdState::default(),
            editing: None,
            new_row: None,
        };
        (studio, Task::none())
    }

    /// The window title.
    pub fn title(&self) -> String {
        match &self.connection {
            Some(conn) => format!("DecentDB Studio — {}", conn.display_name()),
            None => "DecentDB Studio".to_string(),
        }
    }

    /// The active iced theme.
    pub fn theme(&self) -> Theme {
        self.settings.theme.to_iced()
    }

    /// Handle a message and optionally schedule follow-up work.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),

            // ---------- Connection ----------
            Message::OpenDatabaseDialog => Task::perform(
                pick_open_database(),
                Message::DatabasePicked,
            ),
            Message::NewDatabaseDialog => {
                Task::perform(pick_new_database(), Message::DatabaseCreatePicked)
            }
            Message::DatabasePicked(Some(path)) | Message::DatabaseCreatePicked(Some(path)) => {
                self.open_connection(path)
            }
            Message::DatabasePicked(None) | Message::DatabaseCreatePicked(None) => Task::none(),
            Message::OpenRecent(path) => self.open_connection(PathBuf::from(path)),
            Message::OpenInMemory => match Connection::open_memory() {
                Ok(conn) => {
                    self.connection = Some(conn);
                    self.set_status("Opened in-memory database", false);
                    self.refresh_schema()
                }
                Err(e) => {
                    self.set_status(format!("Failed to open in-memory db: {e}"), true);
                    Task::none()
                }
            },
            Message::CloseDatabase => {
                self.connection = None;
                self.schema = Schema::default();
                self.results.clear();
                self.browse = None;
                self.selected_object = None;
                self.set_status("Database closed", false);
                Task::none()
            }
            Message::SchemaLoaded(Ok(schema)) => {
                self.schema = schema;
                Task::none()
            }
            Message::SchemaLoaded(Err(e)) => {
                self.set_status(format!("Schema load failed: {e}"), true);
                Task::none()
            }

            // ---------- Editor / query ----------
            Message::EditorAction(action) => {
                self.editor.perform(action);
                Task::none()
            }
            Message::RunQuery | Message::RunSelectionOrAll => self.run_query(),
            Message::QueryFinished(Ok(results)) => {
                self.running = false;
                let count = results.len();
                let rows: usize = results.iter().map(|r| r.rows.len()).sum();
                self.results = results;
                self.active_result = 0;
                self.panel = Panel::Query;
                self.set_status(
                    format!("{count} statement(s) executed; {rows} row(s) returned"),
                    false,
                );
                self.refresh_schema()
            }
            Message::QueryFinished(Err(e)) => {
                self.running = false;
                self.set_status(e, true);
                Task::none()
            }
            Message::ExplainCurrent => self.explain_query(),
            Message::ExplainFinished(Ok(lines)) => {
                self.explain_lines = lines;
                self.panel = Panel::Explain;
                self.set_status("Explain plan generated", false);
                Task::none()
            }
            Message::ExplainFinished(Err(e)) => {
                self.set_status(format!("Explain failed: {e}"), true);
                Task::none()
            }
            Message::FormatSql => {
                let formatted = format_sql(&self.editor.text());
                self.editor = text_editor::Content::with_text(&formatted);
                Task::none()
            }
            Message::ClearEditor => {
                self.editor = text_editor::Content::new();
                Task::none()
            }
            Message::InsertSnippet(snippet) => {
                self.editor = text_editor::Content::with_text(&snippet);
                Task::none()
            }
            Message::SelectResultTab(i) => {
                self.active_result = i;
                Task::none()
            }
            Message::ApplyCompletion(candidate) => {
                self.apply_completion(&candidate);
                Task::none()
            }

            // ---------- Navigation ----------
            Message::SelectPanel(panel) => {
                self.panel = panel;
                if panel == Panel::Data {
                    return self.load_browse();
                }
                Task::none()
            }
            Message::SelectObject(name) => {
                self.selected_object = Some(name);
                self.browse_page = 0;
                if self.panel == Panel::Data {
                    return self.load_browse();
                }
                if self.panel == Panel::Query {
                    self.panel = Panel::Structure;
                }
                Task::none()
            }
            Message::ToggleGroup(group) => {
                match group {
                    SidebarGroup::Tables => self.expanded.tables = !self.expanded.tables,
                    SidebarGroup::Views => self.expanded.views = !self.expanded.views,
                    SidebarGroup::Indexes => self.expanded.indexes = !self.expanded.indexes,
                    SidebarGroup::Triggers => self.expanded.triggers = !self.expanded.triggers,
                }
                Task::none()
            }
            Message::SidebarFilterChanged(text) => {
                self.sidebar_filter = text;
                Task::none()
            }

            // ---------- Data browser ----------
            Message::BrowseLoaded(Ok(rs)) => {
                self.browse = Some(rs);
                Task::none()
            }
            Message::BrowseLoaded(Err(e)) => {
                self.set_status(format!("Browse failed: {e}"), true);
                Task::none()
            }
            Message::BrowseNextPage => {
                self.browse_page += 1;
                self.load_browse()
            }
            Message::BrowsePrevPage => {
                if self.browse_page > 0 {
                    self.browse_page -= 1;
                }
                self.load_browse()
            }
            Message::RefreshData => self.load_browse(),

            // ---------- Data editing ----------
            Message::BeginEdit(row, col, current) => {
                self.new_row = None;
                self.editing = Some(CellEdit {
                    row,
                    col,
                    draft: current,
                });
                Task::none()
            }
            Message::EditChanged(text) => {
                if let Some(edit) = &mut self.editing {
                    edit.draft = text;
                }
                Task::none()
            }
            Message::CancelEdit => {
                // Escape: cancel the most specific pending action.
                if self.editing.is_some() {
                    self.editing = None;
                } else if self.new_row.is_some() {
                    self.new_row = None;
                } else if self.convert.open && !self.convert.running {
                    self.convert.open = false;
                } else {
                    self.status = None;
                }
                Task::none()
            }
            Message::CommitEdit => self.commit_edit(),
            Message::DeleteRow(row) => self.delete_row(row),
            Message::BeginAddRow => {
                self.editing = None;
                if let Some(table) = self.current_table() {
                    self.new_row = Some(
                        table
                            .columns
                            .iter()
                            .map(|c| (c.name.clone(), String::new()))
                            .collect(),
                    );
                } else {
                    self.set_status("Select a table to add a row", true);
                }
                Task::none()
            }
            Message::NewRowChanged(i, text) => {
                if let Some(row) = &mut self.new_row {
                    if let Some(cell) = row.get_mut(i) {
                        cell.1 = text;
                    }
                }
                Task::none()
            }
            Message::CancelNewRow => {
                self.new_row = None;
                Task::none()
            }
            Message::CommitNewRow => self.commit_new_row(),
            Message::EditApplied(Ok(affected)) => {
                self.editing = None;
                self.new_row = None;
                self.set_status(format!("{affected} row(s) modified"), false);
                // Reload the page and refresh schema (row counts may change).
                let reload = self.load_browse();
                Task::batch([reload, self.refresh_schema()])
            }
            Message::EditApplied(Err(e)) => {
                self.set_status(format!("Edit failed: {e}"), true);
                Task::none()
            }

            // ---------- ERD ----------
            Message::ErdMessage(msg) => {
                erd::update(self, msg);
                Task::none()
            }

            // ---------- Export ----------
            Message::ExportFormatChanged(fmt) => {
                self.export_format = fmt;
                Task::none()
            }
            Message::ExportResults => {
                let Some(result) = self.results.get(self.active_result).cloned() else {
                    self.set_status("No results to export", true);
                    return Task::none();
                };
                let fmt = self.export_format;
                let table = self
                    .selected_object
                    .clone()
                    .unwrap_or_else(|| "exported".to_string());
                let content = decentdb_studio::export::export(&result, fmt, &table);
                Task::perform(pick_export(fmt), move |path| {
                    Message::ExportPicked(path, content.clone())
                })
            }
            Message::ExportPicked(Some(path), content) => {
                match std::fs::write(&path, content) {
                    Ok(()) => self.set_status(format!("Exported to {}", path.display()), false),
                    Err(e) => self.set_status(format!("Export failed: {e}"), true),
                }
                Task::none()
            }
            Message::ExportPicked(None, _) => Task::none(),
            Message::ExportDatabaseSqlite => {
                if self.connection.is_none() {
                    self.set_status("Open a database first", true);
                    return Task::none();
                }
                Task::perform(pick_save_sqlite(), Message::ExportDatabaseSqlitePicked)
            }
            Message::ExportDatabaseSqlitePicked(Some(path)) => {
                let Some(conn) = self.connection.clone() else {
                    return Task::none();
                };
                self.set_status("Exporting to SQLite…", false);
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            conn.export_to_sqlite(&path)
                                .map(|rows| rows as u64)
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("export task failed: {e}")))
                    },
                    Message::EditApplied,
                )
            }
            Message::ExportDatabaseSqlitePicked(None) => Task::none(),
            Message::ExportDatabaseDump => {
                if self.connection.is_none() {
                    self.set_status("Open a database first", true);
                    return Task::none();
                }
                Task::perform(pick_save_sql_dump(), Message::ExportDatabaseDumpPicked)
            }
            Message::ExportDatabaseDumpPicked(Some(path)) => {
                let Some(conn) = self.connection.clone() else {
                    return Task::none();
                };
                match conn.dump_sql() {
                    Ok(sql) => match std::fs::write(&path, sql) {
                        Ok(()) => self.set_status(format!("Dumped SQL to {}", path.display()), false),
                        Err(e) => self.set_status(format!("Dump write failed: {e}"), true),
                    },
                    Err(e) => self.set_status(format!("Dump failed: {e}"), true),
                }
                Task::none()
            }
            Message::ExportDatabaseDumpPicked(None) => Task::none(),

            // ---------- Conversion ----------
            Message::OpenConvertDialog => {
                self.convert = ConvertState {
                    open: true,
                    ..ConvertState::default()
                };
                Task::none()
            }
            Message::CloseConvertDialog => {
                self.convert.open = false;
                Task::none()
            }
            Message::PickConvertSource => {
                Task::perform(pick_sqlite_source(), Message::ConvertSourcePicked)
            }
            Message::ConvertSourcePicked(path) => {
                if let Some(p) = &path {
                    // Suggest a target path next to the source.
                    let mut target = p.clone();
                    target.set_extension("ddb");
                    self.convert.target = Some(target);
                }
                self.convert.source = path;
                Task::none()
            }
            Message::PickConvertTarget => {
                Task::perform(pick_new_database(), Message::ConvertTargetPicked)
            }
            Message::ConvertTargetPicked(path) => {
                self.convert.target = path;
                Task::none()
            }
            Message::RunConversion => self.run_conversion(),
            Message::ConversionFinished(Ok(report)) => {
                self.convert.running = false;
                self.convert.log.push(format!(
                    "Done: {} tables, {} rows, {} indexes, {} warnings",
                    report.tables,
                    report.rows,
                    report.indexes,
                    report.warnings.len()
                ));
                for w in &report.warnings {
                    self.convert.log.push(format!("warning: {w}"));
                }
                self.convert.report = Some(report);
                // Auto-open the converted database.
                if let Some(target) = self.convert.target.clone() {
                    self.convert.open = false;
                    return self.open_connection(target);
                }
                Task::none()
            }
            Message::ConversionFinished(Err(e)) => {
                self.convert.running = false;
                self.convert.log.push(format!("error: {e}"));
                self.set_status(format!("Conversion failed: {e}"), true);
                Task::none()
            }

            // ---------- Misc ----------
            Message::ThemeChanged(theme) => {
                self.settings.theme = theme;
                self.settings.save();
                Task::none()
            }
            Message::DismissStatus => {
                self.status = None;
                if self.convert.open && !self.convert.running {
                    self.convert.open = false;
                }
                Task::none()
            }
            Message::Checkpoint => {
                let Some(conn) = self.connection.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move { conn.checkpoint().map_err(|e| e.to_string()) },
                    Message::CheckpointDone,
                )
            }
            Message::CheckpointDone(Ok(())) => {
                self.set_status("Checkpoint complete", false);
                Task::none()
            }
            Message::CheckpointDone(Err(e)) => {
                self.set_status(format!("Checkpoint failed: {e}"), true);
                Task::none()
            }
        }
    }

    /// Render the full UI.
    pub fn view(&self) -> Element<'_, Message> {
        views::root(self)
    }

    /// Global keyboard shortcuts.
    ///
    /// - `Ctrl/Cmd + Enter` runs the current statement (or selection).
    /// - `Ctrl/Cmd + R` runs the selection or full buffer.
    /// - `Escape` dismisses the status message or closes the convert dialog.
    pub fn subscription(&self) -> iced::Subscription<Message> {
        use iced::keyboard::{self, key, Event as KeyEvent};
        keyboard::listen().map(|event| match event {
            KeyEvent::KeyPressed { key, modifiers, .. } => {
                let cmd = modifiers.command();
                match key.as_ref() {
                    key::Key::Named(key::Named::Enter) if cmd => Message::RunQuery,
                    key::Key::Named(key::Named::Escape) => Message::CancelEdit,
                    key::Key::Character("r") if cmd => Message::RunSelectionOrAll,
                    _ => Message::Noop,
                }
            }
            _ => Message::Noop,
        })
    }

    // ---------- Helpers ----------

    fn open_connection(&mut self, path: PathBuf) -> Task<Message> {
        match Connection::open(&path) {
            Ok(conn) => {
                self.connection = Some(conn);
                let path_str = path.to_string_lossy().into_owned();
                self.settings.push_recent(&path_str);
                self.settings.save();
                self.set_status(format!("Opened {}", path.display()), false);
                self.selected_object = None;
                self.results.clear();
                self.browse = None;
                self.refresh_schema()
            }
            Err(e) => {
                self.set_status(format!("Failed to open {}: {e}", path.display()), true);
                Task::none()
            }
        }
    }

    fn refresh_schema(&mut self) -> Task<Message> {
        let Some(conn) = self.connection.clone() else {
            return Task::none();
        };
        Task::perform(
            async move { conn.schema().map_err(|e| e.to_string()) },
            Message::SchemaLoaded,
        )
    }

    fn run_query(&mut self) -> Task<Message> {
        let Some(conn) = self.connection.clone() else {
            self.set_status("Open a database first", true);
            return Task::none();
        };
        let sql = self.current_sql();
        if sql.trim().is_empty() {
            self.set_status("Nothing to run", true);
            return Task::none();
        }
        self.running = true;
        self.set_status("Running…", false);
        Task::perform(
            async move { conn.execute_batch(&sql).map_err(|e| e.to_string()) },
            Message::QueryFinished,
        )
    }

    fn explain_query(&mut self) -> Task<Message> {
        let Some(conn) = self.connection.clone() else {
            self.set_status("Open a database first", true);
            return Task::none();
        };
        let sql = self.current_sql();
        let first = decentdb_studio::db::split_statements(&sql)
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .unwrap_or_default();
        if first.trim().is_empty() {
            self.set_status("Nothing to explain", true);
            return Task::none();
        }
        Task::perform(
            async move { conn.explain(&first).map_err(|e| e.to_string()) },
            Message::ExplainFinished,
        )
    }

    fn load_browse(&mut self) -> Task<Message> {
        let Some(conn) = self.connection.clone() else {
            return Task::none();
        };
        let Some(table) = self.selected_object.clone() else {
            self.set_status("Select a table to browse", true);
            return Task::none();
        };
        let page_size = self.settings.page_size;
        let offset = self.browse_page * page_size;
        Task::perform(
            async move {
                conn.browse_table(&table, page_size, offset)
                    .map_err(|e| e.to_string())
            },
            Message::BrowseLoaded,
        )
    }

    /// The currently selected table from the schema, if the selection names one.
    fn current_table(&self) -> Option<&decentdb_studio::db::schema::Table> {
        self.selected_object
            .as_deref()
            .and_then(|name| self.schema.table(name))
    }

    /// Build a row-identity (column, value) list used to target an UPDATE or
    /// DELETE. Prefers the primary key; falls back to matching every column so
    /// rows without a declared PK can still be edited (best-effort).
    fn row_identity(&self, page_row: usize) -> Option<Vec<(String, decentdb::Value)>> {
        let rs = self.browse.as_ref()?;
        let row = rs.rows.get(page_row)?;
        let table = self.current_table()?;

        let key_cols: Vec<String> = if !table.primary_key_columns.is_empty() {
            table.primary_key_columns.clone()
        } else {
            // No PK: use all columns as the identity.
            rs.columns.clone()
        };

        let mut identity = Vec::new();
        for key in &key_cols {
            let idx = rs.columns.iter().position(|c| c == key)?;
            identity.push((key.clone(), row[idx].clone()));
        }
        Some(identity)
    }

    fn commit_edit(&mut self) -> Task<Message> {
        let Some(edit) = self.editing.clone() else {
            return Task::none();
        };
        let Some(conn) = self.connection.clone() else {
            return Task::none();
        };
        let Some(rs) = self.browse.clone() else {
            return Task::none();
        };
        let Some(table) = self.selected_object.clone() else {
            return Task::none();
        };
        let Some(column) = rs.columns.get(edit.col).cloned() else {
            return Task::none();
        };
        let Some(identity) = self.row_identity(edit.row) else {
            self.set_status("Cannot identify row to update", true);
            return Task::none();
        };

        // Determine the target type from the schema for correct literal parsing.
        let col_type = self
            .current_table()
            .and_then(|t| t.columns.iter().find(|c| c.name == column))
            .map(|c| c.type_name.clone())
            .unwrap_or_default();
        let new_value = decentdb_studio::db::value::parse_for_type(&edit.draft, &col_type);

        Task::perform(
            async move {
                conn.update_cell(&table, &column, &new_value, &identity)
                    .map_err(|e| e.to_string())
            },
            Message::EditApplied,
        )
    }

    fn delete_row(&mut self, page_row: usize) -> Task<Message> {
        let Some(conn) = self.connection.clone() else {
            return Task::none();
        };
        let Some(table) = self.selected_object.clone() else {
            return Task::none();
        };
        let Some(identity) = self.row_identity(page_row) else {
            self.set_status("Cannot identify row to delete", true);
            return Task::none();
        };
        Task::perform(
            async move {
                conn.delete_row(&table, &identity)
                    .map_err(|e| e.to_string())
            },
            Message::EditApplied,
        )
    }

    fn commit_new_row(&mut self) -> Task<Message> {
        let Some(conn) = self.connection.clone() else {
            return Task::none();
        };
        let Some(table_name) = self.selected_object.clone() else {
            return Task::none();
        };
        let Some(draft) = self.new_row.clone() else {
            return Task::none();
        };
        let Some(table) = self.current_table().cloned() else {
            return Task::none();
        };

        // Build column/value pairs; blank fields become engine defaults (None).
        let mut columns: Vec<(String, Option<decentdb::Value>)> = Vec::new();
        for (name, raw) in &draft {
            if raw.trim().is_empty() {
                columns.push((name.clone(), None));
            } else {
                let col_type = table
                    .columns
                    .iter()
                    .find(|c| &c.name == name)
                    .map(|c| c.type_name.clone())
                    .unwrap_or_default();
                let value = decentdb_studio::db::value::parse_for_type(raw, &col_type);
                columns.push((name.clone(), Some(value)));
            }
        }

        Task::perform(
            async move {
                conn.insert_row(&table_name, &columns)
                    .map_err(|e| e.to_string())
            },
            Message::EditApplied,
        )
    }

    fn run_conversion(&mut self) -> Task<Message> {
        let (Some(source), Some(target)) =
            (self.convert.source.clone(), self.convert.target.clone())
        else {
            self.set_status("Choose source and target files", true);
            return Task::none();
        };
        self.convert.running = true;
        self.convert.log.clear();
        self.convert
            .log
            .push(format!("Converting {} -> {}", source.display(), target.display()));

        Task::perform(
            async move {
                // Run conversion on a blocking thread to keep the UI responsive.
                let result = tokio::task::spawn_blocking(move || {
                    let target_conn = Connection::open(&target).map_err(|e| e.to_string())?;
                    convert::convert(&source, &target_conn, &ConvertOptions::default(), |_| {})
                        .map_err(|e| e.to_string())
                })
                .await;
                match result {
                    Ok(inner) => inner,
                    Err(join) => Err(format!("conversion task failed: {join}")),
                }
            },
            Message::ConversionFinished,
        )
    }

    /// Replace the partial identifier immediately before the cursor with the
    /// chosen completion `candidate`.
    ///
    /// Implemented purely through [`text_editor::Action`]s so it works on the
    /// live editor state and preserves undo history: backspace over the partial
    /// word, then paste the full candidate.
    fn apply_completion(&mut self, candidate: &str) {
        use iced::widget::text_editor::{Action, Edit, Motion};
        use std::sync::Arc;

        let partial = self.partial_word_before_cursor();
        // If the candidate already starts with the partial word, only paste the
        // remaining suffix; otherwise replace the whole partial word.
        let (backspaces, to_insert) = if candidate
            .to_lowercase()
            .starts_with(&partial.to_lowercase())
        {
            (0usize, candidate[partial.len()..].to_string())
        } else {
            (partial.chars().count(), candidate.to_string())
        };

        for _ in 0..backspaces {
            self.editor.perform(Action::Edit(Edit::Backspace));
        }
        self.editor
            .perform(Action::Edit(Edit::Paste(Arc::new(to_insert))));
        // Add a trailing space for fluency after keywords.
        self.editor.perform(Action::Edit(Edit::Insert(' ')));
        let _ = Motion::Right; // keep import meaningful if unused
    }

    /// The partial identifier ending at the cursor position.
    fn partial_word_before_cursor(&self) -> String {
        let cursor = self.editor.cursor();
        let line_idx = cursor.position.line;
        let col = cursor.position.column;
        let line = self
            .editor
            .line(line_idx)
            .map(|l| l.text.to_string())
            .unwrap_or_default();
        let upto: String = line.chars().take(col).collect();
        upto.chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Completion candidates for the current partial word (keywords + schema
    /// identifiers), capped for display.
    pub fn completion_candidates(&self) -> Vec<String> {
        let partial = self.partial_word_before_cursor();
        if partial.len() < 2 {
            return Vec::new();
        }
        let lower = partial.to_lowercase();
        let mut out: Vec<String> = Vec::new();
        for kw in SQL_KEYWORDS {
            if kw.to_lowercase().starts_with(&lower) && !kw.eq_ignore_ascii_case(&partial) {
                out.push((*kw).to_string());
            }
        }
        for ident in self.schema.completion_identifiers() {
            if ident.to_lowercase().starts_with(&lower) && !ident.eq_ignore_ascii_case(&partial) {
                out.push(ident);
            }
        }
        out.dedup();
        out.truncate(8);
        out
    }

    /// The SQL to execute: the selection if present, otherwise the full buffer.
    fn current_sql(&self) -> String {
        if let Some(selection) = self.editor.selection() {
            if !selection.trim().is_empty() {
                return selection;
            }
        }
        self.editor.text()
    }

    pub fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.status = Some(StatusMessage {
            text: text.into(),
            error,
        });
    }
}

// ---------- Async file-dialog helpers ----------

async fn pick_open_database() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("DecentDB", &["ddb", "decentdb", "db"])
        .add_filter("All files", &["*"])
        .set_title("Open DecentDB database")
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn pick_new_database() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("DecentDB", &["ddb"])
        .set_title("Create DecentDB database")
        .set_file_name("database.ddb")
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn pick_sqlite_source() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("SQLite", &["sqlite", "sqlite3", "db", "db3"])
        .add_filter("All files", &["*"])
        .set_title("Choose SQLite database to convert")
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn pick_export(fmt: Format) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter(fmt.label(), &[fmt.extension()])
        .set_file_name(format!("export.{}", fmt.extension()))
        .set_title("Export results")
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn pick_save_sqlite() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("SQLite", &["sqlite", "db"])
        .set_file_name("export.sqlite")
        .set_title("Export database to SQLite")
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn pick_save_sql_dump() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("SQL", &["sql"])
        .set_file_name("dump.sql")
        .set_title("Export database as SQL dump")
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

/// A very small SQL pretty-printer: uppercases leading keywords and places major
/// clauses on their own lines. Intentionally conservative so it never corrupts
/// a statement's semantics.
fn format_sql(sql: &str) -> String {
    const CLAUSES: &[&str] = &[
        "SELECT", "FROM", "WHERE", "GROUP BY", "HAVING", "ORDER BY", "LIMIT", "OFFSET",
        "INNER JOIN", "LEFT JOIN", "RIGHT JOIN", "JOIN", "VALUES", "SET",
    ];
    let mut out = sql.to_string();
    for clause in CLAUSES {
        // Replace a case-insensitive standalone clause with a newline + uppercase.
        let lowered = out.to_lowercase();
        let needle = clause.to_lowercase();
        let mut search_from = 0;
        let mut rebuilt = String::new();
        let mut last = 0;
        while let Some(rel) = lowered[search_from..].find(&needle) {
            let idx = search_from + rel;
            let before_ok = idx == 0 || !lowered.as_bytes()[idx - 1].is_ascii_alphanumeric();
            let after = idx + needle.len();
            let after_ok =
                after >= lowered.len() || !lowered.as_bytes()[after].is_ascii_alphanumeric();
            if before_ok && after_ok {
                rebuilt.push_str(&out[last..idx]);
                if idx != 0 {
                    rebuilt.push('\n');
                }
                rebuilt.push_str(clause);
                last = after;
            }
            search_from = after;
        }
        rebuilt.push_str(&out[last..]);
        out = rebuilt;
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_clauses() {
        let formatted = format_sql("select * from t where id = 1 order by id");
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("\nFROM"));
        assert!(formatted.contains("\nWHERE"));
        assert!(formatted.contains("\nORDER BY"));
    }
}
