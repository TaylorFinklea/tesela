/**
 * APNs background-push helper — sends a single content-available
 * silent push to one device token via Apple's HTTP/2 API. Used by
 * sync durability Phase 3c to wake the iOS app when new ops land
 * in the relay so it can pull before the OS suspends it.
 *
 * Cloudflare Workers runtime only: Web Crypto (`crypto.subtle`),
 * `fetch`, `TextEncoder`, `atob`/`btoa`. No Node APIs.
 */

export interface ApnsEnv {
  APNS_KEY_P8?: string;    // PEM contents of the APNs auth key (.p8)
  APNS_KEY_ID?: string;    // 10-char APNs key id (JWT header `kid`)
  APNS_TEAM_ID?: string;   // 10-char Apple team id (JWT claim `iss`)
  APNS_BUNDLE_ID?: string; // e.g. "app.tesela.ios" — the `apns-topic`
  APNS_HOST?: string;      // optional override; default "https://api.push.apple.com"
}

/** Module-level JWT cache — APNs allows token reuse up to ~60 min;
 *  we re-mint after 50 min to stay well inside the window. Keyed by
 *  kid so swapping keys (rotation) invalidates automatically. */
let jwtCache: { kid: string; token: string; expiresAt: number } | null = null;
const JWT_CACHE_TTL_MS = 50 * 60 * 1000;

/**
 * Sends ONE content-available background push to `deviceTokenHex`.
 * Returns true on a 2xx from APNs; false on ANY failure OR if APNs
 * is not configured (any required env field missing). MUST NOT throw.
 */
export async function sendApnsBackgroundPush(
  env: ApnsEnv,
  deviceTokenHex: string,
): Promise<boolean> {
  try {
    // Config guard — the relay runs fine before the key is provisioned.
    const { APNS_KEY_P8, APNS_KEY_ID, APNS_TEAM_ID, APNS_BUNDLE_ID } = env;
    if (!APNS_KEY_P8 || !APNS_KEY_ID || !APNS_TEAM_ID || !APNS_BUNDLE_ID) {
      return false;
    }

    const token = await getOrMintJwt(APNS_KEY_P8, APNS_KEY_ID, APNS_TEAM_ID);
    const host = env.APNS_HOST || "https://api.push.apple.com";

    const res = await fetch(`${host}/3/device/${deviceTokenHex}`, {
      method: "POST",
      headers: {
        authorization: `bearer ${token}`,
        "apns-topic": APNS_BUNDLE_ID,
        "apns-push-type": "background",
        "apns-priority": "5",
        "apns-expiration": "0",
        "content-type": "application/json",
      },
      body: JSON.stringify({ aps: { "content-available": 1 } }),
    });
    return res.ok;
  } catch {
    // ANY unexpected error (crypto, network, parse) → false, never throw.
    return false;
  }
}

// ─── JWT (ES256 provider token) ───────────────────────────────────

async function getOrMintJwt(
  keyP8: string,
  kid: string,
  teamId: string,
): Promise<string> {
  const now = Date.now();
  if (jwtCache && jwtCache.kid === kid && now < jwtCache.expiresAt) {
    return jwtCache.token;
  }
  const token = await mintJwt(keyP8, kid, teamId);
  jwtCache = { kid, token, expiresAt: now + JWT_CACHE_TTL_MS };
  return token;
}

async function mintJwt(
  keyP8: string,
  kid: string,
  teamId: string,
): Promise<string> {
  const header = base64urlEncode(
    new TextEncoder().encode(JSON.stringify({ alg: "ES256", kid })),
  );
  const iat = Math.floor(Date.now() / 1000);
  const payload = base64urlEncode(
    new TextEncoder().encode(JSON.stringify({ iss: teamId, iat })),
  );
  const signingInput = `${header}.${payload}`;

  const der = pemToDer(keyP8);
  const cryptoKey = await crypto.subtle.importKey(
    "pkcs8",
    der,
    { name: "ECDSA", namedCurve: "P-256" },
    false,
    ["sign"],
  );
  // Web Crypto ECDSA returns raw r||s (IEEE P1361, 64 bytes for P-256)
  // — that IS the JWT ES256 signature. Base64url-encode directly.
  const sigBuf = await crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-256" },
    cryptoKey,
    new TextEncoder().encode(signingInput),
  );
  const signature = base64urlEncode(new Uint8Array(sigBuf));
  return `${signingInput}.${signature}`;
}

// ─── Encoding helpers ──────────────────────────────────────────────

/** Base64url (RFC 7515 §2): no `+`, `/`, or `=` padding. */
function base64urlEncode(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!);
  return btoa(bin)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

/** Strip PEM armor + whitespace, then base64-decode to raw DER bytes. */
function pemToDer(pem: string): Uint8Array {
  const b64 = pem
    .replace(/-----BEGIN PRIVATE KEY-----/, "")
    .replace(/-----END PRIVATE KEY-----/, "")
    .replace(/\s/g, "");
  const bin = atob(b64);
  const der = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) der[i] = bin.charCodeAt(i);
  return der;
}
