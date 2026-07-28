# Contributor Payment-Period Conduct Note

A short reminder for contributors working on Aegis contracts during a paid
contribution period (e.g. a GrantFox campaign).

> **TL;DR:** Do the work well, self-review honestly, let maintainers and the
> program evaluate — and don't flood community channels with payment
> complaints. Payment is **not automatic** and is **never guaranteed** by an
> issue label.

## 1. Labels are not promises
- A `Maybe Rewarded` label (or any campaign tag) is **discretionary**, not a
  commitment to pay. It signals the issue *may* qualify — the decision is made
  through the evaluation process below.
- Reward eligibility depends on issue tier, per-campaign paid-issue caps, and
  available campaign budget. None of these are decided by the contributor.

## 2. Avoid spamming community channels
- Repeated complaints about "when will I be paid" or "why isn't my issue
  rewarded" flood the channels and slow everyone down.
- If you have a genuine question about your submission, ask it **once**, clearly,
  with a link to your PR — then wait for a maintainer or program response.
- Disagreements about reward decisions belong in the evaluation thread, not in
  general chat.

## 3. Self-review before expecting payment
Before you consider a contribution "done," review it as a maintainer would:
- Does it actually satisfy the issue's acceptance criteria?
- Do `make build` and `make test` pass locally? (Failing checks block approval —
  see [Failing CI Response Guide](./failing-ci-guide.md).)
- Is the code minimal, readable, and free of `#[allow]`-hidden warnings?
- Did you update docs/README if the change affects them?
- Is the PR description clear about what changed and why?

If you wouldn't merge it yourself, don't expect a maintainer to.

## 4. GrantFox evaluation process
Rewards for campaign issues follow the GrantFox flow:
1. **Maintainer assignment** — a maintainer assesses whether the completed work
   meets the issue's bar.
2. **Program approval** — the GrantFox admin team approves against the campaign's
   limited budget and per-contributor paid-issue caps.
3. **Payout** — released only after the campaign ends, via Stellar USDC escrow.

Because approval happens in steps and against a finite budget, **a merged PR does
not equal immediate or guaranteed payment**. This is by design, not a slight.

## 5. Testing & CI expectations
- All contract changes must build for `wasm32-unknown-unknown` and pass the test
  suite — locally *and* in automation.
- A red check is a blocker. Fix it; don't argue it. See the
  [Failing CI Response Guide](./failing-ci-guide.md).
- Quality (correctness, edge cases, docs) matters more than raw PR count.

## 6. Be patient and professional
- Evaluation queues have many contributors. Silence usually means "in review,"
  not "ignored."
- Keep discussion technical and respectful. Toxic or spammy behavior can itself
  disqualify contributions from reward consideration.
