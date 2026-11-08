// A small OpenID Connect provider for the e2e tests (phase 3 WP3-4): discovery, an
// authorization endpoint with its own sign-in form and a session cookie (so `prompt=none`
// works like a real provider with a live session), the token endpoint with PKCE S256 and
// client authentication, JWKS, UserInfo and RP-initiated logout. ID tokens are RS256,
// signed with a key generated at start-up by node:crypto; no npm packages.
//
// Written from the specifications only: OpenID Connect Core 1.0 (3.1.2.1 authorization
// request, 3.1.2.6 error codes, 3.1.3.3 token response, 2 ID token claims), Discovery 1.0
// (3), RP-Initiated Logout 1.0 (2), RFC 7636 (S256), RFC 7517 (JWK), RFC 7515 (JWS).
//
// Test controls: `POST /control` with JSON `{"outage": true}` makes every authorization
// request answer `error=temporarily_unavailable` (the provider outage path); `{"outage":
// false}` ends it. `GET /health` answers `ok`.
import { createServer } from "node:http";
import { createHash, generateKeyPairSync, randomBytes, sign } from "node:crypto";

const port = Number(process.env.OIDC_MOCK_PORT ?? 3012);
const issuer = `http://127.0.0.1:${port}`;
const SESSION = "mock_op_session";
const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const jwk = { ...publicKey.export({ format: "jwk" }), kid: "mock-1", alg: "RS256", use: "sig" };

const sessions = new Map(); // session id -> { user, authTime }
const codes = new Map(); // code -> { clientId, redirectUri, nonce, challenge, user, authTime, sid }
const tokens = new Map(); // access token -> user
let outage = false;

const b64url = (buf) => Buffer.from(buf).toString("base64url");
const now = () => Math.floor(Date.now() / 1000);
const id = () => b64url(randomBytes(18));

function jwt(claims) {
  const head = b64url(JSON.stringify({ alg: "RS256", kid: jwk.kid, typ: "JWT" }));
  const body = b64url(JSON.stringify(claims));
  const sig = sign("sha256", Buffer.from(`${head}.${body}`), privateKey);
  return `${head}.${body}.${b64url(sig)}`;
}

/** The claims of a test person: any username, a stable subject and a verified address. */
function person(user) {
  return {
    sub: `sub-${user}`,
    preferred_username: user,
    name: user.charAt(0).toUpperCase() + user.slice(1),
    email: `${user}@example.test`,
    email_verified: true,
  };
}

function cookies(req) {
  const out = {};
  for (const part of (req.headers.cookie ?? "").split(";")) {
    const [k, ...v] = part.trim().split("=");
    if (k) out[k] = v.join("=");
  }
  return out;
}

function send(res, status, body, headers = {}) {
  const json = typeof body !== "string";
  res.writeHead(status, {
    "content-type": json ? "application/json" : "text/html; charset=utf-8",
    "cache-control": "no-store",
    ...headers,
  });
  res.end(json ? JSON.stringify(body) : body);
}

function redirect(res, to, headers = {}) {
  res.writeHead(302, { location: to, "cache-control": "no-store", ...headers });
  res.end();
}

function back(redirectUri, params) {
  const u = new URL(redirectUri);
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null) u.searchParams.set(k, v);
  }
  return u.toString();
}

async function body(req) {
  let data = "";
  for await (const chunk of req) data += chunk;
  return data;
}

function escape(s) {
  return String(s).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

/** The provider's own sign-in form (the person "signs in at the provider"). */
function loginPage(query) {
  const hidden = Object.entries(query)
    .map(([k, v]) => `<input type="hidden" name="${escape(k)}" value="${escape(v)}">`)
    .join("");
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Mock provider</title></head>
<body><main><h1>Mock provider sign-in</h1>
<form method="post" action="/authorize">${hidden}
<label for="user">Mock username</label> <input id="user" name="user" autocomplete="off">
<button type="submit" name="decision" value="allow">Sign in at the mock provider</button>
<button type="submit" name="decision" value="deny">Deny</button>
</form></main></body></html>`;
}

function issueCode(res, q, user, authTime, sid, headers = {}) {
  const code = id();
  codes.set(code, {
    clientId: q.client_id,
    redirectUri: q.redirect_uri,
    nonce: q.nonce,
    challenge: q.code_challenge,
    user,
    authTime,
    sid,
  });
  redirect(res, back(q.redirect_uri, { code, state: q.state }), headers);
}

function authorize(req, res, q) {
  if (!q.redirect_uri || !q.client_id) {
    send(res, 400, "missing redirect_uri or client_id");
    return;
  }
  if (outage) {
    redirect(res, back(q.redirect_uri, { error: "temporarily_unavailable", state: q.state }));
    return;
  }
  if (q.code_challenge_method !== "S256" || !q.code_challenge) {
    redirect(res, back(q.redirect_uri, { error: "invalid_request", state: q.state }));
    return;
  }
  const sid = cookies(req)[SESSION];
  const live = sid && sessions.get(sid);
  // max_age=0 asks for a fresh sign-in (Core 3.1.2.1); prompt=login likewise
  const fresh = q.max_age === "0" || q.prompt === "login";
  if (q.prompt === "none") {
    if (!live || fresh) {
      redirect(res, back(q.redirect_uri, { error: "login_required", state: q.state }));
      return;
    }
    issueCode(res, q, live.user, live.authTime, sid);
    return;
  }
  if (live && !fresh) {
    issueCode(res, q, live.user, live.authTime, sid);
    return;
  }
  send(res, 200, loginPage(q));
}

async function authorizeForm(req, res) {
  const form = Object.fromEntries(new URLSearchParams(await body(req)));
  const { decision, user, ...q } = form;
  if (decision === "deny") {
    redirect(res, back(q.redirect_uri, { error: "access_denied", state: q.state }));
    return;
  }
  const name = (user ?? "").trim().toLowerCase();
  if (!name) {
    send(res, 200, loginPage(q));
    return;
  }
  const sid = id();
  const authTime = now();
  sessions.set(sid, { user: name, authTime });
  issueCode(res, q, name, authTime, sid, {
    "set-cookie": `${SESSION}=${sid}; Path=/; HttpOnly; SameSite=Lax`,
  });
}

function clientAuth(req, form) {
  const basic = req.headers.authorization ?? "";
  if (basic.startsWith("Basic ")) {
    const [user] = Buffer.from(basic.slice(6), "base64").toString().split(":");
    return decodeURIComponent(user);
  }
  return form.client_id;
}

async function token(req, res) {
  const form = Object.fromEntries(new URLSearchParams(await body(req)));
  const entry = codes.get(form.code);
  codes.delete(form.code);
  const clientId = clientAuth(req, form);
  if (!entry || form.grant_type !== "authorization_code") {
    send(res, 400, { error: "invalid_grant" });
    return;
  }
  const verifier = createHash("sha256").update(form.code_verifier ?? "").digest("base64url");
  if (entry.redirectUri !== form.redirect_uri || entry.clientId !== clientId || verifier !== entry.challenge) {
    send(res, 400, { error: "invalid_grant" });
    return;
  }
  const access = id();
  tokens.set(access, entry.user);
  const claims = {
    iss: issuer,
    aud: entry.clientId,
    iat: now(),
    exp: now() + 300,
    auth_time: entry.authTime,
    nonce: entry.nonce,
    sid: entry.sid,
    ...person(entry.user),
  };
  send(res, 200, { access_token: access, token_type: "Bearer", expires_in: 300, id_token: jwt(claims) });
}

function userinfo(req, res) {
  const user = tokens.get((req.headers.authorization ?? "").replace(/^Bearer /, ""));
  if (!user) {
    send(res, 401, { error: "invalid_token" });
    return;
  }
  send(res, 200, person(user));
}

function endSession(req, res, q) {
  const sid = cookies(req)[SESSION];
  if (sid) sessions.delete(sid);
  const clear = { "set-cookie": `${SESSION}=; Path=/; Max-Age=0` };
  if (q.post_logout_redirect_uri) {
    redirect(res, back(q.post_logout_redirect_uri, { state: q.state }), clear);
  } else {
    send(res, 200, "<!doctype html><title>Signed out</title><p>Signed out at the mock provider.</p>", clear);
  }
}

const discovery = {
  issuer,
  authorization_endpoint: `${issuer}/authorize`,
  token_endpoint: `${issuer}/token`,
  userinfo_endpoint: `${issuer}/userinfo`,
  jwks_uri: `${issuer}/jwks`,
  end_session_endpoint: `${issuer}/end-session`,
  response_types_supported: ["code"],
  subject_types_supported: ["public"],
  id_token_signing_alg_values_supported: ["RS256"],
  scopes_supported: ["openid", "profile", "email"],
  token_endpoint_auth_methods_supported: ["client_secret_basic", "client_secret_post"],
  code_challenge_methods_supported: ["S256"],
  claims_supported: ["sub", "iss", "aud", "exp", "iat", "auth_time", "nonce", "name", "email", "email_verified", "preferred_username"],
};

createServer(async (req, res) => {
  const url = new URL(req.url, issuer);
  const q = Object.fromEntries(url.searchParams);
  try {
    switch (`${req.method} ${url.pathname}`) {
      case "GET /health":
        send(res, 200, "ok");
        return;
      case "GET /.well-known/openid-configuration":
        send(res, 200, discovery);
        return;
      case "GET /jwks":
        send(res, 200, { keys: [jwk] });
        return;
      case "GET /authorize":
        authorize(req, res, q);
        return;
      case "POST /authorize":
        await authorizeForm(req, res);
        return;
      case "POST /token":
        await token(req, res);
        return;
      case "GET /userinfo":
        userinfo(req, res);
        return;
      case "GET /end-session":
        endSession(req, res, q);
        return;
      case "POST /control": {
        const c = JSON.parse((await body(req)) || "{}");
        if (typeof c.outage === "boolean") outage = c.outage;
        send(res, 200, { outage });
        return;
      }
      default:
        send(res, 404, { error: "not_found" });
    }
  } catch (e) {
    send(res, 500, { error: "server_error", error_description: String(e) });
  }
}).listen(port, "127.0.0.1", () => console.log(`oidc mock on ${port}`));
