# Azure Trusted Signing — setup and cost guardrails

Ghost signs its Windows installer with **Azure Trusted Signing** (see
[`RELEASING.md`](../RELEASING.md)). The actual signing is cheap and predictable;
the only real way to get a surprise Azure bill is to spin up *other* paid
resources in the same subscription and forget them. This doc covers both: how to
stand up signing, and how to make absolutely sure the bill stays bounded.

> TL;DR: Trusted Signing is a flat low monthly fee. Put it in its own resource
> group, set a Budget with alerts, and create nothing else. The resource group
> is your one-click off switch.

## What signing actually costs

- **Azure Trusted Signing** is billed at a **flat monthly rate** per account:
  - **Basic** — about **$9.99/month**, includes a large monthly signing quota
    (thousands of signatures — far more than a release pipeline uses).
  - **Premium** — about **$99.99/month**, for high-volume / many certificate
    profiles. Ghost does not need this.
- Signing a build consumes quota, not extra dollars, until you exceed the
  included quota — which a release pipeline realistically never will.
- The resources Trusted Signing requires are **not compute-billed**: the Trusted
  Signing account, a certificate profile, and an Entra (Azure AD) app
  registration / service principal. None of these bill per-hour.

**Translation:** if the only thing in your subscription is Trusted Signing, the
bill is ~$10/month, every month, full stop. There is no usage spike to fear from
signing itself.

## Where surprise bills actually come from

Not from signing — from unrelated resources created in the same subscription and
left running:

- VMs, AKS clusters, App Service plans (billed per hour, even when idle).
- Premium Key Vault HSM, premium storage, bandwidth egress.
- "Free trial" resources that silently convert to pay-as-you-go.

The guardrails below are designed so that even if you fat-finger something, the
blast radius is one resource group you can delete in seconds.

## One-time setup (signing only)

1. **Create a pay-as-you-go subscription** (or use an existing one you control).
2. **Make a dedicated resource group**, e.g. `rg-ghost-signing`, in a region that
   offers Trusted Signing (e.g. East US). Put *everything signing-related* in it
   and **nothing else**. This group is your kill switch.
3. **Create the Trusted Signing account** (Basic tier) in that resource group.
4. **Create a certificate profile** (Public Trust for a normally-distributed
   app). ⚠️ **Eligibility gotcha:** Public Trust certs require identity
   validation. An **organization** typically must have a verifiable legal
   existence of **3+ years**; **individual** validation is also supported but
   takes time. Start this early — it can gate your first signed release by days.
5. **Register an Entra app (service principal)** for CI and grant it the
   **Trusted Signing Certificate Profile Signer** role *scoped to the account*
   (not the whole subscription). Create a client secret for it.
6. Put the credentials into GitHub Actions secrets exactly as listed in
   `RELEASING.md` (`AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`,
   `AZURE_TS_ENDPOINT`, `AZURE_TS_ACCOUNT`, `AZURE_TS_PROFILE`).

## Cost guardrails — do these before you create anything

1. **Dedicated resource group (already above).** All signing resources live in
   `rg-ghost-signing`. To stop *all* spend: delete the group. One action, done.

2. **Set a Budget with alerts.** Azure Portal → *Cost Management + Billing* →
   *Cost Management* → *Budgets* → **+ Add**:
   - Scope: the subscription (or the resource group).
   - Amount: something comfortably above $10 but low enough to catch mistakes —
     e.g. **$25/month**.
   - Alert thresholds: **50%, 80%, 100%** (and optionally a *forecasted* 100%
     alert) emailing you.
   - ⚠️ **Budgets are alerts, not caps.** Azure does **not** stop spending when a
     budget is hit. The alert is your early warning; the resource group is the
     actual stop. Do not assume a budget will halt charges.

3. **Don't create compute.** No VMs, AKS, App Service, premium HSM. If a tutorial
   tells you to, you're on the wrong path — Trusted Signing needs none of it.

4. **Watch the spend.** Cost Management → *Cost analysis*, filtered to
   `rg-ghost-signing`, grouped by *Service name*. After the first full month you
   should see essentially one line item (~$10) and nothing else.

5. **(Optional) Hard auto-stop.** If you want a true cap, wire a Budget alert →
   *Action Group* → a Logic App / Function that deletes `rg-ghost-signing` on
   breach. This is overkill for a flat-rate $10 service and adds its own moving
   parts — the dedicated resource group already gives you a fast manual stop, so
   only do this if you specifically want spend that cannot run away unattended.

## Turning it off completely

Delete `rg-ghost-signing`. That removes the Trusted Signing account, the
certificate profile, and any associated resources, and stops the monthly charge.
The service principal lives in Entra ID (no cost) and can be removed separately
under *Microsoft Entra ID → App registrations*.

## Quick reference

| Item | Cost | Notes |
| --- | --- | --- |
| Trusted Signing (Basic) | ~$9.99/mo flat | Includes ample signing quota |
| Certificate profile | included | Public Trust needs identity validation |
| Entra service principal | free | CI auth; no compute |
| Budget + alerts | free | Alerts only — not a hard cap |
| Apple Developer (for comparison) | $99/yr flat | No per-build billing either |
