use anyhow::Context;
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use uuid::Uuid;

use super::models::{AuthResponse, LoginRequest, RegisterRequest, User, UserPublic};
use crate::middleware::auth::Claims;
use crate::{config::Config, db::Db, error::AppError};

pub async fn register(
    db: &Db,
    req: RegisterRequest,
    cfg: &Config,
) -> crate::error::Result<AuthResponse> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(&req.email)
        .fetch_one(db)
        .await?;

    if exists {
        return Err(AppError::Conflict("email already registered".into()));
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .to_string();

    let risk_tolerance = req.risk_tolerance.unwrap_or_else(|| "moderate".into());

    let user: User = sqlx::query_as(
        "INSERT INTO users (id, email, password_hash, risk_tolerance, investment_horizon_months)
         VALUES ($1, $2, $3, $4, 12)
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&req.email)
    .bind(&hash)
    .bind(&risk_tolerance)
    .fetch_one(db)
    .await?;

    let token = mint_token(&user, cfg)?;
    Ok(AuthResponse {
        token,
        user: to_public(user),
    })
}

pub async fn login(db: &Db, req: LoginRequest, cfg: &Config) -> crate::error::Result<AuthResponse> {
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(db)
        .await?;

    let user = user.ok_or_else(|| AppError::Unauthorized("invalid credentials".into()))?;

    let parsed = PasswordHash::new(&user.password_hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed)
        .map_err(|_| AppError::Unauthorized("invalid credentials".into()))?;

    let token = mint_token(&user, cfg)?;
    Ok(AuthResponse {
        token,
        user: to_public(user),
    })
}

fn mint_token(user: &User, cfg: &Config) -> crate::error::Result<String> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user.id,
        email: user.email.clone(),
        iat: now,
        exp: now + (cfg.jwt_expiry_hours as usize * 3600),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))
}

fn to_public(u: User) -> UserPublic {
    UserPublic {
        id: u.id,
        email: u.email,
        risk_tolerance: u.risk_tolerance,
    }
}
