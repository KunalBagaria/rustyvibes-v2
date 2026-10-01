import { formatBytes, type Release } from "../shared/release";

export const SECURITY_HEADERS: Readonly<Record<string, string>> = {
  "content-security-policy":
    "default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self'; " +
    "connect-src 'self'; font-src 'self'; media-src 'self' blob:; frame-ancestors 'none'; base-uri 'self'; form-action 'none'",
  "x-content-type-options": "nosniff",
  "referrer-policy": "strict-origin-when-cross-origin",
  "permissions-policy": "camera=(), microphone=(), geolocation=()",
};

/** Marks the page ready/soon and fills in release details. */
export function withRelease(page: Response, release: Release | null): Response {
  const rewritten = new HTMLRewriter()
    .on("html", {
      element(e) {
        e.setAttribute("data-release", release ? "ready" : "soon");
      },
    })
    .on("[data-release-version]", {
      element(e) {
        if (release) e.setInnerContent(release.version);
      },
    })
    .on("[data-release-size]", {
      element(e) {
        if (release) e.setInnerContent(formatBytes(release.size));
      },
    })
    .transform(page);
  const headers = new Headers(rewritten.headers);
  headers.delete("etag");
  headers.delete("content-length");
  for (const [name, value] of Object.entries(SECURITY_HEADERS)) headers.set(name, value);
  headers.set("cache-control", "public, max-age=0, must-revalidate");
  return new Response(rewritten.body, { status: rewritten.status, headers });
}
