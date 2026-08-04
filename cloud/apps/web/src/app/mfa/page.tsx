import { redirect } from "next/navigation";
import { auth } from "@/auth";
import { MfaChallenge } from "@/components/mfa-challenge";

/**
 * The second-factor challenge.
 *
 * Lives outside the `(app)` group on purpose: everything in there is behind
 * the middleware gate that sends people *here*, so hosting the challenge
 * inside it would be a redirect loop.
 */
export default async function MfaPage({
  searchParams,
}: {
  searchParams: Promise<{ from?: string }>;
}) {
  const session = await auth();
  if (!session?.user) redirect("/signin");

  const { from } = await searchParams;
  // Only same-site paths: `from` comes off the query string, and handing it
  // to a redirect unchecked is an open redirect.
  const next = from && from.startsWith("/") && !from.startsWith("//") ? from : "/dashboard";

  return (
    <div className="mx-auto flex min-h-screen max-w-md items-center px-6">
      <MfaChallenge next={next} />
    </div>
  );
}
