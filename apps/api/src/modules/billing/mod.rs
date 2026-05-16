//! Billing module — Sprint 4.
//!
//! Right now this carries the **referral payout** loop only:
//!  - Frontend captures `?ref=<handle>` from the signup URL.
//!  - Wallet handlers forward the handle to `record_referral`.
//!  - We resolve handle → user_id via `md5(id::text)[:8]` and INSERT
//!    into `referrals` (unique on `new_user_id`, so duplicate signups
//!    via the same person can't double-pay).
//!  - Under `EXECUTION_MOCK=true` the payout is mocked: `paid_at = NOW()`,
//!    `tx_hash = "mock:..."`. Under `EXECUTION_MOCK=false` the real path
//!    submits a Circle Nanopayment from the treasury wallet — left as a
//!    TODO with a clear error so the operator knows to wire it.
//!
//! The 50¢-per-referral default is intentionally tiny — it's a "thanks"
//! signal, not an incentive that would attract sybils.

pub mod aum_stream;
pub mod handlers;
pub mod service;
pub mod types;
