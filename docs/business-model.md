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

## The model — one price, land on the wedge, expand seat-by-seat

Ghost is **$79/month per seat, flat** — no tiers, no "contact sales," no
enterprise pricing page (see `CLAUDE.md` "Product identity"). Priced to be
expensable by the ops lead without a manager's signoff, and high enough to
cover LLM spend.

| Who | What they get | Why they pay |
|---|---|---|
| The ops lead at a 10–50 person wealth-management or accounting firm | The full trust pipeline: Organizer core, Zones/routines, all filing presets, exportable/tamper-evident audit, undo | Client data barred from cloud tools by firm policy; this is the only automation option that's structurally allowed |
| Additional seats at the same firm | Same product, same price per seat | Land with one ops lead (see `gtm-organizer.md`), expand seat-by-seat into the firm as trust compounds |

There is no free tier, no usage metering, and no separate "Team" SKU — every
seat gets the same product at the same price. Firm-wide rollout is a matter of
adding seats, not upgrading a tier. This keeps the pitch honest to the kill
list in `CLAUDE.md`: no enterprise motion (SSO, compliance questionnaires,
"contact sales") until real pull justifies it, not before.

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

Pricing ($79/seat, flat) is already published — see `CLAUDE.md`. The remaining
work here is proving time-to-value with real instrumentation
(`organizer_time_to_value`), not gating pricing on it.

The first paid-relevant feature (audit-as-compliance-artifact) is scoped as
concrete work in `docs/PRODUCT_ROADMAP.md` §6.
