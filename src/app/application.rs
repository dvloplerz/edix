use iced::{Alignment, Color, Element, Length, Task, Theme};
use iced::widget::{button, column, container, row, text, text_input, Scrollable};
use std::path::PathBuf;
use native_dialog::FileDialog;
use crate::services::excel_diff;

/// Main application state
#[derive(Debug, Clone, Default)]
pub struct DifferApp {
    /// Selected Excel file path
    file_path: Option<PathBuf>,
    /// Source sheet name (default: "03")
    source_sheet: String,
    /// GL sheet name (default: "GL")
    gl_sheet: String,
    /// Column index to compare (0-based, default: 3 for column D)
    column_index: usize,
    /// Row limit for source sheet (default: 488)
    source_limit: usize,
    /// Row limit for GL sheet (default: 932)
    gl_limit: usize,
    /// Comparison results
    results: Option<Vec<(usize, String)>>,
    /// GL set size for summary
    gl_set_size: usize,
    /// Mismatch count
    mismatch_count: usize,
    /// Status message
    status: String,
    /// Loading state
    is_loading: bool,
    /// Error message
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// File picker events
    FilePickerOpened,
    FilePickerClosed(Option<PathBuf>),
    /// Drag and drop events
    EventOccurred(iced::Event),
    /// Input field changes
    SourceSheetChanged(String),
    GLSheetChanged(String),
    ColumnIndexChanged(String),
    SourceLimitChanged(String),
    GLLimitChanged(String),
    /// Action buttons
    RunComparison,
    /// Background task results
    ComparisonCompleted(Result<(Vec<(usize, String)>, usize, usize), excel_diff::ExcelDiffError>),
    /// Reset
    Reset,
}

impl DifferApp {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                file_path: None,
                source_sheet: "03".to_string(),
                gl_sheet: "GL".to_string(),
                column_index: 3,
                source_limit: 488,
                gl_limit: 932,
                results: None,
                gl_set_size: 0,
                mismatch_count: 0,
                status: "Ready".to_string(),
                is_loading: false,
                error: None,
            },
            Task::none()
        )
    }

    pub fn title(&self) -> String {
        String::from("Excel Differ - Accounting Audit Tool")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FilePickerOpened => {
                return Task::perform(
                    async move {
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        std::thread::spawn(move || {
                            let res = FileDialog::new()
                                .add_filter("Excel File", &["xlsx"])
                                .show_open_single_file()
                                .unwrap_or(None);
                            let _ = tx.send(res);
                        });
                        rx.await.unwrap_or(None)
                    },
                    Message::FilePickerClosed
                );
            }
            Message::FilePickerClosed(path) => {
                if let Some(path) = path {
                    if path.extension().and_then(|s| s.to_str()) == Some("xlsx") {
                        self.file_path = Some(path.clone());
                        self.status = format!("Selected file: {}", path.display());
                    } else {
                        self.status = "Invalid file type. Please select an .xlsx file.".to_string();
                    }
                } else {
                    self.status = "No file selected".to_string();
                }
                Task::none()
            }
            Message::EventOccurred(event) => {
                if let iced::Event::Window(iced::window::Event::FileDropped(path)) = event {
                    if path.extension().and_then(|s| s.to_str()) == Some("xlsx") {
                        self.file_path = Some(path.clone());
                        self.status = format!("Dropped file: {}", path.display());
                    } else {
                        self.status = "Invalid file type. Please drop an .xlsx file.".to_string();
                    }
                }
                Task::none()
            }
            Message::SourceSheetChanged(sheet) => {
                self.source_sheet = sheet;
                Task::none()
            }
            Message::GLSheetChanged(sheet) => {
                self.gl_sheet = sheet;
                Task::none()
            }
            Message::ColumnIndexChanged(text) => {
                if let Ok(index) = text.parse::<usize>() {
                    self.column_index = index;
                    self.status = format!("Column index set to {}", index);
                } else if !text.is_empty() {
                    self.status = "Invalid column index".to_string();
                }
                Task::none()
            }
            Message::SourceLimitChanged(text) => {
                if let Ok(limit) = text.parse::<usize>() {
                    self.source_limit = limit;
                    self.status = format!("Source limit set to {}", limit);
                } else if !text.is_empty() {
                    self.status = "Invalid limit".to_string();
                }
                Task::none()
            }
            Message::GLLimitChanged(text) => {
                if let Ok(limit) = text.parse::<usize>() {
                    self.gl_limit = limit;
                    self.status = format!("GL limit set to {}", limit);
                } else if !text.is_empty() {
                    self.status = "Invalid limit".to_string();
                }
                Task::none()
            }
            Message::RunComparison => {
                if self.file_path.is_none() {
                    self.status = "Please select an Excel file first".to_string();
                    return Task::none();
                }

                self.is_loading = true;
                self.status = "Running comparison...".to_string();
                self.error = None;

                let file_path = self.file_path.as_ref().unwrap().clone();
                let source_sheet = self.source_sheet.clone();
                let gl_sheet = self.gl_sheet.clone();
                let column_index = self.column_index;
                let source_limit = self.source_limit;
                let gl_limit = self.gl_limit;

                return Task::perform(
                    async move {
                        excel_diff::compare_sheets(
                            file_path,
                            source_sheet,
                            gl_sheet,
                            column_index,
                            source_limit,
                            gl_limit,
                        ).await
                    },
                    Message::ComparisonCompleted
                );
            }
            Message::ComparisonCompleted(result) => {
                self.is_loading = false;
                match result {
                    Ok((results, gl_set_size, mismatch_count)) => {
                        self.results = Some(results);
                        self.gl_set_size = gl_set_size;
                        self.mismatch_count = mismatch_count;
                        self.status = format!(
                            "Comparison complete. Found {} mismatches.",
                            mismatch_count
                        );
                    }
                    Err(err) => {
                        self.error = Some(err.to_string());
                        self.status = "Comparison failed".to_string();
                    }
                }
                Task::none()
            }
            Message::Reset => {
                *self = DifferApp::new().0;
                Task::none()
            }
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::event::listen().map(Message::EventOccurred)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let file_picker = button("Select Excel File")
            .on_press(Message::FilePickerOpened);

        let file_info = if let Some(path) = &self.file_path {
            column![
                text(format!("Selected: {}", path.display())),
                text("Drop an .xlsx file here to change").size(12).style(|_theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    ..Default::default()
                })
            ].spacing(5)
        } else {
            column![
                text("No file selected"),
                text("Drag and drop an .xlsx file here").size(12).style(|_theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    ..Default::default()
                })
            ].spacing(5)
        };

        let inputs = column![
            row![
                text("Source Sheet:"),
                text_input("03", &self.source_sheet)
                    .on_input(Message::SourceSheetChanged)
                    .width(100)
            ],
            row![
                text("GL Sheet:"),
                text_input("GL", &self.gl_sheet)
                    .on_input(Message::GLSheetChanged)
                    .width(100)
            ],
            row![
                text("Column Index (0-based):"),
                text_input("3", &self.column_index.to_string())
                    .on_input(Message::ColumnIndexChanged)
                    .width(100)
            ],
            row![
                text("Source Limit:"),
                text_input("488", &self.source_limit.to_string())
                    .on_input(Message::SourceLimitChanged)
                    .width(100)
            ],
            row![
                text("GL Limit:"),
                text_input("932", &self.gl_limit.to_string())
                    .on_input(Message::GLLimitChanged)
                    .width(100)
            ]
        ].spacing(10);

        let run_button: Element<Message> = if self.is_loading {
            Element::from(
                button("Run Comparison")
                    .style(button::secondary)
            )
        } else {
            Element::from(
                button("Run Comparison")
                    .on_press(Message::RunComparison)
                    .style(button::primary)
            )
        };

        let reset_button = button("Reset")
            .on_press(Message::Reset);

        let action_buttons: Element<Message> = if self.is_loading {
            column![run_button, reset_button].spacing(10).into()
        } else {
            row![run_button, reset_button].spacing(10).into()
        };

        let status_bar: Element<Message> = container(
            text(&self.status)
                .size(14)
        )
        .width(Length::Fill)
        .padding(10)
        .style(move |_theme: &Theme| {
            if self.is_loading {
                container::Style::default()
            } else if self.error.is_some() {
                container::Style::default()
                    .background(Color::from_rgb(1.0, 0.9, 0.9))
            } else {
                container::Style::default()
                    .background(Color::from_rgb(0.9, 1.0, 0.9))
            }
        })
        .into();

        let results_view = if let Some(results) = &self.results {
            if results.is_empty() {
                Element::from(text("No mismatches found"))
            } else {
                let content = column![
                    text(format!("Found {} mismatches:", results.len())).size(16),
                    Scrollable::new(
                        column(
                            results.iter()
                                .map(|(row, value)| {
                                    Element::from(
                                        container(
                                            row![
                                                text(format!("Row {}:", row)).width(80),
                                                text(value)
                                            ]
                                        )
                                        .width(Length::Fill)
                                        .padding(5)
                                    )
                                })
                                .collect::<Vec<_>>()
                        )
                        .spacing(5)
                    )
                    .height(300)
                ];
                Element::from(container(content).padding(10))
            }
        } else if self.is_loading {
            Element::from(
                container(text("Running comparison..."))
                    .padding(10)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
            )
        } else {
            Element::from(
                container(text("Select a file and click 'Run Comparison' to see results"))
                    .padding(10)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
            )
        };

        column![
            row![file_picker, file_info].spacing(10),
            inputs,
            action_buttons,
            results_view,
            status_bar
        ]
        .padding(20)
        .spacing(15)
        .into()
    }
}
