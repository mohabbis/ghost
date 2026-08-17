# Competitive landscape

Ghost sells **governed execution**: a human-approved, verified, audited pipeline over
software that was never designed to be automated. That is a narrow category, and the
products it gets compared to mostly sit in *adjacent* categories — context layers,
assistants, RPA, and workflow orchestrators.

`cloud/docs/PRIOR_ART.md` covers the open-source orchestrators (Temporal, Camunda,
Windmill, n8n, …) as engineering prior art. This document covers **commercial
competitors** — who a buyer might put next to Ghost on a shortlist, and what to take
or refuse from each.

The map:

| Category | Example | Relationship to Ghost |
|---|---|---|
| **Ambient context layer** | [Littlebird](#littlebird) | Adjacent above — analyzed below |
| Governed execution / trust runtime | *(no named direct competitor yet)* | Ghost's category |
| Agent frameworks | Claude, Cursor, Codex | **Clients**, not competitors — they propose, Ghost gates |
| No-code automation | Zapier, Make, n8n Cloud | Connectors without a trust pipeline — explicit non-goal |
| Enterprise RPA | UiPath, Automation Anywhere | Same problem, coordinate-first, no approval-gate story |
| Screen-recall / memory | Rewind, Windows Recall | Same layer as Littlebird, weaker on privacy stance |

Only Littlebird is analyzed in depth so far. The rest are named to keep the map honest,
not because a survey of each has been done.

---

## Littlebird

> **Sourcing caveat.** `littlebird.ai` is blocked by this environment's network egress
> proxy, so nothing here is quoted from the vendor's own site. Everything below comes
> from press coverage, review sites, and search summaries (see [Sources](#sources)).
> Treat feature specifics as **secondary-source**, and re-verify against the product
> before any of this reaches a pitch, a comparison page, or a roadmap commitment.

### What it actually is

A macOS-native, always-on assistant that builds a **private memory of your work**. It
reads *structured text* from the active window across every app you have open — not
screenshots — transcribes your meetings, and stores that context as local text on the
Mac. You then ask it questions, and it answers with the context already in hand.
"Routines" run a saved prompt on a recurring schedule (daily briefing, weekly activity
summary). Connectors to email and calendar pull in further context and, per one review,
can "take actions on your behalf."

The company: founded 2024 by Alap Shah, Naman Shah, and Alexander Green — the Shah
brothers previously founded Sentieo (sold to AlphaSense). **$11M seed**, announced March
2026. Product Hunt #1 product of the day. **SOC 2 certified**, GDPR/CCPA compliant,
AES-256 at rest, TLS 1.3 in transit, no training on user data.

Pricing: **free** Basic tier; Plus at **$17/mo** annual or $20 monthly; higher tiers
reported up to ~$100/mo; 14-day trial on paid plans.

### The finding: this is not a head-on competitor

Littlebird is a **context layer**, not an execution platform. The clearest statement of
this comes from a competitor's own comparison page, which is where positioning is
usually least flattering and most accurate: Littlebird "is a smart observer on your
machine; unlike agents … that do the work itself, Littlebird gives you better answers
about your work." Another review puts it as "the context layer that feeds into your
other tools."

Set against Ghost's pipeline, the overlap is thin and the gap is structural:

| Capability | Littlebird | Ghost Cloud |
|---|---|---|
| Ambient cross-app context capture | **Yes** — all macOS apps, always on | No — and refused, see below |
| Meeting transcription | **Yes** | No |
| Persistent work memory + Q&A | **Yes**, local | No |
| Scheduled recurring prompts | **Yes** ("Routines") | **No trigger surface at all** |
| Records a demonstrated workflow | Observes, doesn't compile to steps | Yes — Phase 2 Chrome extension → typed steps |
| Executes multi-step work across apps | Limited, via email/calendar connectors | **Yes** — browser + API, typed steps |
| Deny-by-default gate on send/pay/delete | Not evidenced | **Yes** — deterministic `classifyStep` |
| Human approval before sensitive actions | Not evidenced | **Yes**, with expiry, single-use |
| Per-step outcome verification | No | **Yes** |
| Hash-chained audit log | No | **Yes**, two-level |
| Incidents / undo / durable resume | No | **Yes** (undo designed, not built) |
| Multi-tenant org isolation, RBAC | Single-user Mac app | **Yes** |
| SOC 2 | **Yes** | **No** |
| Self-serve free tier | **Yes** | **No** — implementation-led |
| Platform | macOS only | Cloud (browser + API) |

Ghost wins every row that describes *doing the work under controls*. Littlebird wins
every row that describes *knowing about the work* — plus, notably, the two rows that are
about being a real company you can buy from today: SOC 2 and a free tier.

They are also not competing for the same dollar. $17/mo personal productivity versus
$500–2,000/mo team plus $2.5k–15k implementation are different budgets, different
buyers, and different sales motions.

### So what is the actual threat?

Three of them, and none is "Littlebird takes Ghost's customers."

**1. Narrative capture.** Littlebird has $11M, a shipping consumer product, a #1 Product
Hunt launch, and press on the story "AI that already knows what you're working on."
Ghost has a stronger engine and no self-serve on-ramp. The risk is that Littlebird
defines the category in the buyer's head first, and Ghost gets evaluated as *"Littlebird
but harder to set up"* — judged on a rubric written by a product that doesn't do
governed execution and doesn't need to.

**2. They own the layer above Ghost's hardest unsolved problem.** Ghost's Phase 2 answer
to "which workflow should we automate?" is: record one in Chrome, compile it to typed
steps. Littlebird already watches *every app on the machine, continuously*. It is much
better positioned to know which work is repetitive — because it sees all of it, and
Ghost sees only what someone chose to record in a browser tab. If Littlebird adds
execution, it descends into Ghost's layer carrying context Ghost cannot get.

**3. Asymmetric compliance urgency.** Littlebird has SOC 2 as a $17/mo consumer app,
where it's a nice-to-have. Ghost sells approval-gated audit trails to regulated ops
teams — bookkeeping, healthcare admin, financial ops — where SOC 2 is a **procurement
blocker**, and Ghost doesn't have it. A product whose entire pitch is auditability being
outrun on third-party attestation by a personal-productivity app is a positioning
problem, not just a checklist gap.

### The tripwires

Littlebird becomes a direct competitor the moment it does any of these. Each is a step
down into Ghost's layer:

1. **Write-execution beyond email/calendar** — especially browser automation.
2. **A team/org tier** with shared memory and admin controls.
3. **An approval concept** — any "confirm before it sends" gate.
4. **Beyond macOS**, i.e. server-side or cross-platform execution.
5. **Compiling observed activity into a reusable, editable routine** rather than a
   recurring prompt. This is the one to watch hardest — it is Ghost's Phase 2, reached
   from ambient capture instead of from an extension.

If none of these ship, Littlebird stays a complement — plausibly even a context source
Ghost consumes.

### An uncomfortable note for the record

Ghost's legacy desktop app **already built Littlebird's wedge and deprioritized it**.
`src-tauri/src/core/atlas.rs` + `storage/atlas.rs` are a local, offline semantic-memory
graph over the user's work. `commands/experimental.rs` includes observer mode.
`core/ocr.rs` does on-device OCR with no network. Local-first, no-cloud, never-delete.

Littlebird raised $11M taking approximately that wedge to market as a $17/mo macOS app
with meeting notes and a good privacy story.

The correct reading is **not** "go back to the desktop app." The wedge was abandoned for
sound reasons recorded in `CLAUDE.md`: the local-first delivery model doesn't carry
forward, and ambient observation is against Ghost's own trust boundary. The correct
reading is narrower and more useful: **the wedge is fundable, and the market rewards
products that visibly know the user's work.** Ghost's version of that has to be built
out of run history, not surveillance.

---

## What Ghost should adopt

Framed the way `PRIOR_ART.md` frames its steals — the refusals matter as much.

**Adopt: a trigger surface, and treat it as competitive, not backlog.**
Littlebird ships scheduled recurring execution at $17/mo. `PRIOR_ART.md` lists "Triggers
and schedules" as deferred with the note *"no trigger surface exists at all."* That
reads differently now. It also has a genuine product blocker recorded alongside it —
scheduling *unattended* runs of an approval-gated workflow needs an answer for who gets
notified — which leads directly to the next item.

**Adopt: a notification surface. This is the highest-leverage unblock in the repo.**
It currently blocks three separately-deferred things: multiple approvers, Slack/email
approval routing (both from Windmill), and scheduled runs. One piece of infrastructure,
three features, and one of them is now a competitive gap. Build this first.

**Adopt: workflow intelligence from run history, not from observation.**
Littlebird's real insight is that a product feels indispensable when it already knows
your work. Ghost can have a scoped version of this without watching anything: it already
stores every run, every step, every approval, every verification. It should be able to
tell a customer *"this workflow ran 43 times last month, 91% of its steps never needed
approval, and step 7 has failed 4 times"* — which is a better automation-ROI argument
than ambient capture can make, because it's measured on work Ghost actually executed.
No new capture surface, no new trust boundary, uses data that exists.

**Adopt: state the screenshot stance as loudly as they state theirs.**
Littlebird made "we don't take screenshots" a headline privacy claim. Ghost **does**
capture per-step screenshots — and that's defensible: they are evidence for an auditor,
scoped to a run a human triggered, on a workflow a human approved, not ambient recording
of a person's day. That distinction is real and currently under-stated. Rule 10 forbids
overclaiming; it does not forbid claiming what is true. Write it down in
`trust-pipeline.md` with the same clarity Littlebird writes theirs.

**Adopt: SOC 2, with urgency.** See the asymmetry above. For Ghost's buyer this is a
blocker, not a badge.

**Adopt: one self-serve path that completes.** Not a free tier, and not a reversal of
the implementation-led model in `business-model.md` — that ordering is deliberate. But
"$2,500 minimum before you see it work" loses every evaluation where the buyer wants to
try before they talk. One templated workflow a prospect can run end-to-end — gate,
approve, verify, audit log, all of it — is the demo the engine has earned and doesn't
currently have a front door for.

## What Ghost should refuse

**Always-on ambient capture across all applications.** This violates a stated trust
boundary in `CLAUDE.md` — *"No monitoring the customer hasn't asked for"* — and it is a
different product with a different buyer. Ghost's capture is scoped, per-workflow, and
initiated by a human who is demonstrating a task on purpose. Do not blur that to match a
competitor's feature list.

**A local-first / on-device trust story.** Already decided and documented. Ghost's trust
story is least privilege + approval + verification + tamper-evident audit. Littlebird's
strong local-privacy story is an argument for stating Ghost's story better, not for
changing it.

**Meeting transcription, personal memory, and "ask me about your day."** Adjacent
product, adjacent buyer, and `product-direction.md` already lists "not a chatbot or 'AI
coworker'" as a non-goal.

**macOS-native as the primary surface.** Ghost executes against browsers and APIs on
servers. Desktop automation stays where the strategy puts it: after browser, before
vision.

---

## Sources

- [Littlebird raises $11 Million — PR Newswire](https://www.prnewswire.com/news-releases/littlebird-raises-11-million-to-launch-the-only-ai-that-already-knows-what-youre-working-on-302721664.html)
- [Littlebird raises $11M for its AI-assisted 'recall' tool — TechCrunch](https://techcrunch.com/2026/03/23/littlebird-raises-11m-to-capture-context-from-your-computer-so-you-can-query-your-data/) *(egress-blocked; cited from search summary)*
- [Littlebird — Product Hunt](https://www.producthunt.com/products/littlebird)
- [Littlebird pricing](https://littlebird.ai/pricing) *(egress-blocked)*
- [Littlebird Review 2026 — Efficient App](https://efficient.app/apps/littlebird) *(egress-blocked; cited from search summary)*
- [Littlebird Review — Agent Finder](https://agent-finder.co/reviews/littlebird)
- [6 Best Littlebird Alternatives — Carly](https://www.usecarly.com/blog/littlebird-alternatives/) — source of the "smart observer … gives you better answers about your work" characterization
- [Littlebird AI Review — Toolworthy](https://www.toolworthy.ai/tool/littlebird-ai)
- [Littlebird AI — App Store](https://apps.apple.com/us/app/littlebird-ai/id6737920045)

Internal: [`product-direction.md`](product-direction.md) ·
[`business-model.md`](business-model.md) · [`trust-pipeline.md`](trust-pipeline.md) ·
[`../cloud/docs/PRIOR_ART.md`](../cloud/docs/PRIOR_ART.md) ·
[`../cloud/docs/CURSOR_HANDOFF.md`](../cloud/docs/CURSOR_HANDOFF.md)
