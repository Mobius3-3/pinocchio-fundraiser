# Fundraiser Program

A Solana fundraiser smart contract written in Rust using the `pinocchio` ecosystem.

## Overview

This program supports a simple token-based fundraiser lifecycle:

1. `initialize` creates a fundraiser PDA with target amount + duration.
2. `contribute` records contributor amounts (bounded to 10% per contributor).
3. `checker` lets the maker claim funds if the target is met.
4. `refund` lets contributors reclaim funds after expiry if the target is not met.

## Program ID

`4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT`

## Instruction Discriminators

The first byte of `instruction_data` selects the instruction:

- `0` → `initialize`
- `1` → `checker` (`process_check_contributions_instruction`)
- `2` → `contribute`
- `3` → `refund`

## State + PDAs

- `fundraiser` PDA seeds: `['fundraiser', maker]`
- `contributor` PDA seeds: `['contributor', fundraiser, contributor]`

State structs:

- `Fundraiser`: maker, mint, target amount, current amount, start time, duration, bump.
- `Contributor`: contributed amount, bump.

## Rules and Limits

From `src/constants.rs`:

- `MIN_AMOUNT_TO_RAISE = 3`
- `SECONDS_TO_DAYS = 86400`
- `MAX_CONTRIBUTION_PERCENTAGE = 10`
- `PERCENTAGE_SCALER = 100`

Effective behavior:

- A contributor cannot contribute more than 10% of `amount_to_raise` in total.
- `contribute` is only valid before fundraiser duration elapses.
- `checker` succeeds only when vault amount >= target.
- `refund` succeeds only after duration elapses and only when target is not met.

## Local Development

From `fundraiser/`:

```bash
cargo build
cargo test
```

## Tests

Core behavior tests live in `src/tests/mod.rs` and cover:

- Fundraiser initialization field correctness
- Contribution path (including contributor PDA auto-create)
- Successful maker claim when target is met
- Contributor refund path when fundraiser expires unfunded
- Compute unit snapshots for each instruction

## Notes

Additional compute unit notes are in `docs/notes.md`.
