use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    jwt_secret: Arc<String>,
    users: Arc<Mutex<HashMap<String, UserRecord>>>,
}

#[derive(Clone)]
struct UserRecord {
    password_hash: String,
    // For now we keep billing simple: account is active by default in dev.
    subscription_active: bool,
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    token: String,
}

#[derive(Debug, Serialize)]
struct MeResponse {
    email: String,
    subscription_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    subscription_active: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let listen: SocketAddr = std::env::var("LUCIDITY_AUTH_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:9091".to_string())
        .parse()
        .context("invalid LUCIDITY_AUTH_LISTEN (expected host:port)")?;

    let jwt_secret = std::env::var("LUCIDITY_AUTH_JWT_SECRET")
        .unwrap_or_else(|_| "dev-insecure-secret-change-me".to_string());

    let state = AppState {
        jwt_secret: Arc::new(jwt_secret),
        users: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/signup", post(signup))
        .route("/v1/login", post(login))
        .route("/v1/me", get(me))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    log::info!("lucidity-auth listening on {}", listen);
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

async fn signup(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err((StatusCode::BAD_REQUEST, "invalid email".into()));
    }
    if req.password.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, "password too short".into()));
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("hash failed: {e}"),
            )
        })?
        .to_string();

    let mut users = state.users.lock().await;
    if users.contains_key(&email) {
        return Err((StatusCode::CONFLICT, "email already exists".into()));
    }
    users.insert(
        email.clone(),
        UserRecord {
            password_hash: hash,
            subscription_active: true,
        },
    );

    let token = issue_token(&state.jwt_secret, &email, true)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(AuthResponse { token }))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let email = req.email.trim().to_lowercase();
    let users = state.users.lock().await;
    let user = users
        .get(&email)
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "invalid credentials".into()))?
        .clone();
    drop(users);

    let parsed = PasswordHash::new(&user.password_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("bad hash: {e}")))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid credentials".into()))?;

    let token = issue_token(&state.jwt_secret, &email, user.subscription_active)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(AuthResponse { token }))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, (StatusCode, String)> {
    let claims = authorize(&state.jwt_secret, &headers)?;
    Ok(Json(MeResponse {
        email: claims.sub,
        subscription_active: claims.subscription_active,
    }))
}

fn issue_token(secret: &str, email: &str, subscription_active: bool) -> anyhow::Result<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let exp = now + Duration::from_secs(30 * 24 * 3600).as_secs();
    let claims = Claims {
        sub: email.to_string(),
        exp: exp as usize,
        subscription_active,
    };
    let token = jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

fn authorize(secret: &str, headers: &HeaderMap) -> Result<Claims, (StatusCode, String)> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "missing authorization".into()))?;
    let token = auth
        .strip_prefix("Bearer ")
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "expected bearer token".into()))?
        .trim();

    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let decoded = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| (StatusCode::UNAUTHORIZED, format!("invalid token: {e}")))?;

    if decoded.claims.sub.trim().is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "invalid token".into()));
    }
    Ok(decoded.claims)
}
