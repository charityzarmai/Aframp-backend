## feat(mint): SLA Timers, Escalation Engine & Stellar Timebounds

Adds automated SLA enforcement for mint requests that stall in `PENDING` / `PARTIALLY_APPROVED` state.

### What changed

- **30-min background worker** evaluates all stalled requests each cycle
- **4h** → warning reminder to Tier-1 approver (Slack + Email)
- **12h** → escalation to Tier-2 manager + department lead
- **24h** → auto-expire request, block Stellar submission
- **Stellar timebound guard** — `assert_submittable()` blocks any transaction from hitting the ledger if the internal SLA is already breached; `max_time` is always capped at `min(sla_expires_at, now+23h)`
- **Expiration guard** — expired requests cannot be re-approved without fresh re-submission (#123)
- All SLA actions are atomic, idempotent (optimistic `WHERE stage = <expected>` locking), and written to `mint_audit_log` + new `mint_escalation_log` (#117)

### Schema

3 new tables: `mint_sla_state`, `mint_escalation_log`, `mint_stellar_timebounds`. A DB trigger auto-initialises the SLA state row on every `mint_requests INSERT`.

### SLA stage flow

```
pending → warned → escalated → expired
       ↘ resolved (request left pending naturally)
```

Closes #MINT-SLA-001 · Refs #117 · Refs #123
