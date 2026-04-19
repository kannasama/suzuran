# TODO

Project-level tasks and ideas not yet captured in a formal plan.

## Template engine hardening

- `src/organizer/template.rs`: unclosed `{` at end of input is silently swallowed (the brace and any partial token are dropped). Should either pass the `{` through literally or be documented as rejected/undefined. Low real-world risk — a malformed template would produce a wrong path immediately visible during testing — but worth fixing before v1.0.
