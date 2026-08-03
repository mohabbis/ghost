-- Separation of duties: whoever triggers a run may not approve its gates.
--
-- Defaults to false so every existing workflow keeps today's behaviour rather
-- than silently acquiring a restriction on deploy. That default is also the only
-- correct one right now: organizations are auto-created single-member on first
-- sign-in and there is no invite flow, so turning this on by default would leave
-- most orgs with nobody eligible to approve and every run stuck at its first
-- gate.
ALTER TABLE "Workflow" ADD COLUMN "requireSeparateApprover" BOOLEAN NOT NULL DEFAULT false;
