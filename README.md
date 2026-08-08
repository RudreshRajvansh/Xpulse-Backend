# XPulse Backend

Rust + axum API server for XPulse, a consent-first personal health record built on ABDM rails.

Covers authentication, consent grants with audit trail, prescriptions, lab orders, appointments, support tickets and patient records — with Argon2id password hashing and per-endpoint role authorization.

Run locally with `cargo run`. Listens on `PORT` (default 3000), health check at `/health`.
