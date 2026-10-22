# XPulse backend

Rust (axum) API behind the XPulse Android app. XPulse is a personal health record built on ABDM, where patients decide who gets to see their data.

`cargo run` starts it on `PORT` (3000 if unset). `cargo test` runs the integration tests.

| Variable | What it does |
| --- | --- |
| `XPULSE_SEED_PASSWORD` | Password for the demo accounts. If it's unset, each account gets a random password, printed once when the data file is first created. |
| `XPULSE_SESSION_TTL_SECS` | How long a login lasts. Defaults to 43200 (12 hours). |
| `XPULSE_DEV` | Set to `1` to print password reset codes in the server log. There's no email or SMS provider yet. |

## Access rules

Patients can always read their own record. Anyone else needs a grant from that patient, and a grant only covers what the patient picked: `Medicines`, `Allergies`, `Profile`, `LabResults`, `Reports`. Only a patient can create a grant, only for a doctor, and for 24 hours at most. Revoking takes effect on the next request. Reads under a grant go into the patient's audit log, and so do refused attempts.

| Record | Patient | Doctor | Other roles |
| --- | --- | --- | --- |
| Medicines, allergies, profile, lab results, reports | Own | With a grant covering it | No |
| Audit log, support tickets, care suggestions | Own | No | No |

Staff accounts belong to a facility. Admins belong to an area, which is a group of facilities. Work queues only show the caller's own facility, and anything from another facility returns 404.

| Role | Sees and acts on |
| --- | --- |
| Doctor | Grants addressed to them, plus their facility's waiting queue. Prescribing needs a `Medicines` grant and ordering a lab needs `LabResults`. |
| Pharmacy | Undispensed prescriptions written at its facility (patient name and items) |
| Diagnostics | Lab orders placed at its facility |
| Receptionist | Its facility's check-in code and waiting queue |
| Customer care | All support tickets. This role isn't tied to a facility. |
| Admin | Staff list and activity counts for the facilities in its area |
| Super admin | The whole platform. Can create staff accounts, but not other super admins. |

The seed data has three facilities: `fac_1` and `fac_2` in `area_1`, and `fac_3` in `area_2`. All demo staff accounts are in `fac_1`.
