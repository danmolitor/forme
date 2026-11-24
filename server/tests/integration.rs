use std::path::PathBuf;
use std::sync::Arc;

use axum::http::HeaderName;
use axum::http::HeaderValue;
use axum_test::TestServer;
use serde_json::json;

mod helpers {
    use super::*;

    pub fn config_open() -> Arc<forme_server::config::Config> {
        Arc::new(forme_server::config::Config {
            http_port: 3000,
            api_key: None,
            templates_dir: None,
        })
    }

    pub fn config_with_auth(key: &str) -> Arc<forme_server::config::Config> {
        Arc::new(forme_server::config::Config {
            http_port: 3000,
            api_key: Some(key.to_string()),
            templates_dir: None,
        })
    }

    pub fn config_with_templates(dir: PathBuf) -> Arc<forme_server::config::Config> {
        Arc::new(forme_server::config::Config {
            http_port: 3000,
            api_key: None,
            templates_dir: Some(dir),
        })
    }

    pub fn server(config: Arc<forme_server::config::Config>) -> TestServer {
        let app = forme_server::routes::router(config);
        TestServer::new(app)
    }

    /// Minimal valid Forme document JSON (single page, single text node).
    pub fn minimal_document() -> serde_json::Value {
        json!({
            "children": [{
                "kind": { "type": "Page" },
                "children": [{
                    "kind": { "type": "Text", "content": "Hello, world!" }
                }]
            }]
        })
    }
}

// --- Health endpoint ---

#[tokio::test]
async fn health_returns_ok() {
    let server = helpers::server(helpers::config_open());
    let response = server.get("/health").await;
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "ok");
    assert!(!body["version"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn health_is_public_even_with_auth() {
    let server = helpers::server(helpers::config_with_auth("secret-key"));
    let response = server.get("/health").await;
    response.assert_status_ok();
}

// --- Auth middleware ---

#[tokio::test]
async fn auth_rejects_without_header() {
    let server = helpers::server(helpers::config_with_auth("secret-key"));
    let response = server
        .post("/v1/render")
        .json(&json!({ "template": helpers::minimal_document() }))
        .await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn auth_rejects_wrong_key() {
    let server = helpers::server(helpers::config_with_auth("secret-key"));
    let response = server
        .post("/v1/render")
        .add_header(
            "Authorization".parse::<HeaderName>().unwrap(),
            "Bearer wrong-key".parse::<HeaderValue>().unwrap(),
        )
        .json(&json!({ "template": helpers::minimal_document() }))
        .await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn auth_passes_with_correct_key() {
    let server = helpers::server(helpers::config_with_auth("secret-key"));
    let response = server
        .post("/v1/render")
        .add_header(
            "Authorization".parse::<HeaderName>().unwrap(),
            "Bearer secret-key".parse::<HeaderValue>().unwrap(),
        )
        .json(&json!({ "template": helpers::minimal_document() }))
        .await;
    response.assert_status_ok();
}

#[tokio::test]
async fn no_auth_required_when_key_not_set() {
    let server = helpers::server(helpers::config_open());
    let response = server
        .post("/v1/render")
        .json(&json!({ "template": helpers::minimal_document() }))
        .await;
    response.assert_status_ok();
}

// --- Inline render ---

#[tokio::test]
async fn inline_render_produces_pdf() {
    let server = helpers::server(helpers::config_open());
    let response = server
        .post("/v1/render")
        .json(&json!({ "template": helpers::minimal_document() }))
        .await;
    response.assert_status_ok();
    let bytes = response.as_bytes();
    assert!(
        bytes.starts_with(b"%PDF"),
        "Response should start with PDF header"
    );
}

#[tokio::test]
async fn inline_render_with_invalid_json() {
    let server = helpers::server(helpers::config_open());
    let response = server
        .post("/v1/render")
        .json(&json!({ "template": "not a valid document" }))
        .await;
    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

// --- Slug render ---

#[tokio::test]
async fn slug_render_returns_404_when_no_templates_dir() {
    let server = helpers::server(helpers::config_open());
    let response = server.post("/v1/render/invoice").json(&json!({})).await;
    response.assert_status_not_found();
    let body: serde_json::Value = response.json();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("FORME_TEMPLATES_DIR"));
}

#[tokio::test]
async fn slug_render_loads_template_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let doc = helpers::minimal_document();
    std::fs::write(
        dir.path().join("invoice.json"),
        serde_json::to_string(&doc).unwrap(),
    )
    .unwrap();

    let server = helpers::server(helpers::config_with_templates(dir.path().to_path_buf()));
    let response = server.post("/v1/render/invoice").json(&json!({})).await;
    response.assert_status_ok();
    assert!(response.as_bytes().starts_with(b"%PDF"));
}

#[tokio::test]
async fn slug_render_returns_helpful_404() {
    let dir = tempfile::tempdir().unwrap();
    let server = helpers::server(helpers::config_with_templates(dir.path().to_path_buf()));
    let response = server.post("/v1/render/nonexistent").json(&json!({})).await;
    response.assert_status_not_found();
    let body: serde_json::Value = response.json();
    let error_msg = body["error"].as_str().unwrap();
    assert!(
        error_msg.contains("forme build"),
        "Error should mention 'forme build': {error_msg}"
    );
}

// --- Path traversal prevention ---

#[tokio::test]
async fn slug_rejects_dot_dot() {
    let dir = tempfile::tempdir().unwrap();
    let server = helpers::server(helpers::config_with_templates(dir.path().to_path_buf()));
    let response = server
        .post("/v1/render/..%2Fetc%2Fpasswd")
        .json(&json!({}))
        .await;
    // URL-decoded slug will contain ".." — should be rejected
    let status = response.status_code();
    assert!(
        status == 400 || status == 404,
        "Path traversal should be rejected, got {status}"
    );
}

#[tokio::test]
async fn slug_rejects_forward_slash() {
    let dir = tempfile::tempdir().unwrap();
    let server = helpers::server(helpers::config_with_templates(dir.path().to_path_buf()));
    let response = server.post("/v1/render/foo%2Fbar").json(&json!({})).await;
    let status = response.status_code();
    assert!(
        status == 400 || status == 404,
        "Forward slash in slug should be rejected, got {status}"
    );
}

#[tokio::test]
async fn slug_rejects_backslash() {
    let dir = tempfile::tempdir().unwrap();
    let server = helpers::server(helpers::config_with_templates(dir.path().to_path_buf()));
    let response = server.post("/v1/render/foo%5Cbar").json(&json!({})).await;
    let status = response.status_code();
    assert!(
        status == 400 || status == 404,
        "Backslash in slug should be rejected, got {status}"
    );
}

// --- Redaction ---

/// A one-page PDF, uncompressed, with correct xref offsets. `annot` and
/// `info_title` let a value appear outside the page text.
fn redact_fixture(page_text: &str, annot: &str, info_title: &str, encrypted: bool) -> Vec<u8> {
    let content = format!("BT /F1 12 Tf 72 700 Td ({page_text}) Tj ET");
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Annots [7 0 R] >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len() + 1),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Title ({info_title}) >>"),
        format!("<< /Type /Annot /Subtype /Text /Rect [400 400 420 420] /Contents ({annot}) >>"),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![];
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    let enc = if encrypted {
        " /Encrypt << /Filter /Standard >>"
    } else {
        ""
    };
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 6 0 R{enc} >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn contains(h: &[u8], n: &str) -> bool {
    h.windows(n.len()).any(|w| w == n.as_bytes())
}

#[tokio::test]
async fn redact_patterns_remove_matches_outside_the_page_text() {
    // The SSN is only in a note annotation and the document title, not in
    // the page text. A pattern request must still remove it.
    let pdf = redact_fixture(
        "Name: Jane Example",
        "SSN 123-45-6789",
        "Record 123-45-6789",
        false,
    );
    let server = helpers::server(helpers::config_open());
    let response = server
        .post("/v1/redact")
        .json(&json!({ "pdf": b64(&pdf), "presets": ["ssn"] }))
        .await;
    response.assert_status_ok();
    let out = response.as_bytes();
    assert!(out.starts_with(b"%PDF"));
    assert!(
        !contains(out, "123-45-6789"),
        "the SSN must not be returned"
    );
}

#[tokio::test]
async fn redact_refusal_is_422_redaction_refused() {
    let pdf = redact_fixture("SSN 123-45-6789", "note", "title", true);
    let server = helpers::server(helpers::config_open());
    let response = server
        .post("/v1/redact")
        .json(&json!({ "pdf": b64(&pdf), "redactions": [{ "page": 0, "x": 60, "y": 80, "width": 300, "height": 20 }] }))
        .await;
    response.assert_status(axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = response.json();
    assert_eq!(body["code"], "REDACTION_REFUSED", "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .starts_with("Redaction refused:"),
        "{body}"
    );
}
