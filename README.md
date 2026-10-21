# XPulse Backend

Rust + axum API server for XPulse, a consent-first personal health record built on ABDM rails.

Run with `cargo run` (listens on `PORT`, default 3000). Run the integration suite with `cargo test`.

| Variable | Purpose |
| --- | --- |
| `XPULSE_SEED_PASSWORD` | Password for the seeded demo accounts. If unset, each account gets a random password, printed once when the store is created. |
| `XPULSE_SESSION_TTL_SECS` | Session lifetime. Default 43200 (12 hours). |
| `XPULSE_DEV` | Set to `1` to print password reset codes to the server log. No email or SMS provider is wired in yet. |

## Who can read what

A patient always has full access to their own record. Nobody else can read it without a live consent grant from that patient, and a grant only opens the scopes the patient chose: `Medicines`, `Allergies`, `Profile`, `LabResults`, `Reports`.

| Record | Patient | Doctor | Any other role |
| --- | --- | --- | --- |
| Medicines, allergies, profile, lab results, reports | Own | Only with a live grant covering that scope | Never |
| Audit log, support tickets, care suggestions | Own | Never | Never |

Grants can only be issued by the patient, only to a doctor, and last at most 24 hours. Revoking takes effect on the next request. Every read under consent, and every refused attempt, is written to the patient's audit log under the signed-in user's id.

| Role | Can act on |
| --- | --- |
| Doctor | Grants issued to them, and the waiting queue. Prescribing needs a `Medicines` grant, ordering labs needs a `LabResults` grant. |
| Pharmacy | The pending prescription queue: patient name and prescribed items. |
| Diagnostics | The lab order queue: patient name, test, status and result. |
| Receptionist | The check-in code, waiting queue and admitting patients. |
| Customer care | The support ticket queue. |
| Admin | Platform counts and the user directory. |
| Super admin | Everything admin can, plus creating staff accounts. Cannot create super admins. |
