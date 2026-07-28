# Business model & moat

> **Updated for the cloud direction.** Ghost is now a cloud SaaS AI operator (see
> `docs/product-direction.md`). Pricing is the tiered model in "The model" below;
> the earlier "$79/month flat, local-first" framing is superseded. Much of the
> historical "local-first moat" analysis in this file is retained as context, but
> the moat is now **trust + auditability + hybrid execution across a customer's
> existing systems**, not on-device isolation.

Operational notes, not a vision essay — pair with `docs/gtm-organizer.md`
(persona/channels) and `docs/PRODUCT_ROADMAP.md` (near-term build order).

## The hard truth

Local-first is a **positioning** strength but a **monetization** weakness *by
default*:

- no cloud lock-in → switching cost is near zero unless we build it;
- no server-side usage metering → usage-based pricing is hard;
- data stays on-device → no data network effect falls out for free;
- easier to copy/pirate a binary than to revoke a cloud account.

So the edge and the revenue must be **engineered**, not assumed. This doc names
the specific edges and the specific model. If Ghost stays a pure single-user
utility with no accumulation and no team layer, it is a nice tool with a weak
business. The business lives in the durable moats and the team/compliance tier
below.

## The edge (moat), strongest first

1. **Counter-positioning — the deep one.** Cloud agents (Rational-style
   "agentic employees", and the general autonomous-agent wave) are structurally
   committed to running in the cloud and holding the user's credentials. They
   **cannot** offer "your data and logins never leave your machine" without
   becoming a different company. The ops lead at a small wealth-management or
   accounting firm who has been explicitly told "we cannot put client data
   through a cloud tool" is ours by construction, not by feature parity. This
   is architecture the incumbent can't copy, which is the only kind of moat that
   survives a better-funded competitor.

2. **Trust as brand.** In software that *acts on your computer*, trust is the
   scarce resource, and one autonomous-agent horror story poisons the category.
   Ghost's whole pipeline (inspect → approve → audit → undo, deny-by-default, no
   silent delete/overwrite, rule-attributed exportable audit) is a trust brand
   that compounds and is slow/expensive for others to earn. Reinforce it in
   every release; never spend it on a growth hack.

3. **Manufactured switching costs.** Local-first gives none for free, so build
   the accumulation:
   - **Routines/Zones/trust rules as the user's own IP** — the more they
     configure and record, the more they'd lose by leaving. (This is why the
     wizard, presets, and Ghost Routines matter beyond UX: they are
     switching-cost factories.)
   - **Audit history as a compliance record** they can't recreate elsewhere.
   - **Weekly-habit integration** for the ops-lead wedge.

4. **A routine/template marketplace = a network effect bolted onto a local
   app.** The app runs locally; the *sharing layer* is networked. Community and
   verified playbooks make Ghost more valuable as more people publish — the
   Raycast Store / VS Code extensions / Alfred workflows pattern. This is how
   local-first earns a network effect without cloud lock-in.

5. **Vertical depth.** Being the *best* at the client-filing job for a small
   wealth-management or accounting firm (deep classification, the exact
   presets, compliance-grade audit export) beats a horizontal cloud agent on
   that niche's real pains. Depth is a moat via focus.

## Proof it's not hopeless — local-first that monetizes

These are comps for the *thesis* (local-first can be a sustainable business),
not a pricing template — Ghost deliberately does not copy their tiering (see
below):

- **Obsidian** (closest analog): free for personal use, paid **Commercial
  license** for business use, paid **Sync** (E2E-encrypted — you sell the relay,
  not access to data) and **Publish** add-ons. Local-first, profitable, no VC
  dependency.
- **Raycast**: free core, **Pro** subscription (AI + advanced), **Team** tier,
  extension store. Local-first Mac productivity app that raised real money.
- **1Password / Little Snitch / TablePlus / Proxyman / BBEdit / JetBrains**:
  paid local tools via license or subscription. Sustainable without cloud
  lock-in.
- **Tailscale / Syncthing**: local-first with freemium + team monetization.

## The model — tiered, with implementation-led early revenue

Ghost is priced as a cloud SaaS with tiers plus paid setup:

| Tier | Price | For |
|---|---|---|
| Individual | $29–49 / month | A single operator automating their own recurring workflows |
| Professional | $99–199 / user / month | A team member running multi-step, connector-backed workflows |
| Team | $500–2,000 / month | A business, priced by runs, integrations, and governance |
| Implementation services | $2,500–15,000 (one-time) | Configuring a customer's specific workflows |

The early money is expected to come from **implementation, not self-serve
subscriptions.** Operations-heavy SMBs don't wake up craving automation software;
they pay to remove a specific, recurring, expensive headache. Land by configuring
one high-value workflow (reporting, reconciliation, document processing), prove it,
then expand into more workflows and more seats.

The moat is engineered, not assumed: **trust + auditability** (approval gates,
verification, hash-chained audit an auditor accepts), **hybrid execution** across
the customer's existing systems (API → browser → desktop → vision fallback, so
Ghost works where clean integrations don't exist), and **accumulated workflow
IP** (each configured workflow raises switching cost). Enterprise motion (SSO,
compliance questionnaires, procurement) comes later, when real pull justifies it.

## What we do NOT do (protect the moat)

- No pivot to cloud-first storage or vendor-held credentials to chase
  usage-metering revenue — that *is* the counter-positioning moat; spending it
  makes us a worse copy of Rational.
- No dark-pattern lock-in. Switching cost must come from accumulated value
  (routines, audit history), never from data hostage-taking — data stays local
  and exportable. Trust is the brand; don't spend it.
- No feature that breaks a privacy boundary in `CLAUDE.md` to add a paid hook.

## Threats & the defense

| Threat | Defense |
|---|---|
| OS vendors build it in (Shortcuts, Power Automate, Recall) | Cross-app depth, trust brand, and verticals the OS won't serve; be the reviewable/audited layer they aren't |
| Cloud agents add an "on-prem / local" mode | Architecture-deep counter-positioning + an earned trust brand aren't a checkbox; our whole stack is deny-by-default and reviewable, theirs is retrofitted |
| Low willingness to pay for a consumer utility | Revenue is per-seat at firms where paying for control + compliance is normal; there is no free tier funneling low-intent individuals |
| Piracy of the binary | Compliance value (audit retention, tamper-evidence, exportable reports) can't be pirated; that's where the value is, not a locked-away SSO tier |

## Near-term, revenue-relevant build order

1. **Deepen switching cost now**: make routines/Zones/trust rules feel like the
   user's accumulating IP (export/import, naming, reuse). Every configured Zone
   should raise the cost of leaving.
2. **Make the audit log a compliance artifact**: retention, tamper-evidence, and
   a clean exportable report — every seat gets this, it is not a locked tier.
   (Rule-attributed export already shipped in #96; build retention + report
   formatting on top.)
3. **Prototype shared policy templates**: shared policy/playbook templates and
   org-wide trust rules — what makes a second, third, and fourth seat at the
   same firm an easy yes once the first ops lead trusts Ghost.
4. **Stand up a routine/template marketplace** (even read-only/community first)
   to seed the network effect and author ecosystem — later, once the wedge has
   real referenceable users, not a near-term dependency for revenue.

Pricing for the cloud product is the tiered table above (see also `CLAUDE.md`
and the marketing site). Early money is implementation services, then seats.

Near-term paid proof for the cloud MVP: land one customer workflow end-to-end
(record → approve → execute → verify → audit), then expand playbooks. Desktop
Organizer monetization notes elsewhere in this file are historical.
