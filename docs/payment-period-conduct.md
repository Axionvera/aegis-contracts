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

## 2. No spam or payment complaints in community channels

- Repeated messages about "when will I be paid" or "why isn't my issue
  rewarded" constitute channel spam. They will be removed without warning.
- If you have a genuine question about your submission, ask it **once**, clearly,
  with a link to your PR — then wait for a maintainer or program response. Do
  not bump, repost, or follow up in unrelated threads.
- Disagreements about reward decisions belong in the evaluation thread, not in
  general chat. Raising them publicly may result in forfeiture of reward
  consideration for the disputed submission.

## 3. Self-review before expecting payment

Before you consider a contribution complete or ask about payment, verify every
point a maintainer would check:

- Does your submission satisfy **every** acceptance criterion in the issue?
- Do `make build` and `make test` pass locally with zero warnings?
- Is the code minimal, readable, and free of `#[allow]`-hidden warnings?
- Did you update all relevant docs and the README if the change affects them?
- Is the PR description clear about what changed, why, and which criteria it
  addresses?

**If you would not approve the PR yourself, do not expect a maintainer to.** A
submission that fails basic self-review wastes maintainer time and will delay
or forfeit reward consideration.

## 4. GrantFox evaluation process

Rewards for campaign issues follow a three-stage process that is sequential
and budget-constrained:

1. **Maintainer assessment** — a maintainer reviews the completed work against
   the issue's acceptance criteria and project quality standards.
2. **Program approval** — the GrantFox admin team approves against the
   campaign's limited budget and per-contributor paid-issue caps.
3. **Payout** — released only after the campaign concludes, via Stellar USDC
   escrow.

Because approval happens in stages and against a finite budget, **a merged PR
does not equal immediate or guaranteed payment**. This is by design, not an
oversight. No amount of channel activity accelerates the process.

## 5. CI and testing compliance

- All contract changes must build for `wasm32-unknown-unknown` and pass the
  full test suite — locally **and** in CI automation.
- A red CI check is a hard blocker. Fix the issue before requesting re-review;
  do not argue about it in community channels. Use the
  [Failing CI Response Guide](./failing-ci-guide.md) for troubleshooting.
- Quality (correctness, edge-case coverage, documentation) matters more than
  raw PR count. Submitting low-effort PRs to inflate numbers is not acceptable.

## 6. Patience and professionalism

- Evaluation queues serve many contributors. Silence from maintainers means
  "in review," not "ignored." Do not ask for status updates in community
  channels within 7 days of submission.
- All discussion must remain technical and respectful. Toxic behaviour,
  harassment, or public demands for payment disqualify the associated
  contributions from reward consideration and may lead to a permanent ban from
  the project and future GrantFox campaigns.