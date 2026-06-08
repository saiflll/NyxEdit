use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use tiny_http::{Header, Response, Server};

type DynResponse = Response<Box<dyn Read + Send + 'static>>;

pub struct ProxyState {
    pub port: u16,
    pub logs: Arc<Mutex<Vec<String>>>,
}

impl ProxyState {
    pub fn new(logs: Arc<Mutex<Vec<String>>>) -> Self {
        ProxyState { port: 0, logs }
    }
}

pub fn start_proxy(state: &Arc<Mutex<ProxyState>>) {
    let server = Server::http("127.0.0.1:0").expect("Failed to start proxy server");
    let port = server.server_addr().to_ip().unwrap().port();

    {
        let mut s = state.lock().unwrap();
        s.port = port;
        s.logs.lock().unwrap().push(format!("Proxy started on port {}", port));
    }

    let logs = {
        let s = state.lock().unwrap();
        s.logs.clone()
    };

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let target = extract_target(&url);

            let response: DynResponse = match target {
                Some(target_url) => handle_proxy(&target_url, &logs),
                None => res_text("NyxEdit Proxy — use /proxy?url=<encoded_url>", 200),
            };

            if let Err(e) = request.respond(response) {
                log_error(&logs, &format!("Respond error: {}", e));
            }
        }
    });
}

fn extract_target(url: &str) -> Option<String> {
    let parsed = url::Url::parse(&format!("http://localhost{}", url)).ok()?;
    parsed
        .query_pairs()
        .find(|(key, _)| key == "url")
        .map(|(_, val)| val.into_owned())
}

fn res_text(body: &str, status: u16) -> DynResponse {
    Response::from_string(body.to_string())
        .with_status_code(status as i32)
        .boxed()
}

fn handle_proxy(
    target_url: &str,
    logs: &Arc<Mutex<Vec<String>>>,
) -> DynResponse {
    log_info(logs, &format!("→ {}", target_url));

    let client = match reqwest::blocking::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log_error(logs, &format!("Client build error: {}", e));
            return res_text(&format!("Internal error: {}", e), 500);
        }
    };

    match client.get(target_url).send() {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("text/html")
                .to_string();

            let body = match resp.bytes() {
                Ok(b) => b.to_vec(),
                Err(e) => {
                    log_error(logs, &format!("Body read error: {}", e));
                    return res_text(&format!("Read error: {}", e), 502);
                }
            };

            log_info(
                logs,
                &format!("← {} ({} bytes) [{}]", status, body.len(), content_type),
            );

            let mut response = Response::from_data(body)
                .with_status_code(status as i32)
                .with_header(
                    Header::from_bytes(b"Content-Type", content_type.as_bytes())
                        .unwrap(),
                );
            response.add_header(
                Header::from_bytes(b"Access-Control-Allow-Origin", b"*").unwrap(),
            );
            response.boxed()
        }
        Err(e) => {
            log_error(logs, &format!("Fetch error: {} — {}", target_url, e));
            res_text(&format!("Proxy fetch error: {}", e), 502)
        }
    }
}

fn log_info(logs: &Arc<Mutex<Vec<String>>>, msg: &str) {
    let mut log = logs.lock().unwrap();
    log.push(msg.to_string());
    if log.len() > 200 {
        log.remove(0);
    }
}

fn log_error(logs: &Arc<Mutex<Vec<String>>>, msg: &str) {
    let mut log = logs.lock().unwrap();
    log.push(format!("✗ {}", msg));
    if log.len() > 200 {
        log.remove(0);
    }
}

#[tauri::command]
pub fn get_proxy_port(state: tauri::State<Arc<Mutex<ProxyState>>>) -> u16 {
    let s = state.lock().unwrap();
    s.port
}

#[tauri::command]
pub fn get_proxy_logs(state: tauri::State<Arc<Mutex<ProxyState>>>) -> Vec<String> {
    let s = state.lock().unwrap();
    let log = s.logs.lock().unwrap();
    log.iter().rev().take(50).rev().cloned().collect()
}
