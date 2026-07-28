# Contributor Payment-Period Conduct Note

> **This is a formal conduct policy. Violations may result in suspension from
> current and future paid campaigns, removal from community channels, or
> permanent disqualification from reward consideration.**

A short reminder for contributors working on Aegis contracts during a paid
contribution period (e.g. a GrantFox campaign).

> **TL;DR:** Do the work well, self-review honestly, let maintainers and the
> program evaluate — and do not flood community channels with payment
> complaints. Payment is **not automatic** and is **never guaranteed** by an
> issue label.

## 1. Labels are not promises
- A `Maybe Rewarded` label (or any campaign tag) is **discretionary**, not a
  commitment to pay. It signals the issue *may* qualify — the decision is made
  through the evaluation process below.
- Reward eligibility depends on issue tier, per-campaign paid-issue caps, and
  available campaign budget. None of these are decided by the contributor.

## 2. No spam or payment harassment
- Repeated complaints about "when will I be paid" or "why isn't my issue
  rewarded" flood the channels and slow everyone down. **Do not do this.**
- If you have a genuine question about your submission, ask it **once**, clearly,
  with a link to your PR — then wait for a maintainer or program response.
- Disagreements about reward decisions belong in the evaluation thread, not in
  general chat. Taking reward disputes into public channels is a conduct
  violation.

## 3. Mandatory self-review before payment inquiries
Before you ask about payment, verify your own work meets the bar:
- Does it actually satisfy every acceptance criterion in the issue?
- Do `make build` and `make test` pass locally? (Failing checks block approval —
  see [Failing CI Response Guide](./failing-ci-guide.md).)
- Is the code minimal, readable, and free of `#[allow]`-hidden warnings?
- Did you update docs or README if the change affects them?
- Is the PR description clear about what changed and why?

**If you would not merge it yourself, do not ask about payment.**

## 4. GrantFox evaluation process
Rewards for campaign issues follow the GrantFox flow:
1. **Maintainer assignment** — a maintainer assesses whether the completed work
   meets the issue's bar.
2. **Program approval** — the GrantFox admin team approves against the campaign's
   limited budget and per-contributor paid-issue caps.
3. **Payout** — released only after the campaign ends, via Stellar USDC escrow.

Because approval happens in steps and against a finite budget, **a merged PR does
not equal immediate or guaranteed payment**. This is by design. Stated plainly:
merge and payment are separate events.

## 5. Testing & CI expectations
- All contract changes must build for `wasm32-unknown-unknown` and pass the test
  suite — locally *and* in automation.
- A red check is a blocker. Fix it — do not argue about it. See the
  [Failing CI Response Guide](./failing-ci-guide.md).
- Quality (correctness, edge cases, docs) matters more than raw PR count.
- Submitting low-effort or incomplete work to chase payment volume is
  discouraged and may affect future campaign eligibility.

## 6. Professional conduct is mandatory
- Evaluation queues have many contributors. Silence means "in review," not
  "ignored." Do not ping repeatedly.
- Keep all discussion technical and respectful. Toxic, hostile, or spammy
  behavior is a direct violation of this policy.
- Violations will be acted on without warning, up to and including permanent
  removal from the program.