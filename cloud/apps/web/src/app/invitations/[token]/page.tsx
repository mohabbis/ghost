import { AcceptInvitation } from "@/components/accept-invitation";

/**
 * Invitation landing page.
 *
 * Deliberately outside the `(app)` group: someone arriving here may not be
 * signed in yet, and the app layout would bounce them to /signin and lose the
 * token. The client component handles both states.
 *
 * The token only ever travels in the path and is posted to the accept route;
 * nothing here reveals whether it is valid before the user is authenticated.
 */
export default async function InvitationPage({
  params,
}: {
  params: Promise<{ token: string }>;
}) {
  const { token } = await params;
  return (
    <div className="mx-auto flex min-h-screen max-w-md items-center px-6">
      <AcceptInvitation token={token} />
    </div>
  );
}
