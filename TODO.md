# TODO

Project-level tasks and ideas not yet captured in a formal plan.

## Organize job: dry-run with no matching rule

- `src/jobs/organize.rs`: when `dry_run: true` and no rule matches, the handler returns
  `BadRequest` rather than a graceful `{"dry_run": true, "proposed_path": null}`. Current
  behavior is arguably more useful (tells the user the rule set is incomplete), but diverges
  from the plan spec. Revisit if the UI needs a non-error signal for "no rule matched."

## Template engine hardening

- `src/organizer/template.rs`: unclosed `{` at end of input is silently swallowed (the brace and any partial token are dropped). Should either pass the `{` through literally or be documented as rejected/undefined. Low real-world risk — a malformed template would produce a wrong path immediately visible during testing — but worth fixing before v1.0.
