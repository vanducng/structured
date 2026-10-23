use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

const MCP_URL: &str = "https://mcp.structured.app/mcp";
const REDIRECT: &str = "http://127.0.0.1:8734/callback";
const SCOPE: &str = "email";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Auth {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: f64,
}

pub fn config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("structured");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config").join("structured")
}

pub fn auth_path() -> PathBuf {
    config_dir().join("auth.json")
}

pub fn load() -> Result<Auth> {
    let path = auth_path();
    if !path.exists() {
        return Ok(Auth::default());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn save(auth: &Auth) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    let path = auth_path();
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(auth)?;
    {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&tmp)?.write_all(body.as_bytes())?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn logout() -> Result<()> {
    let path = auth_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn http() -> Result<Client> {
    Ok(Client::builder().timeout(Duration::from_secs(30)).build()?)
}

pub fn bearer(http: &Client, auth: &mut Auth) -> Result<String> {
    if token_ok(auth) {
        return Ok(auth.access_token.clone());
    }
    refresh(http, auth)?;
    if auth.access_token.is_empty() {
        return Err(Error::Auth);
    }
    Ok(auth.access_token.clone())
}

fn token_ok(auth: &Auth) -> bool {
    !auth.access_token.is_empty() && auth.expires_at > now() + 30.0
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn refresh(http: &Client, auth: &mut Auth) -> Result<()> {
    if auth.refresh_token.is_empty() || auth.client_id.is_empty() {
        return Err(Error::Auth);
    }
    let meta = oauth_meta(http)?;
    let response = http
        .post(&meta.token_endpoint)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", auth.refresh_token.as_str()),
            ("client_id", auth.client_id.as_str()),
            ("resource", MCP_URL),
        ])
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    apply_token(auth, &response)?;
    save(auth)
}

pub fn login(http: &Client) -> Result<Auth> {
    let mut auth = load()?;
    if auth.client_id.is_empty() {
        auth.client_id = register(http)?;
        save(&auth)?;
    }
    let verifier = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect::<String>();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect::<String>();
    let meta = oauth_meta(http)?;
    let query = [
        ("response_type", "code"),
        ("client_id", auth.client_id.as_str()),
        ("redirect_uri", REDIRECT),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", state.as_str()),
        ("scope", SCOPE),
        ("resource", MCP_URL),
    ]
    .map(|(k, v)| format!("{k}={}", urlencoding::encode(v)))
    .join("&");
    let url = format!("{}?{query}", meta.authorization_endpoint);
    println!("Opening Structured sign-in in the browser.");
    println!("{url}");
    let _ = std::process::Command::new(open_cmd()).arg(&url).status();
    let callback = wait_callback(&state)?;
    let response = http
        .post(&meta.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", callback.as_str()),
            ("redirect_uri", REDIRECT),
            ("client_id", auth.client_id.as_str()),
            ("code_verifier", verifier.as_str()),
            ("resource", MCP_URL),
        ])
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    apply_token(&mut auth, &response)?;
    save(&auth)?;
    println!("Signed in.");
    Ok(auth)
}

fn open_cmd() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    }
}

struct Meta {
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: String,
}

fn oauth_meta(http: &Client) -> Result<Meta> {
    let resource: Value = http
        .get("https://mcp.structured.app/.well-known/oauth-protected-resource")
        .send()?
        .error_for_status()?
        .json()?;
    let issuer = resource["authorization_servers"][0]
        .as_str()
        .ok_or_else(|| Error::Api("missing authorization server".into()))?
        .trim_end_matches('/');
    let meta: Value = http
        .get(format!("{issuer}/.well-known/oauth-authorization-server"))
        .send()?
        .error_for_status()?
        .json()?;
    Ok(Meta {
        authorization_endpoint: text(&meta, "authorization_endpoint")?,
        token_endpoint: text(&meta, "token_endpoint")?,
        registration_endpoint: text(&meta, "registration_endpoint")?,
    })
}

fn text(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Error::Api(format!("oauth metadata missing {key}")))
}

fn register(http: &Client) -> Result<String> {
    let meta = oauth_meta(http)?;
    let response = http
        .post(meta.registration_endpoint)
        .json(&serde_json::json!({
            "client_name": "structured",
            "redirect_uris": [REDIRECT],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
            "scope": SCOPE,
        }))
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    response
        .get("client_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Error::Api("registration did not return client_id".into()))
}

fn apply_token(auth: &mut Auth, payload: &Value) -> Result<()> {
    auth.access_token = payload
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Auth)?
        .to_owned();
    if let Some(refresh) = payload.get("refresh_token").and_then(Value::as_str) {
        auth.refresh_token = refresh.to_owned();
    }
    let expires_in = payload
        .get("expires_in")
        .and_then(Value::as_f64)
        .unwrap_or(3600.0);
    auth.expires_at = now() + expires_in - 30.0;
    Ok(())
}

fn wait_callback(expected_state: &str) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:8734")?;
    listener.set_nonblocking(true)?;
    let deadline = SystemTime::now() + Duration::from_secs(180);
    println!("Waiting up to 3 minutes...");
    loop {
        if SystemTime::now() > deadline {
            return Err(Error::Auth);
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut buf = [0_u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let location = req.split_whitespace().nth(1).unwrap_or("");
                let body = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<p>Signed in. Return to the terminal.</p>";
                let _ = stream.write_all(body);
                if !location.starts_with("/callback") {
                    continue;
                }
                let query = location.split_once('?').map(|(_, q)| q).unwrap_or("");
                let mut code = None;
                let mut state = None;
                let mut error = None;
                for pair in query.split('&') {
                    let Some((k, v)) = pair.split_once('=') else {
                        continue;
                    };
                    let v = urlencoding::decode(v).unwrap_or_default().into_owned();
                    match k {
                        "code" => code = Some(v),
                        "state" => state = Some(v),
                        "error" => error = Some(v),
                        _ => {}
                    }
                }
                if let Some(error) = error {
                    return Err(Error::Api(error));
                }
                if state.as_deref() != Some(expected_state) {
                    return Err(Error::Api("sign-in state mismatch".into()));
                }
                return code.ok_or(Error::Auth);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err.into()),
        }
    }
}
