//! Mini serveur HTTP de test (HTTP/1.1, réponses fixes, compteur d'appels).
//! Remplace httpmock : zéro dépendance, portable, déterministe.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Route {
    method: String,
    path: String,
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

#[derive(Clone)]
pub struct MockServer {
    addr: String,
    routes: Arc<Mutex<Vec<Route>>>,
    hits: Arc<Mutex<HashMap<String, usize>>>,
}

impl MockServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
        let server = MockServer {
            addr,
            routes: Arc::new(Mutex::new(Vec::new())),
            hits: Arc::new(Mutex::new(HashMap::new())),
        };
        let routes = server.routes.clone();
        let hits = server.hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let routes = routes.clone();
                let hits = hits.clone();
                std::thread::spawn(move || handle(stream, routes, hits));
            }
        });
        server
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Enregistre une réponse fixe pour (méthode, chemin exact hors query).
    pub fn add(&self, method: &str, path: &str, status: u16, headers: &[(&str, &str)], body: &str) {
        self.routes.lock().unwrap().push(Route {
            method: method.to_uppercase(),
            path: path.to_string(),
            status,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: body.to_string(),
        });
    }

    pub fn hits(&self, method: &str, path: &str) -> usize {
        *self
            .hits
            .lock()
            .unwrap()
            .get(&format!("{} {}", method.to_uppercase(), path))
            .unwrap_or(&0)
    }
}

fn handle(
    mut stream: TcpStream,
    routes: Arc<Mutex<Vec<Route>>>,
    hits: Arc<Mutex<HashMap<String, usize>>>,
) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    // Lire jusqu'à la fin des en-têtes.
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_uppercase();
    let target = parts.next().unwrap_or_default();
    let path = target.split('?').next().unwrap_or_default().to_string();

    // Consommer un éventuel corps (Content-Length) — nos tests n'en envoient pas.
    let content_length: usize = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0);
    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(buf.len());
    let mut already = buf.len() - header_end;
    while already < content_length {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => already += n,
            Err(_) => return,
        }
    }

    *hits
        .lock()
        .unwrap()
        .entry(format!("{method} {path}"))
        .or_insert(0) += 1;

    let route = routes
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.method == method && r.path == path)
        .cloned();

    let (status, headers, body) = match route {
        Some(r) => (r.status, r.headers, r.body),
        None => (
            404,
            Vec::new(),
            r#"{"message":"no mock route"}"#.to_string(),
        ),
    };
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Mock",
    };
    let mut resp = format!("HTTP/1.1 {status} {reason}\r\n");
    for (k, v) in &headers {
        resp.push_str(&format!("{k}: {v}\r\n"));
    }
    resp.push_str("content-type: application/json\r\n");
    resp.push_str(&format!("content-length: {}\r\n", body.len()));
    resp.push_str("connection: close\r\n\r\n");
    resp.push_str(&body);
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}
