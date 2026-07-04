# Business model & moat

Why a local-first desktop app can still win and still make money. Operational
notes, not a vision essay — pair with `docs/gtm-organizer.md` (persona/channels)
and `docs/PRODUCT_ROADMAP.md` (near-term build order).

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
   becoming a different company. Every buyer who *cannot* send data to a vendor
   cloud — regulated finance/legal/health, government, EU/GDPR-bound, and
   privacy-conscious SMBs — is ours by construction, not by feature parity. This
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
   - **Weekly-habit integration** for the bookkeeper wedge.

4. **A routine/template marketplace = a network effect bolted onto a local
   app.** The app runs locally; the *sharing layer* is networked. Community and
   verified playbooks make Ghost more valuable as more people publish — the
   Raycast Store / VS Code extensions / Alfred workflows pattern. This is how
   local-first earns a network effect without cloud lock-in.

5. **Vertical depth.** Being the *best* at the bookkeeper filing job (deep
   classification, the exact presets, compliance-grade audit export) beats a
   horizontal cloud agent on that niche's real pains. Depth is a moat via focus.

## Proof it's not hopeless — local-first that monetizes

These are the comps to reason from, not cloud-SaaS playbooks:

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

## The model — tiered, land on the wedge, monetize the firm

| Tier | Who | Price shape | What unlocks | Why they pay |
|---|---|---|---|---|
| **Free / Personal** | Individuals tidying their own machine | $0 | Organizer core: scan → plan → approve → move/rename → audit → undo | Adoption + trust; the top of funnel and the credibility engine |
| **Pro** | Prosumers, solo bookkeepers | subscription (Raycast/Obsidian band) | Unlimited Zones/routines, all presets, audit export + longer retention, priority builds, E2E sync across *your own* devices (paid relay, data stays encrypted) | Save real weekly hours; keep a portable compliance record |
| **Team / Business** | Bookkeeping firms, practices, MSPs — **the revenue engine** | per-seat | Shared/managed policy templates, org-wide trust rules, team playbook library, exportable audit across the team, SSO, MDM/enterprise deployment, signed builds, support/SLA | Businesses pay for **control, compliance, and support** — not for cloud. This is where local-first monetizes best |
| **Add-ons** | Any tier | usage/one-off | Verified marketplace premium templates (rev-share with authors), enterprise onboarding | Marketplace network effect + author ecosystem |

Complementary licensing option (Obsidian-proven, low friction): **free for
personal use, paid commercial-use license** — cleaner than metering for a
prosumer desktop tool.

Sequencing: **Free individual → Pro → Team.** Land with bookkeepers (see
`gtm-organizer.md`), then expand seat-by-seat into the firm. Do **not** try to
monetize the individual heavily; monetize control and compliance at the team
level.

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
| Low willingness to pay for a consumer utility | Revenue is B2B/Team where paying for control + compliance + support is normal; Free/Personal is funnel, not the business |
| Piracy of the binary | Team/compliance value (SSO, managed policy, support, audit retention) can't be pirated; that's where the money is |

## Near-term, revenue-relevant build order

1. **Deepen switching cost now**: make routines/Zones/trust rules feel like the
   user's accumulating IP (export/import, naming, reuse). Every configured Zone
   should raise the cost of leaving.
2. **Make the audit log a compliance artifact**: retention, tamper-evidence, and
   a clean exportable report — the paid Team/compliance hook. (Rule-attributed
   export already shipped in #96; build retention + report formatting on top.)
3. **Prototype the Team layer**: shared policy/playbook templates and org-wide
   trust rules — the first thing a bookkeeping *firm* pays per-seat for.
4. **Stand up a routine/template marketplace** (even read-only/community first)
   to seed the network effect and author ecosystem.
5. **Only then** publish pricing, and only with the time-to-value number earned
   from local instrumentation (`organizer_time_to_value`) — not before.

The first two paid features (audit-as-compliance-artifact, Team policy templates)
are scoped as concrete work in `docs/PRODUCT_ROADMAP.md` §6–§7.
