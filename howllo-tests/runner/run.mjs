#!/usr/bin/env node
// Deterministic, isolated API scenario. Actor credentials never enter reports.
import { createHmac, randomBytes, randomUUID } from "node:crypto";
import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const dbUrl = process.env.HOWLLO_TEST_DATABASE_URL;
const database = dbUrl && new URL(dbUrl);
const dbName = database?.pathname.slice(1) ?? "";
if (!database || !/(test|e2e)/i.test(dbName) || !["127.0.0.1", "localhost"].includes(database.hostname)) {
  throw new Error("HOWLLO_TEST_DATABASE_URL must point to a local database with test/e2e in its name");
}
const port = Number(process.env.HOWLLO_TEST_PORT ?? 7710);
if (!Number.isInteger(port) || port < 7710 || port > 7799) throw new Error("Use an isolated port from 7710 to 7799");
const base = `http://127.0.0.1:${port}`;
const binary = resolve(root, "howllo-server/target/debug/howllo-server");
if (!existsSync(binary)) throw new Error("Build the test binary first: cargo build --manifest-path howllo-server/Cargo.toml");
const runId = randomUUID();
const suffix = runId.slice(0, 8);
const signingKey = randomBytes(32).toString("hex");
const records = [];
const traces = [];
const assets = { workspaceA: null, workspaceB: null, boards: {}, post: null };
const sockets = [];
let server;

function jwt(claims) {
  const encode = value => Buffer.from(JSON.stringify(value)).toString("base64url");
  const data = `${encode({ alg: "HS256", typ: "JWT" })}.${encode(claims)}`;
  return `${data}.${createHmac("sha256", signingKey).update(data).digest("base64url")}`;
}

function actor(id, ipOrdinal) {
  const email = `${id.toLowerCase()}-${suffix}@example.test`;
  const accountToken = jwt({ sub: `howllo-test-${runId}-${id}`, email, name: id, exp: Math.floor(Date.now() / 1000) + 3600 });
  const sessions = new Map();
  return Object.freeze({
    id,
    email,
    async request(method, path, payload, workspace, useAccount = false) {
      const token = useAccount ? accountToken : (sessions.get(workspace) ?? accountToken);
      return request(id, token, ipOrdinal, method, path, payload, workspace);
    },
    async join(workspace) {
      const response = await request(id, accountToken, ipOrdinal, "POST", "/api/auth/workspace-session", { tenant_slug: workspace }, workspace);
      check(`AUTH-JOIN-${id}`, id, workspace, response.status === 201 && typeof response.body?.session_token === "string", "workspace session created", response);
      if (response.status !== 201) throw new Error(`${id} could not join ${workspace}`);
      sessions.set(workspace, response.body.session_token);
    },
    ticket(workspace) {
      const token = sessions.get(workspace);
      if (!token) throw new Error(`${id} has no workspace session for realtime observer`);
      return token;
    },
  });
}

async function openSocket(token, workspace) {
  const address = new URL(`ws://127.0.0.1:${port}/ws`);
  address.searchParams.set("tenant_slug", workspace);
  address.searchParams.set("ticket", token);
  const socket = new WebSocket(address);
  sockets.push(socket);
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("WebSocket handshake timed out")), 5000);
    socket.addEventListener("open", () => { clearTimeout(timer); resolve(); }, { once: true });
    socket.addEventListener("error", () => { clearTimeout(timer); reject(new Error("WebSocket handshake rejected")); }, { once: true });
  });
  return socket;
}

async function request(actorId, token, ipOrdinal, method, path, payload, workspace) {
  const headers = { "cf-connecting-ip": `198.51.100.${ipOrdinal}`, "x-request-id": randomUUID() };
  if (token) headers.authorization = `Bearer ${token}`;
  if (payload !== undefined) headers["content-type"] = "application/json";
  const response = await fetch(`${base}${path}`, {
    method, headers, body: payload === undefined ? undefined : JSON.stringify(payload), signal: AbortSignal.timeout(10000),
  });
  const raw = await response.text();
  let body;
  try { body = raw ? JSON.parse(raw) : null; } catch { body = raw.slice(0, 300); }
  const result = { status: response.status, body, requestId: response.headers.get("x-request-id") };
  traces.push({ actor_id: actorId, workspace_id: workspace ?? null, method, path, request_id: result.requestId, status: result.status });
  return result;
}

function check(id, actorId, workspace, okay, expected, observed, severity = "P1") {
  records.push({ id, actor: actorId, workspace, status: okay ? "PASS" : "FAIL", severity: okay ? null : severity,
    expected, observed: { status: observed?.status ?? null, request_id: observed?.requestId ?? null,
      detail: typeof observed?.body === "string" ? observed.body : (observed?.body?.error ?? null) } });
  return okay;
}

function requireCheck(id, actorId, workspace, okay, expected, observed, severity) {
  if (!check(id, actorId, workspace, okay, expected, observed, severity)) throw new Error(`${id} failed`);
}

function uuid(value) {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value ?? "")) throw new Error("invalid observed UUID");
  return value;
}

function observe(sql) {
  // Independent database observer. Even a mistaken SQL statement cannot write.
  const stdout = execFileSync("psql", ["--no-psqlrc", "--tuples-only", "--no-align", "--dbname", dbUrl, "--command", sql],
    { encoding: "utf8", env: { ...process.env, PGOPTIONS: "-c default_transaction_read_only=on" }, timeout: 10000 }).trim();
  return stdout ? JSON.parse(stdout) : null;
}

async function waitReady() {
  for (let i = 0; i < 60; i++) {
    if (server.exitCode !== null) throw new Error(`isolated API exited with ${server.exitCode}`);
    try { if ((await fetch(`${base}/api/ready`, { signal: AbortSignal.timeout(1000) })).ok) return; } catch { /* starting */ }
    await new Promise(resolve => setTimeout(resolve, 500));
  }
  throw new Error("isolated API did not become ready");
}

function discover() {
  return JSON.parse(execFileSync("python3", [resolve(root, "howllo-tests/discovery/discover.py"), "--json", "--root", root],
    { encoding: "utf8", timeout: 10000 }));
}

async function scenario(manifest) {
  const presets = manifest.board_types.frontend_presets;
  requireCheck("CAP-BOARD-TYPES", "SYS-01", null, presets.length > 0, "at least one dynamically discovered board preset", { status: presets.length });
  const ownerA = actor("A-OWNER", 11), ownerB = actor("B-OWNER", 12), admin = actor("A-ADMIN-1", 20),
    moderator = actor("A-MOD", 21), rejectedInvitee = actor("INV-REJECT", 22),
    author = actor("U-FEATURE", 13),
    voter1 = actor("U-VOTER-1", 14), voter2 = actor("U-VOTER-2", 15), voter3 = actor("U-VOTER-3", 16),
    answer = actor("U-ANSWER-1", 17), foreign = actor("U-FOREIGN", 18), guest = actor("U-GUEST", 19);
  const people = [ownerA, ownerB, admin, moderator, rejectedInvitee, author, voter1, voter2, voter3, answer, foreign];
  const identities = new Set();
  for (const person of people) {
    const me = await person.request("GET", "/api/me", undefined, null, true);
    requireCheck(`AUTH-${person.id}`, person.id, null, me.status === 200 && !!me.body?.id && !identities.has(me.body.id),
      "distinct authenticated user identity", me);
    identities.add(me.body.id);
  }
  const a = await ownerA.request("POST", "/api/admin/tenants", { name: `Howllo Test A ${suffix}` }, null, true);
  requireCheck("TENANT-A", ownerA.id, null, a.status === 201 && !!a.body?.slug, "workspace A created", a);
  assets.workspaceA = a.body;
  const b = await ownerB.request("POST", "/api/admin/tenants", { name: `Howllo Test B ${suffix}` }, null, true);
  requireCheck("TENANT-B", ownerB.id, null, b.status === 201 && !!b.body?.slug && b.body.id !== a.body.id, "distinct workspace B created", b);
  assets.workspaceB = b.body;
  const A = a.body.slug, B = b.body.slug;
  const dbTenants = observe(`SELECT json_build_object('count',count(*)) FROM tenants WHERE id IN ('${uuid(a.body.id)}','${uuid(b.body.id)}')`);
  requireCheck("DB-TENANTS", "SYS-02", A, dbTenants?.count === 2, "both API workspaces exist in observed database", { status: dbTenants?.count });

  for (const [person, role] of [[admin, "admin"], [moderator, "moderator"], [rejectedInvitee, "moderator"]]) {
    const invitation = await ownerA.request("POST", "/api/admin/invitations",
      { tenant_slug: A, email: person.email, role }, A, true);
    requireCheck(`INV-CREATE-${person.id}`, ownerA.id, A, invitation.status === 201 && !!invitation.body?.id,
      `${role} invitation created`, invitation);
    const inviteId = uuid(invitation.body.id);
    const pending = observe(`SELECT json_build_object('status',status,'tenant_id',tenant_id) FROM workspace_invitations WHERE id='${inviteId}'`);
    check(`INV-DB-PENDING-${person.id}`, "SYS-02", A, pending?.status === "pending" && pending?.tenant_id === a.body.id,
      "invitation pending in correct workspace", { status: 200, body: pending });
    const inbox = await person.request("GET", "/api/me/invitations", undefined, null, true);
    requireCheck(`INV-INBOX-${person.id}`, person.id, A, inbox.status === 200 && Array.isArray(inbox.body) && inbox.body.some(item => item.id === inviteId),
      "invitee sees only own pending invitation", inbox);
    const decision = person === rejectedInvitee ? "reject" : "accept";
    const response = await person.request("POST", `/api/me/invitations/${inviteId}/${decision}`, undefined, null, true);
    requireCheck(`INV-${decision.toUpperCase()}-${person.id}`, person.id, A, response.status === 200,
      `invitee ${decision}s invitation`, response);
    const replay = await person.request("POST", `/api/me/invitations/${inviteId}/accept`, undefined, null, true);
    check(`INV-REPLAY-${person.id}`, person.id, A, replay.status >= 400,
      "resolved invitation cannot be replayed", replay, "P0");
    const state = observe(`SELECT json_build_object('status',i.status,'role',m.role,'audit_count',(SELECT count(*) FROM audit_logs al WHERE al.entity_id=i.id AND al.action='invitation_${decision === "accept" ? "accepted" : "rejected"}')) FROM workspace_invitations i LEFT JOIN memberships m ON m.tenant_id=i.tenant_id AND m.user_id=i.user_id WHERE i.id='${inviteId}'`);
    check(`INV-DB-RESOLVED-${person.id}`, "SYS-02", A,
      state?.status === (decision === "accept" ? "accepted" : "rejected") &&
      (decision === "accept" ? state.role === role : state.role == null) && state.audit_count === 1,
      "invitation, membership and audit reflect one decision", { status: 200, body: state }, "P0");
    if (decision === "accept") await person.join(A);
  }
  const moderatorInvite = await moderator.request("POST", "/api/admin/invitations",
    { tenant_slug: A, email: `forbidden-${suffix}@example.test`, role: "admin" }, A);
  check("RBAC-MOD-INVITE-DENIED", moderator.id, A, moderatorInvite.status === 403,
    "moderator cannot invite staff", moderatorInvite, "P1");
  const rejectAccess = await rejectedInvitee.request("POST", "/api/admin/boards",
    { tenant_slug: A, slug: `rejected-${suffix}`, name: "Forbidden", board_type: "discussions", is_private: false }, A, true);
  check("RBAC-REJECT-NO-STAFF-ACCESS", rejectedInvitee.id, A, rejectAccess.status === 403,
    "rejected invite grants no staff permission", rejectAccess, "P0");

  for (const preset of presets) {
    const slug = `${preset.type}-${suffix}`;
    const payload = { tenant_slug: A, slug, name: preset.type, board_type: preset.type,
      is_private: false, allow_votes: preset.default_votes, allow_comments: preset.default_comments };
    const created = await ownerA.request("POST", "/api/admin/boards", payload, A, true);
    requireCheck(`BOARD-CREATE-${preset.type}`, ownerA.id, A, created.status === 201 && created.body?.board_type === preset.type,
      `board ${preset.type} created with discovered type`, created);
    assets.boards[preset.type] = created.body;
    const enabled = await ownerA.request("PATCH", `/api/admin/boards/${uuid(created.body.id)}`,
      { name: preset.type, board_type: preset.type, is_private: false, is_enabled: true,
        allow_votes: preset.default_votes, allow_comments: preset.default_comments }, A, true);
    requireCheck(`BOARD-ENABLE-${preset.type}`, ownerA.id, A, enabled.status === 200 && enabled.body?.is_enabled === true,
      "board enabled", enabled);
  }
  const chosen = presets.find(p => p.type === "feature-requests") ?? presets[0];
  const chosenBoard = assets.boards[chosen.type];
  const privateBoard = await ownerA.request("POST", "/api/admin/boards",
    { tenant_slug: A, slug: `private-${suffix}`, name: "Private test", board_type: chosen.type, is_private: true }, A, true);
  requireCheck("BOARD-PRIVATE", ownerA.id, A, privateBoard.status === 201, "private board created", privateBoard);
  const privateEnabled = await ownerA.request("PATCH", `/api/admin/boards/${uuid(privateBoard.body.id)}`,
    { name: "Private test", board_type: chosen.type, is_private: true, is_enabled: true }, A, true);
  requireCheck("BOARD-PRIVATE-ENABLE", ownerA.id, A, privateEnabled.status === 200, "private board enabled", privateEnabled);
  const published = await ownerA.request("PATCH", `/api/admin/workspace-publication?tenant_slug=${encodeURIComponent(A)}`,
    { is_published: true }, A, true);
  requireCheck("TENANT-PUBLISH", ownerA.id, A, published.status === 200, "workspace published after boards enabled", published);

  for (const person of [author, voter1, voter2, voter3, answer]) await person.join(A);
  await foreign.join(B);
  const list = await guest.request("GET", `/api/boards?tenant_slug=${encodeURIComponent(A)}`, undefined, A);
  check("BOARD-PUBLIC-LIST", guest.id, A, list.status === 200 && Array.isArray(list.body) && list.body.some(item => item.slug === chosenBoard.slug),
    "guest sees enabled public board", list);
  const guestPrivate = await guest.request("GET", `/api/boards/${privateBoard.body.slug}?tenant_slug=${encodeURIComponent(A)}`, undefined, A);
  check("SEC-PRIVATE-GUEST", guest.id, A, guestPrivate.status === 403 || guestPrivate.status === 404,
    "guest cannot read private board", guestPrivate, "P0");
  const foreignPrivate = await foreign.request("GET", `/api/boards/${privateBoard.body.slug}?tenant_slug=${encodeURIComponent(A)}`, undefined, B);
  check("SEC-PRIVATE-FOREIGN", foreign.id, A, foreignPrivate.status === 403 || foreignPrivate.status === 404,
    "workspace B session cannot read workspace A private board", foreignPrivate, "P0");
  const deniedBoard = await author.request("POST", "/api/admin/boards",
    { tenant_slug: A, slug: `forged-${suffix}`, name: "Forged", board_type: chosen.type, is_private: false }, A);
  check("BOARD-MEMBER-DENIED", author.id, A, deniedBoard.status === 403 || deniedBoard.status === 404,
    "member cannot create board through API", deniedBoard, "P1");

  const createdPost = await author.request("POST", `/api/boards/${chosenBoard.slug}/posts`,
    { tenant_slug: A, title: `Feature ${suffix}`, body: `A deterministic feature request ${suffix}.` }, A);
  requireCheck("FEATURE-CREATE", author.id, A, createdPost.status === 201 && !!createdPost.body?.id,
    "member creates feature request", createdPost);
  assets.post = createdPost.body.id;
  const postId = uuid(createdPost.body.id);
  const postScope = observe(`SELECT json_build_object('tenant_id',tenant_id,'board_id',board_id,'user_id',user_id,'review_state',review_state) FROM posts WHERE id='${postId}'`);
  requireCheck("FEATURE-DB-SCOPE", "SYS-02", A, postScope?.tenant_id === a.body.id && postScope?.board_id === chosenBoard.id,
    "post belongs to workspace A and chosen board", { status: 200, body: postScope });
  const otherEdit = await voter1.request("PATCH", `/api/posts/${postId}`, { title: "Stolen", body: "Unauthorized edit" }, A);
  check("FEATURE-OTHER-EDIT", voter1.id, A, otherEdit.status === 403 || otherEdit.status === 404,
    "other member cannot edit author's post", otherEdit, "P1");
  const foreignPost = await foreign.request("POST", `/api/boards/${chosenBoard.slug}/posts`,
    { tenant_slug: A, title: "Cross-tenant post", body: "Should be rejected" }, B);
  check("SEC-FOREIGN-POST", foreign.id, A, foreignPost.status === 403 || foreignPost.status === 404,
    "workspace B session cannot post to A", foreignPost, "P0");

  if (chosen.default_votes) {
    const votes = await Promise.all([voter1, voter2, voter3].map(person => person.request("POST", `/api/posts/${postId}/vote`, undefined, A)));
    check("VOTE-CONCURRENT", "SYS-01", A, votes.every(v => v.status === 200), "three distinct voters succeed concurrently",
      { status: votes.map(v => v.status).join(",") });
    await Promise.all(Array.from({ length: 10 }, () => voter1.request("POST", `/api/posts/${postId}/vote`, undefined, A)));
    const counts = observe(`SELECT json_build_object('cached',p.vote_count,'actual',(SELECT count(*) FROM post_votes v WHERE v.post_id=p.id)) FROM posts p WHERE p.id='${postId}'`);
    check("VOTE-COUNT-INVARIANT", "SYS-02", A, counts?.cached === 3 && counts?.actual === 3,
      "one logical vote per user and cached count equals rows", { status: 200, body: counts }, "P0");
    const deniedVote = await foreign.request("POST", `/api/posts/${postId}/vote`, undefined, B);
    check("SEC-FOREIGN-VOTE", foreign.id, A, deniedVote.status === 403 || deniedVote.status === 404,
      "foreign workspace session cannot vote", deniedVote, "P0");
  } else {
    records.push({ id: "VOTE-CONCURRENT", actor: "SYS-01", workspace: A, status: "MISSING_CAPABILITY",
      expected: "a discovered board preset with votes enabled", observed: null });
  }
  if (chosen.default_comments) {
    const first = await voter2.request("POST", `/api/posts/${postId}/comments`, { body: "First answer" }, A);
    const second = await answer.request("POST", `/api/posts/${postId}/comments`, { body: "Second answer" }, A);
    check("COMMENTS-TWO-ACTORS", "SYS-02", A, first.status === 201 && second.status === 201,
      "two different users comment", { status: `${first.status},${second.status}` });
    const count = observe(`SELECT json_build_object('count',count(*)) FROM comments WHERE post_id='${postId}'`);
    check("COMMENTS-DB-INVARIANT", "SYS-02", A, count?.count === 2, "exactly two comments exist", { status: 200, body: count });
  }
  const realtimeA = [], realtimeB = [];
  const socketA = await openSocket(author.ticket(A), A);
  const socketB = await openSocket(foreign.ticket(B), B);
  socketA.addEventListener("message", event => { try { realtimeA.push(JSON.parse(event.data)); } catch { /* malformed checked below */ } });
  socketB.addEventListener("message", event => { try { realtimeB.push(JSON.parse(event.data)); } catch { /* malformed checked below */ } });
  check("REALTIME-ISOLATED-SOCKETS", "SYS-04", A, socketA.readyState === WebSocket.OPEN && socketB.readyState === WebSocket.OPEN,
    "distinct workspace subscribers are connected", { status: 101 });
  let crossSocketRejected = false;
  try { await openSocket(author.ticket(A), B); } catch { crossSocketRejected = true; }
  check("REALTIME-CROSS-WORKSPACE-DENIED", "SYS-04", A, crossSocketRejected,
    "workspace A session cannot subscribe to B", { status: crossSocketRejected ? 403 : 101 }, "P0");
  const deniedStatus = await author.request("PATCH", `/api/admin/posts/${postId}/status`, { status: "planned" }, A);
  check("RBAC-MEMBER-STATUS-DENIED", author.id, A, deniedStatus.status === 403,
    "member cannot change post status directly", deniedStatus, "P1");
  const moderatorStatus = await moderator.request("PATCH", `/api/admin/posts/${postId}/status`, { status: "planned" }, A);
  if (manifest.role_policy.grants.moderator?.includes("ChangeStatus")) {
    check("STATUS-MODERATOR-POLICY", moderator.id, A, moderatorStatus.status === 200,
      "moderator has configured status permission", moderatorStatus, "P1");
  } else {
    check("STATUS-MODERATOR-DENIED", moderator.id, A, moderatorStatus.status === 403,
      "backend enforces current moderator role policy", moderatorStatus, "P1");
    records.push({ id: "STATUS-MODERATOR-TARGET", actor: "SYS-01", workspace: A, status: "MISSING_CAPABILITY",
      expected: "target matrix grants moderator ChangeStatus", observed: { role_policy: manifest.role_policy.source } });
  }
  const invalidTransition = await admin.request("PATCH", `/api/admin/posts/${postId}/status`, { status: "done" }, A);
  check("STATUS-INVALID-TRANSITION", admin.id, A, invalidTransition.status >= 400,
    "admin cannot skip from under_review to done", invalidTransition, "P1");
  for (const [from, to] of [["under_review", "planned"], ["planned", "in_progress"], ["in_progress", "done"]]) {
    const changed = await admin.request("PATCH", `/api/admin/posts/${postId}/status`, { status: to }, A);
    requireCheck(`STATUS-${from}-${to}`, admin.id, A, changed.status === 200,
      `allowed transition ${from} to ${to}`, changed);
  }
  const statusState = observe(`SELECT json_build_object('status',p.status,'history',(SELECT count(*) FROM post_status_history h WHERE h.post_id=p.id),'audit',(SELECT count(*) FROM audit_logs a WHERE a.entity_id=p.id AND a.action='post_status_changed')) FROM posts p WHERE p.id='${postId}'`);
  check("STATUS-DB-AUDIT", "SYS-03", A, statusState?.status === "done" && statusState?.history === 3 && statusState?.audit === 3,
    "post status, history, and audit agree", { status: 200, body: statusState }, "P0");
  await new Promise(resolve => setTimeout(resolve, 250));
  check("REALTIME-STATUS-SAME-WORKSPACE", "SYS-04", A,
    realtimeA.filter(event => event.event_type === "post.status_changed" && event.post_id === postId).length === 3,
    "subscriber A receives three persisted status transitions", { status: realtimeA.length }, "P1");
  check("REALTIME-NO-CROSS-WORKSPACE-LEAK", "SYS-04", B,
    realtimeB.every(event => event.post_id !== postId),
    "workspace B subscriber receives no A post event", { status: realtimeB.length }, "P0");
  if (chosen.default_comments) {
    const locked = await moderator.request("PATCH", `/api/admin/posts/${postId}/lock`, { is_locked: true }, A);
    requireCheck("MOD-LOCK", moderator.id, A, locked.status === 200, "moderator locks post", locked);
    const blockedComment = await author.request("POST", `/api/posts/${postId}/comments`, { body: "Should be blocked" }, A);
    check("MOD-LOCK-BLOCKS-COMMENT", author.id, A, blockedComment.status >= 400,
      "locked post rejects new comment", blockedComment, "P1");
    const unlocked = await moderator.request("PATCH", `/api/admin/posts/${postId}/lock`, { is_locked: false }, A);
    requireCheck("MOD-UNLOCK", moderator.id, A, unlocked.status === 200, "moderator unlocks post", unlocked);
  }
  const hidden = await moderator.request("PATCH", `/api/admin/posts/${postId}/visibility`, { is_hidden: true }, A);
  requireCheck("MOD-HIDE", moderator.id, A, hidden.status === 200, "moderator hides post", hidden);
  const hiddenGuest = await guest.request("GET", `/api/posts/${postId}?tenant_slug=${encodeURIComponent(A)}`, undefined, A);
  check("MOD-HIDDEN-GUEST", guest.id, A, hiddenGuest.status === 403 || hiddenGuest.status === 404,
    "guest cannot read hidden post", hiddenGuest, "P0");
  const unhidden = await moderator.request("PATCH", `/api/admin/posts/${postId}/visibility`, { is_hidden: false }, A);
  requireCheck("MOD-UNHIDE", moderator.id, A, unhidden.status === 200, "moderator restores visibility", unhidden);
  const visibleGuest = await guest.request("GET", `/api/posts/${postId}?tenant_slug=${encodeURIComponent(A)}`, undefined, A);
  check("MOD-VISIBLE-GUEST", guest.id, A, visibleGuest.status === 200,
    "guest can read restored public post", visibleGuest);

  const duplicate = await voter3.request("POST", `/api/boards/${chosenBoard.slug}/posts`,
    { tenant_slug: A, title: `Feature ${suffix}`, body: "A deliberate duplicate for moderation." }, A);
  requireCheck("MOD-DUPLICATE-CREATE", voter3.id, A, duplicate.status === 201 && !!duplicate.body?.id,
    "another member submits duplicate title", duplicate);
  const duplicateId = uuid(duplicate.body.id);
  const pendingState = observe(`SELECT json_build_object('review_state',review_state,'is_hidden',is_hidden) FROM posts WHERE id='${duplicateId}'`);
  requireCheck("MOD-DUPLICATE-PENDING", "SYS-02", A, pendingState?.review_state === "pending" && pendingState?.is_hidden === true,
    "duplicate enters review queue and stays hidden", { status: 200, body: pendingState }, "P0");
  const deniedReview = await author.request("PATCH", `/api/admin/posts/${duplicateId}/review`, { action: "approve" }, A);
  check("RBAC-MEMBER-REVIEW-DENIED", author.id, A, deniedReview.status === 403,
    "member cannot review pending post", deniedReview, "P1");
  const decisions = await Promise.all([admin.request("PATCH", `/api/admin/posts/${duplicateId}/review`, { action: "approve" }, A),
    moderator.request("PATCH", `/api/admin/posts/${duplicateId}/review`, { action: "reject" }, A)]);
  check("MOD-CONCURRENT-ONE-WINNER", "SYS-01", A,
    decisions.filter(row => row.status === 200).length === 1 && decisions.filter(row => row.status >= 400).length === 1,
    "one concurrent moderator decision wins and the other conflicts", { status: decisions.map(row => row.status).join(",") }, "P0");
  const reviewed = observe(`SELECT json_build_object('review_state',p.review_state,'is_hidden',p.is_hidden,'audit',(SELECT count(*) FROM audit_logs a WHERE a.entity_id=p.id AND a.action IN ('post_approved','post_rejected'))) FROM posts p WHERE p.id='${duplicateId}'`);
  check("MOD-CONCURRENT-DB-AUDIT", "SYS-03", A,
    ["approved", "rejected"].includes(reviewed?.review_state) && reviewed?.is_hidden === (reviewed?.review_state === "rejected") && reviewed?.audit === 1,
    "one final review state and one decision audit", { status: 200, body: reviewed }, "P0");
  const audit = observe(`SELECT json_build_object('board_create',count(*) FILTER (WHERE entity_type='board' AND action='board_created'),'all',count(*)) FROM audit_logs WHERE tenant_id='${uuid(a.body.id)}'`);
  check("AUDIT-BOARD", "SYS-03", A, audit?.board_create >= presets.length, "board mutations have workspace audit entries", { status: 200, body: audit });
}

async function main() {
  const reportPath = resolve(root, "howllo-tests/reports", `${runId}.json`);
  const manifest = discover();
  try {
    try {
      const response = await fetch(`${base}/api/ready`, { signal: AbortSignal.timeout(500) });
      if (response.ok) throw new Error(`Port ${port} already serves an API; refusing to mutate it`);
    } catch (error) {
      if (String(error).includes("refusing to mutate")) throw error;
    }
    const existing = observe("SELECT json_build_object('count',count(*)) FROM pg_tables WHERE schemaname='public'");
    requireCheck("DB-CLEAN-START", "SYS-02", null, existing?.count === 0,
      "fresh empty test database before migrations", { status: 200, body: existing }, "P0");
    const environment = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith("HOWLLO_") && key !== "DATABASE_URL"));
    // Run outside the repo: dotenvy must not import production .env settings.
    const isolatedDir = mkdtempSync(join(tmpdir(), "howllo-e2e-"));
    server = spawn(binary, [], { cwd: isolatedDir, stdio: ["ignore", "pipe", "pipe"],
      env: { ...environment, HOWLLO_DATABASE_URL: dbUrl, HOWLLO_BIND_ADDRESS: `127.0.0.1:${port}`,
        HOWLLO_JWT_SECRET: signingKey, HOWLLO_ADMIN_JWT_SECRET: randomBytes(32).toString("hex"),
        HOWLLO_ROOIAM_LEGACY_HS256_ENABLED: "true", HOWLLO_AUTH_LOCAL_ENABLED: "true",
        HOWLLO_STORAGE_ROOT: resolve(isolatedDir, "uploads"), HOWLLO_RATE_LIMIT_ENABLED: "true",
        HOWLLO_MINIO_ENDPOINT: "", HOWLLO_MINIO_BUCKET: "", HOWLLO_MINIO_USER: "", HOWLLO_MINIO_PASSWORD: "" } });
    server.stdout.resume();
    server.stderr.resume();
    await waitReady();
    await scenario(manifest);
  } catch (error) {
    records.push({ id: "RUNNER", actor: "SYS-01", workspace: null, status: "FAIL", severity: "P1",
      expected: "all deterministic scenarios complete", observed: String(error) });
  } finally {
    for (const socket of sockets) socket.close();
    if (server && server.exitCode === null) { server.kill("SIGTERM"); await new Promise(resolve => server.once("exit", resolve)); }
    mkdirSync(dirname(reportPath), { recursive: true });
    const missing = [...new Set([
      ...Object.entries(manifest.capabilities).filter(([, value]) => value.status === "MISSING_CAPABILITY").map(([name]) => name),
      ...records.filter(row => row.status === "MISSING_CAPABILITY").map(row => row.id),
    ])];
    const failed = records.filter(row => row.status === "FAIL");
    const report = { run_id: runId, generated_at: new Date().toISOString(), target: "isolated-local-api", certification: "NOT_CERTIFIED",
      summary: { passed: records.filter(row => row.status === "PASS").length, failed: failed.length,
        missing_capabilities: missing.length }, missing_capabilities: missing, cases: records, traces,
      resources: assets,
      untested_layers: ["full browser state matrix", "complete realtime event matrix", "AI exploratory actions", "all role/persona combinations", "cross-workspace endpoint sweep"],
      note: "API regression only. PASS is not full-system certification." };
    writeFileSync(reportPath, JSON.stringify(report, null, 2));
    console.log(`${failed.length ? "FAIL" : "PASS"}: ${report.summary.passed} passed, ${failed.length} failed, ${missing.length} missing capabilities`);
    console.log(`Report: ${reportPath}`);
    if (failed.length) process.exitCode = 1;
  }
}

await main();
