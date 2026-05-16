use differ::DifferApp;

fn main() -> iced::Result {
    iced::application(DifferApp::new, DifferApp::update, DifferApp::view)
        .title("Excel Differ - Accounting Audit Tool")
        .subscription(DifferApp::subscription)
        .run()
}
