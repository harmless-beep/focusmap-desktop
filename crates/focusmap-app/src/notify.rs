//! Native notifications, opt-in and muted by focus mode.
//!
//! Sent from a worker thread because the Windows toast API can block briefly
//! and the UI thread has a 16 ms budget.

pub fn send(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .appname("FocusMap")
            .timeout(notify_rust::Timeout::Milliseconds(8000))
            .show();
    });
}
