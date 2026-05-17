//! HS-4 / N6 — mint a short-lived JWT for a seeded test user so curl can
//! hit the authed `/portfolios/.../rebalance/*` endpoints without going
//! through the (currently F-WALLET-1-broken) signup flow.
//!
//! Usage:
//!   cargo run --bin forge_test_jwt -- <user-uuid>
//!
//! Reads `JWT_SECRET` from the environment (loads `.env.local` then `.env`).
//! Token expires in 1 hour and carries a placeholder email so the auth
//! middleware accepts it.
//!
//! Do NOT commit the printed token to anything tracked. It's a developer
//! convenience for the N6 smoke recipe documented in
//! `scripts/seed-n6-smoke.sh`.

use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use std::env;
use uuid::Uuid;

#[derive(Serialize)]
struct Claims {
    sub: Uuid,
    email: String,
    exp: usize,
    iat: usize,
}

fn main() {
    dotenvy::from_filename(".env.local").ok();
    dotenvy::dotenv().ok();

    let user_id_arg = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: forge_test_jwt <user-uuid>");
        std::process::exit(2);
    });
    let sub: Uuid = user_id_arg.parse().unwrap_or_else(|e| {
        eprintln!("bad user uuid: {e}");
        std::process::exit(2);
    });
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
        eprintln!("JWT_SECRET not set; check .env.local");
        std::process::exit(2);
    });

    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub,
        email: format!("n6-smoke-{}@aegis.local", &sub.to_string()[0..8]),
        exp: now + 60 * 60,
        iat: now,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("encode jwt");
    println!("{token}");
}
