use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tokio::sync::broadcast;

pub struct BroadcastLayer {
    pub sender: broadcast::Sender<String>,
}

impl<S: Subscriber> tracing_subscriber::Layer<S> for BroadcastLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target().contains("hyper") || event.metadata().target().contains("h2") {
            return; // Ignore spammy web server logs
        }
    
        let mut visitor = EventVisitor { content: String::new() };
        event.record(&mut visitor);
        
        let level = event.metadata().level().to_string();
        let target = event.metadata().target();
        
        let color = match level.as_str() {
            "INFO" => "#3498db",
            "WARN" => "#f1c40f",
            "ERROR" => "#e74c3c",
            "DEBUG" => "#95a5a6",
            _ => "#ecf0f1",
        };

        let html_line = format!(
            "<div style='font-family: inherit; white-space: pre-wrap; margin-bottom: 4px;'>\
                <span style='color: {}; font-weight: bold;'>[{}]</span> \
                <span style='color: #888;'>[{}]</span> \
                <span style='color: #ddd;'>{}</span>\
            </div>",
            color, level, target, visitor.content
        );

        let _ = self.sender.send(html_line);
    }
}

struct EventVisitor {
    content: String,
}
impl tracing_core::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let s = format!("{:?}", value);
            if s.starts_with('"') && s.ends_with('"') {
                self.content = s[1..s.len()-1].to_string();
            } else {
                self.content = s;
            }
        }
    }
}
