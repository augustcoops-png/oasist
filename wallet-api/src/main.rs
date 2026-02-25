//! OASIST Wallet API — HTTP server for local UI-driven wallet generation.
//!
//! The server binds to 127.0.0.1 ONLY — it is not reachable from any external
//! network interface.  All cross-origin and external webhook requests are
//! rejected with 403 Forbidden.
//!
//! Endpoints:
//!   GET  /                          — HTML wallet-generation UI
//!   POST /api/generate-wallet       — JSON wallet generation (localhost only)
//!        ?words=12|15|18|21|24      — (optional) mnemonic word count, default 12
//!
//! Default listen address: 127.0.0.1:9899
//! Override with --port <PORT>

use {
    bip39::{Language, Mnemonic, MnemonicType, Seed},
    hyper::{
        header,
        service::{make_service_fn, service_fn},
        Body, Method, Request, Response, Server, StatusCode,
    },
    solana_sdk::signature::{keypair_from_seed, Signer},
    std::{
        convert::Infallible,
        env,
        fs::OpenOptions,
        io::Write,
        net::SocketAddr,
    },
};

// ── embedded HTML UI ────────────────────────────────────────────────────────

const UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>OASIST Wallet Generator</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: 'Segoe UI', system-ui, sans-serif;
      background: linear-gradient(135deg, #0f0c29, #302b63, #24243e);
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      color: #e0e0e0;
    }
    .card {
      background: rgba(255,255,255,0.06);
      backdrop-filter: blur(12px);
      border: 1px solid rgba(255,255,255,0.12);
      border-radius: 18px;
      padding: 2.5rem 2rem;
      width: 100%;
      max-width: 640px;
      box-shadow: 0 8px 40px rgba(0,0,0,0.4);
    }
    h1 { font-size: 1.7rem; text-align: center; margin-bottom: 0.3rem; color: #c084fc; }
    .subtitle { text-align: center; font-size: 0.9rem; color: #9ca3af; margin-bottom: 1.8rem; }
    label { font-size: 0.85rem; color: #9ca3af; display: block; margin-bottom: 0.4rem; }
    select {
      width: 100%; padding: 0.65rem 1rem; border-radius: 10px;
      background: rgba(255,255,255,0.08); border: 1px solid rgba(255,255,255,0.15);
      color: #e0e0e0; font-size: 1rem; margin-bottom: 1.2rem; outline: none;
    }
    button.generate {
      width: 100%; padding: 0.85rem; border-radius: 12px;
      background: linear-gradient(90deg, #7c3aed, #a855f7);
      border: none; color: #fff; font-size: 1.05rem; font-weight: 600;
      cursor: pointer; transition: opacity .2s;
    }
    button.generate:hover { opacity: 0.88; }
    button.generate:disabled { opacity: 0.5; cursor: default; }
    .result { margin-top: 1.8rem; display: none; flex-direction: column; gap: 1.2rem; }
    .result.visible { display: flex; }
    .field-label { font-size: 0.78rem; color: #9ca3af; margin-bottom: 0.35rem; text-transform: uppercase; letter-spacing: .05em; }
    .field-value {
      background: rgba(0,0,0,0.35); border-radius: 10px;
      padding: 0.75rem 1rem; font-family: monospace; font-size: 0.92rem;
      word-break: break-all; color: #d1fae5; position: relative;
    }
    .mnemonic-value { color: #fde68a; line-height: 1.7; }
    .copy-btn {
      position: absolute; top: 0.5rem; right: 0.6rem;
      background: rgba(255,255,255,0.1); border: none; border-radius: 6px;
      color: #d1d5db; font-size: 0.75rem; padding: 0.25rem 0.55rem;
      cursor: pointer; transition: background .15s;
    }
    .copy-btn:hover { background: rgba(255,255,255,0.2); }
    .saved-note { font-size: 0.8rem; color: #6ee7b7; text-align: center; }
    .error { color: #f87171; text-align: center; font-size: 0.9rem; margin-top: 1rem; display:none; }
    .spinner { display: inline-block; width: 16px; height: 16px; border: 2px solid rgba(255,255,255,0.3);
      border-top-color: #fff; border-radius: 50%; animation: spin .6s linear infinite; vertical-align: middle; margin-right: 6px; }
    @keyframes spin { to { transform: rotate(360deg); } }
  </style>
</head>
<body>
<div class="card">
  <h1>🔐 OASIST Wallet Generator</h1>
  <p class="subtitle">Generate a new wallet address &amp; recovery phrase</p>

  <label for="words">Mnemonic word count</label>
  <select id="words">
    <option value="12" selected>12 words (standard)</option>
    <option value="15">15 words</option>
    <option value="18">18 words</option>
    <option value="21">21 words</option>
    <option value="24">24 words (maximum security)</option>
  </select>

  <button class="generate" id="genBtn" onclick="generateWallet()">Generate Wallet</button>
  <div class="error" id="errMsg"></div>

  <div class="result" id="result">
    <div>
      <div class="field-label">Wallet Address (Public Key)</div>
      <div class="field-value" id="addrBox">
        <span id="addrText"></span>
        <button class="copy-btn" onclick="copyText('addrText', this)">Copy</button>
      </div>
    </div>
    <div>
      <div class="field-label">Recovery Phrase (<span id="wordCountLabel"></span> words) — keep secret!</div>
      <div class="field-value mnemonic-value" id="mnemonicBox">
        <span id="mnemonicText"></span>
        <button class="copy-btn" onclick="copyText('mnemonicText', this)">Copy</button>
      </div>
    </div>
    <div class="saved-note" id="savedNote"></div>
  </div>
</div>
<script>
  async function generateWallet() {
    const btn = document.getElementById('genBtn');
    const words = document.getElementById('words').value;
    const errMsg = document.getElementById('errMsg');
    const result = document.getElementById('result');
    errMsg.style.display = 'none';
    btn.disabled = true;
    btn.innerHTML = '<span class="spinner"></span>Generating…';
    try {
      const res = await fetch('/api/generate-wallet?words=' + words, { method: 'POST' });
      if (!res.ok) throw new Error('Server error: ' + res.status);
      const data = await res.json();
      document.getElementById('addrText').textContent = data.address;
      document.getElementById('mnemonicText').textContent = data.mnemonic;
      document.getElementById('wordCountLabel').textContent = data.word_count;
      document.getElementById('savedNote').textContent =
        data.address_file ? '✓ Address saved to ' + data.address_file : '';
      result.classList.add('visible');
    } catch (e) {
      errMsg.textContent = e.message || 'Failed to generate wallet.';
      errMsg.style.display = 'block';
    } finally {
      btn.disabled = false;
      btn.innerHTML = 'Generate Wallet';
    }
  }
  function copyText(id, btn) {
    const text = document.getElementById(id).textContent;
    navigator.clipboard.writeText(text).then(() => {
      btn.textContent = 'Copied!';
      setTimeout(() => { btn.textContent = 'Copy'; }, 1800);
    });
  }
</script>
</body>
</html>
"#;

// ── address file helper ──────────────────────────────────────────────────────

fn default_address_file() -> Option<String> {
    dirs_next::home_dir().map(|mut p| {
        p.extend([".config", "solana", "oasist-addresses.txt"]);
        p.to_string_lossy().into_owned()
    })
}

fn append_address(pubkey: &str) -> Option<String> {
    let path = default_address_file()?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    writeln!(file, "{pubkey}").ok()?;
    Some(path)
}

// ── wallet generation ────────────────────────────────────────────────────────

struct WalletResult {
    address: String,
    mnemonic: String,
    word_count: usize,
    address_file: Option<String>,
}

fn generate_wallet(words: usize) -> Result<WalletResult, String> {
    let mnemonic_type = match words {
        15 => MnemonicType::Words15,
        18 => MnemonicType::Words18,
        21 => MnemonicType::Words21,
        24 => MnemonicType::Words24,
        _ => MnemonicType::Words12,
    };
    let mnemonic = Mnemonic::new(mnemonic_type, Language::English);
    let seed = Seed::new(&mnemonic, "");
    let keypair = keypair_from_seed(seed.as_bytes())
        .map_err(|e| format!("keypair error: {e}"))?;
    let address = keypair.pubkey().to_string();
    let phrase = mnemonic.phrase().to_string();
    let actual_word_count = phrase.split_whitespace().count();
    let address_file = append_address(&address);
    Ok(WalletResult {
        address,
        mnemonic: phrase,
        word_count: actual_word_count,
        address_file,
    })
}

// ── HTTP handler ─────────────────────────────────────────────────────────────

/// Returns true when the Host header points to localhost / 127.0.0.1.
/// Requests from any other origin (external webhooks, remote callers) are
/// rejected before any processing takes place.
fn is_localhost(req: &Request<Body>) -> bool {
    req.headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|host| {
            // strip optional port suffix
            let host = host.split(':').next().unwrap_or(host);
            host == "localhost" || host == "127.0.0.1"
        })
        .unwrap_or(false)
}

fn forbidden() -> Response<Body> {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("403 Forbidden: external access is not permitted"))
        .unwrap()
}

async fn handle(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    // Reject every request that does not come from localhost.
    // This prevents webhooks, remote API calls, and cross-origin browser
    // requests from triggering wallet generation.
    if !is_localhost(&req) {
        return Ok(forbidden());
    }

    match (req.method(), req.uri().path()) {
        // UI
        (&Method::GET, "/") | (&Method::GET, "/index.html") => Ok(Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(UI_HTML))
            .unwrap()),

        // Wallet generation API — no CORS headers intentionally
        (&Method::POST, "/api/generate-wallet") => {
            let words: usize = req
                .uri()
                .query()
                .and_then(|q| {
                    q.split('&')
                        .find(|p| p.starts_with("words="))
                        .and_then(|p| p.strip_prefix("words="))
                        .and_then(|s| s.parse().ok())
                })
                .unwrap_or(12);

            match generate_wallet(words) {
                Ok(w) => {
                    let body = serde_json::json!({
                        "address":      w.address,
                        "mnemonic":     w.mnemonic,
                        "word_count":   w.word_count,
                        "address_file": w.address_file,
                    });
                    Ok(Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap())
                }
                Err(e) => {
                    let body = serde_json::json!({ "error": e });
                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap())
                }
            }
        }

        // 404
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap()),
    }
}

// ── entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let port: u16 = env::args()
        .skip_while(|a| a != "--port")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9899);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });

    let server = Server::bind(&addr).serve(make_svc);

    println!("OASIST Wallet API listening on http://127.0.0.1:{port} (localhost only)");
    println!("  UI:  http://localhost:{port}/");
    println!("  API: POST http://localhost:{port}/api/generate-wallet?words=12");
    println!("  External access is disabled — webhook requests will be rejected.");

    if let Err(e) = server.await {
        eprintln!("server error: {e}");
    }
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_wallet_12_words() {
        let w = generate_wallet(12).unwrap();
        assert_eq!(w.word_count, 12);
        assert!(!w.address.is_empty());
        assert_eq!(w.mnemonic.split_whitespace().count(), 12);
    }

    #[test]
    fn test_generate_wallet_24_words() {
        let w = generate_wallet(24).unwrap();
        assert_eq!(w.word_count, 24);
        assert_eq!(w.mnemonic.split_whitespace().count(), 24);
    }

    #[test]
    fn test_generate_wallet_unknown_defaults_to_12() {
        let w = generate_wallet(99).unwrap();
        assert_eq!(w.word_count, 12);
    }

    #[test]
    fn test_generate_wallet_unique_addresses() {
        let a = generate_wallet(12).unwrap().address;
        let b = generate_wallet(12).unwrap().address;
        assert_ne!(a, b, "each wallet must have a unique address");
    }

    // ── localhost guard ──────────────────────────────────────────────────────

    fn req_with_host(host: &str) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri("/api/generate-wallet")
            .header(header::HOST, host)
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn test_is_localhost_accepts_localhost() {
        assert!(is_localhost(&req_with_host("localhost")));
        assert!(is_localhost(&req_with_host("localhost:9899")));
    }

    #[test]
    fn test_is_localhost_accepts_127_0_0_1() {
        assert!(is_localhost(&req_with_host("127.0.0.1")));
        assert!(is_localhost(&req_with_host("127.0.0.1:9899")));
    }

    #[test]
    fn test_is_localhost_rejects_external_hosts() {
        assert!(!is_localhost(&req_with_host("example.com")));
        assert!(!is_localhost(&req_with_host("attacker.io")));
        assert!(!is_localhost(&req_with_host("192.168.1.5")));
        assert!(!is_localhost(&req_with_host("0.0.0.0")));
    }

    #[test]
    fn test_is_localhost_rejects_missing_host() {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/generate-wallet")
            .body(Body::empty())
            .unwrap();
        assert!(!is_localhost(&req));
    }
}
