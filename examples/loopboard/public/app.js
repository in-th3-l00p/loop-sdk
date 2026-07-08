/* loopboard frontend — vanilla js, no build step. talks to the loop
backend over fetch (bearer tokens), a live websocket for the board, an
event-source for personal activity, and window.ethereum (EIP-1193) for
the self-custodial door: SIWE login and client-signed usdc transfers. */

const $ = (id) => document.getElementById(id);
const state = {
  token: localStorage.getItem("token"),
  profile: null,
  provider: null,
};

// find an injected wallet. modern wallets announce themselves over EIP-6963
// (so several can coexist without fighting over window.ethereum); we collect
// those and fall back to the legacy window.ethereum for older wallets.
function walletProvider() {
  return new Promise((resolve) => {
    const found = [];
    const onAnnounce = (event) => found.push(event.detail.provider);
    window.addEventListener("eip6963:announceProvider", onAnnounce);
    window.dispatchEvent(new Event("eip6963:requestProvider"));
    setTimeout(() => {
      window.removeEventListener("eip6963:announceProvider", onAnnounce);
      resolve(found[0] || window.ethereum || null);
    }, 100);
  });
}

// ------------------------------------------------------------------ api

async function api(path, body, method) {
  const options = {
    method: method || (body === undefined ? "GET" : "POST"),
    headers: {},
  };
  if (body !== undefined) {
    options.headers["content-type"] = "application/json";
    options.body = JSON.stringify(body);
  }
  if (state.token) options.headers.authorization = `Bearer ${state.token}`;
  const response = await fetch(path, options);
  const payload = await response.json().catch(() => null);
  if (!response.ok) {
    throw new Error(payload?.error || `${response.status}`);
  }
  return payload;
}

function toast(message, isError) {
  const el = document.createElement("div");
  el.className = "toast" + (isError ? " error" : "");
  el.textContent = message;
  $("toasts").append(el);
  setTimeout(() => el.remove(), 6000);
}

// ---------------------------------------------------------------- session

async function establish(session) {
  state.token = session.token;
  localStorage.setItem("token", session.token);
  await enter();
}

async function enter() {
  try {
    state.profile = await api("/me");
  } catch {
    state.token = null;
    localStorage.removeItem("token");
    show("login");
    return;
  }
  show("app");
  $("who").innerHTML = "";
  const who = document.createElement("span");
  who.append(`${state.profile.handle} · `);
  const credits = document.createElement("b");
  credits.textContent = `${state.profile.credits} credits`;
  who.append(credits);
  $("who").append(who);
  renderWallets();
  connectFeed();
  refreshWallet().catch(() => {});
}

function show(view) {
  $("login").classList.toggle("hidden", view !== "login");
  $("app").classList.toggle("hidden", view !== "app");
}

// email one-time code door
$("send-code").onclick = async () => {
  try {
    await api("/auth/otp/send", { email: $("email").value });
    $("email-step-1").classList.add("hidden");
    $("email-step-2").classList.remove("hidden");
    toast("code sent — check the dev server console");
  } catch (e) {
    toast(e.message, true);
  }
};

$("verify-code").onclick = async () => {
  try {
    const session = await api("/auth/otp/verify", {
      email: $("email").value,
      code: $("code").value.trim(),
    });
    await establish(session);
  } catch (e) {
    toast(e.message, true);
  }
};

// sign-in-with-ethereum door (EIP-1193 + EIP-4361 + personal_sign)
$("connect").onclick = async () => {
  const provider = await walletProvider();
  if (!provider) {
    toast("no browser wallet found — install metamask", true);
    return;
  }
  state.provider = provider;
  try {
    const accounts = await provider.request({ method: "eth_requestAccounts" });
    const account = accounts && accounts[0];
    if (!account) {
      toast("wallet returned no account — is it unlocked?", true);
      return;
    }
    const issued = await api(`/auth/wallet/nonce?address=${account}`);
    const hexMessage =
      "0x" +
      [...new TextEncoder().encode(issued.message)]
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
    const signature = await provider.request({
      method: "personal_sign",
      params: [hexMessage, account],
    });
    const session = await api("/auth/wallet/verify", {
      address: account,
      signature,
      nonce: issued.nonce,
    });
    await establish(session);
  } catch (e) {
    // 4001 is the EIP-1193 user-rejected code; anything else is a real fault
    console.error("wallet connect failed", e);
    const reason = e?.code === 4001 ? "signature request rejected" : e?.message || String(e);
    toast(reason, true);
  }
};

$("logout").onclick = async () => {
  await api("/auth/logout", {}).catch(() => {});
  localStorage.removeItem("token");
  location.reload();
};

// ------------------------------------------------------------------ board

$("composer").onsubmit = async (event) => {
  event.preventDefault();
  const text = $("text").value.trim();
  if (!text) return;
  try {
    await api("/posts", { text });
    $("text").value = "";
  } catch (e) {
    toast(e.message, true);
  }
};

function renderBoard(board) {
  const posts = $("posts");
  posts.innerHTML = "";
  for (const post of board.posts) {
    posts.append(renderPost(post));
  }
  const list = $("leaderboard");
  list.innerHTML = "";
  for (const rank of board.leaderboard) {
    const li = document.createElement("li");
    const handle = document.createElement("b");
    handle.textContent = rank.handle;
    const credits = document.createElement("span");
    credits.className = "credits";
    credits.textContent = rank.credits;
    li.append(handle, credits);
    list.append(li);
  }
}

function renderPost(post) {
  const el = document.createElement("article");
  el.className = "post";

  const meta = document.createElement("div");
  meta.className = "meta";
  const handle = document.createElement("span");
  handle.className = "handle";
  handle.textContent = post.handle;
  const when = document.createElement("span");
  when.textContent = ago(post.created_at);
  meta.append(handle, when);

  const text = document.createElement("div");
  text.className = "text";
  text.textContent = post.text;

  const actions = document.createElement("div");
  actions.className = "actions";
  for (const amount of [1, 10]) {
    const button = document.createElement("button");
    button.className = "small";
    button.textContent = `tip ${amount}`;
    button.onclick = () => tip(post.id, amount);
    actions.append(button);
  }
  const onchain = document.createElement("button");
  onchain.className = "small ghost";
  onchain.textContent = "tip usdc";
  onchain.onclick = () => tipOnchain(post.id);
  actions.append(onchain);

  const tips = document.createElement("span");
  tips.className = "tips";
  if (post.tips > 0) tips.textContent = `▲ ${post.tips}`;
  actions.append(tips);

  el.append(meta, text, actions);
  return el;
}

async function tip(postId, amount) {
  try {
    const receipt = await api("/tips", { post_id: postId, amount });
    state.profile.credits = receipt.credits;
    toast(`tipped ${amount} — ${receipt.credits} credits left`);
    enterHeaderOnly();
  } catch (e) {
    toast(e.message, true);
  }
}

function enterHeaderOnly() {
  const bold = $("who").querySelector("b");
  if (bold) bold.textContent = `${state.profile.credits} credits`;
}

// live board: one websocket, the server pushes on every change
function connectBoard() {
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  const socket = new WebSocket(`${scheme}://${location.host}/board`);
  socket.onmessage = (event) => renderBoard(JSON.parse(event.data));
  socket.onclose = () => setTimeout(connectBoard, 2000);
}

// personal activity: tips on your posts arrive as server-sent events
let feed;
function connectFeed() {
  if (feed) feed.close();
  feed = new EventSource(`/feed?token=${state.token}`);
  feed.onmessage = (event) => {
    const received = JSON.parse(event.data);
    toast(`▲ ${received.amount} credits from ${received.from} on post #${received.post_id}`);
    api("/me").then((profile) => {
      state.profile = profile;
      enterHeaderOnly();
    });
  };
}

// --------------------------------------------------------------- on-chain

function renderWallets() {
  const wallet = $("wallet");
  wallet.innerHTML = "";
  for (const w of state.profile.wallets) {
    const row = document.createElement("div");
    row.className = "row";
    const kind = document.createElement("span");
    kind.textContent = w.kind;
    const addr = document.createElement("span");
    addr.className = "addr";
    addr.textContent = `${w.address.slice(0, 6)}…${w.address.slice(-4)}`;
    addr.title = w.address;
    row.append(kind, addr);
    wallet.append(row);
  }
}

async function refreshWallet() {
  const chain = await api("/wallet");
  renderWallets();
  const wallet = $("wallet");
  for (const [label, value] of [
    ["eth (wei)", BigInt(chain.eth).toString()],
    ["usdc (µ)", BigInt(chain.usdc).toString()],
  ]) {
    const row = document.createElement("div");
    row.className = "row";
    const name = document.createElement("span");
    name.textContent = label;
    const amount = document.createElement("b");
    amount.textContent = value;
    row.append(name, amount);
    wallet.append(row);
  }
}
$("refresh-wallet").onclick = () => refreshWallet().catch((e) => toast(e.message, true));

/// real usdc: embedded wallets ask the server to sign; linked wallets
/// fetch the calldata and sign in the browser — self-custody end to end
async function tipOnchain(postId) {
  const micro = prompt("usdc amount in micro-usdc (6 decimals — 1000000 = 1 usdc):", "1000");
  if (!micro) return;
  const amount = "0x" + BigInt(micro).toString(16);
  const kind = state.profile.wallets[0]?.kind;
  try {
    if (kind === "embedded") {
      const handle = await api("/tip-onchain", { post_id: postId, amount });
      toast(`sent on-chain: ${handle.hash.slice(0, 14)}…`);
    } else {
      const provider = state.provider || (await walletProvider());
      if (!provider) {
        toast("no browser wallet found — install metamask", true);
        return;
      }
      const call = await api(`/tip-calldata?post_id=${postId}&amount=${amount}`);
      const [account] = await provider.request({ method: "eth_requestAccounts" });
      const hash = await provider.request({
        method: "eth_sendTransaction",
        params: [{ from: account, to: call.to, data: call.data }],
      });
      toast(`sent on-chain: ${hash.slice(0, 14)}…`);
    }
  } catch (e) {
    toast(e.message, true);
  }
}

// ------------------------------------------------------------------- time

function ago(unix) {
  const seconds = Math.max(1, Math.floor(Date.now() / 1000 - unix));
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}

// ------------------------------------------------------------------ start

connectBoard();
if (state.token) {
  enter();
} else {
  show("login");
}
