# ADR-002: Explicit id registry for pagination (no ledger scan)

- Status: accepted
- Date: 2026-10-04
- Context: ISSUE-020 design

## Problem

`list_requests` needs paging. Contracts cannot scan the ledger, and an
unbounded scan would be a DoS/gas hazard anyway.

## Decision

- Maintain an append-only `AllIds: Vec<BytesN<32>>` registry (one push per
  `deposit`, idempotent, TTL-extended like every write).
- `list_requests(offset, limit)` slices the RAW registry
  (`[offset, offset+limit)`, at most `limit ≤ 50` ledger reads), then drops
  non-matching entries when a status filter is set. Filtered scans walk
  pages client-side until a short page.
- Terminal requests stay listed (filter by status); there are no deletion
  paths.

## Consequences

- `deposit` pays one extra write + one `Vec` push; `track_request_id`
  dedupes with an O(n) in-memory scan (no extra ledger reads).
- If registry growth ever makes deposits measurably expensive, the
  documented migration is a bump-allocator id scheme while keeping `AllIds`
  for reads (see `docs/GAS.md`).
- Deterministic insertion order; pages are stable across calls.
