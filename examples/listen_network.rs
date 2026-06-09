//! Network monitoring example
use rpage::network::NetworkMonitor;

fn main() -> rpage::Result<()> {
    let monitor = NetworkMonitor::new();

    // Record some requests
    use std::collections::HashMap;
    monitor.record_request(rpage::network::RequestRecord {
        request_id: "1".into(),
        url: "https://example.com/api/data".into(),
        method: "GET".into(),
        headers: HashMap::new(),
        resource_type: "XHR".into(),
    });

    monitor.record_response(rpage::network::ResponseRecord {
        request_id: "1".into(),
        url: "https://example.com/api/data".into(),
        status: 200,
        headers: HashMap::new(),
        mime_type: "application/json".into(),
    });

    // Query
    println!("Requests: {}", monitor.requests().len());
    println!("Responses: {}", monitor.responses().len());
    println!("API calls: {}", monitor.find_requests_by_url("/api/").len());

    Ok(())
}
