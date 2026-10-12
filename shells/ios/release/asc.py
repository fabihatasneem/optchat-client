#!/usr/bin/env python3
"""Minimal App Store Connect API client (read-only queries) with the team's API key from .secrets/asc.json.
The JWT is signed with openssl (ES256); neither the token nor the key is ever printed.
  release/asc.py app <bundle id>          the app record (id, name) or "missing"
  release/asc.py builds <bundle id>       recent builds: version, processing state, uploaded date
  release/asc.py get <path>               raw GET (path like /v1/apps?limit=5)
  release/asc.py testflight-setup <bundle id> <tester email>
      idempotent: internal group "Internal" (all builds) + the tester; external group "Friends" with a public link
      (limit 100); beta app localization (description, feedback email); "What to Test" on the latest build.
      Never submits for Beta App Review."""
import base64, json, os, subprocess, sys, tempfile, time, urllib.request, urllib.error
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
cfg = json.load(open(os.path.join(ROOT, ".secrets/asc.json")))
b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
def token():
    head = b64(json.dumps({"alg": "ES256", "kid": cfg["keyId"], "typ": "JWT"}).encode())
    now = int(time.time())
    body = b64(json.dumps({"iss": cfg["issuerId"], "iat": now, "exp": now + 900, "aud": "appstoreconnect-v1"}).encode())
    msg = f"{head}.{body}".encode()
    der = subprocess.run(["openssl", "dgst", "-sha256", "-sign", cfg["keyPath"]], input=msg, capture_output=True, check=True).stdout
    # DER ECDSA (SEQUENCE{INTEGER r, INTEGER s}) -> raw r||s, 32 bytes each
    i = 2 if der[1] < 0x80 else 3
    def int_at(i):
        assert der[i] == 0x02; n = der[i + 1]; v = der[i + 2:i + 2 + n]; return v.lstrip(b"\x00").rjust(32, b"\x00"), i + 2 + n
    r, i = int_at(i); s, _ = int_at(i)
    return f"{head}.{body}.{b64(r + s)}"
def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request("https://api.appstoreconnect.apple.com" + path, data=data, method=method,
                                 headers={"Authorization": "Bearer " + token(), "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            t = r.read(); return json.loads(t) if t else {}
    except urllib.error.HTTPError as e:
        return {"error": e.code, "body": e.read().decode()[:1200]}
def get(path):
    req = urllib.request.Request("https://api.appstoreconnect.apple.com" + path, headers={"Authorization": "Bearer " + token()})
    try:
        with urllib.request.urlopen(req, timeout=30) as r: return json.load(r)
    except urllib.error.HTTPError as e:
        return {"error": e.code, "body": e.read().decode()[:800]}
cmd = sys.argv[1]
if cmd == "app":
    d = get(f"/v1/apps?filter[bundleId]={sys.argv[2]}")
    if "error" in d: print(d); sys.exit(1)
    print(json.dumps([{"id": a["id"], "name": a["attributes"]["name"], "bundleId": a["attributes"]["bundleId"], "sku": a["attributes"].get("sku")} for a in d["data"]]) if d["data"] else "missing")
elif cmd == "builds":
    a = get(f"/v1/apps?filter[bundleId]={sys.argv[2]}")
    if not a.get("data"): print("no app record"); sys.exit(1)
    d = get(f"/v1/builds?filter[app]={a['data'][0]['id']}&sort=-uploadedDate&limit=5")
    for b in d.get("data", []):
        at = b["attributes"]; print(at.get("version"), at.get("processingState"), at.get("uploadedDate"), "expired" if at.get("expired") else "")
elif cmd == "get":
    print(json.dumps(get(sys.argv[2]), indent=1)[:4000])

elif cmd == "testflight-setup":
    bundle, email = sys.argv[2], sys.argv[3]
    app = get(f"/v1/apps?filter[bundleId]={bundle}")["data"][0]["id"]
    groups = {g["attributes"]["name"]: g for g in get(f"/v1/apps/{app}/betaGroups").get("data", [])}
    def group(name, attrs):
        if name in groups: return groups[name]
        r = call("POST", "/v1/betaGroups", {"data": {"type": "betaGroups", "attributes": {"name": name, **attrs},
                 "relationships": {"app": {"data": {"type": "apps", "id": app}}}}})
        if "error" in r: print(name, "create failed:", r); sys.exit(1)
        return r["data"]
    internal = group("Internal", {"isInternalGroup": True, "hasAccessToAllBuilds": True})
    print("Internal:", internal["id"], "internal" if internal["attributes"].get("isInternalGroup") else "", "all builds" if internal["attributes"].get("hasAccessToAllBuilds") else "")
    testers = get(f"/v1/betaGroups/{internal['id']}/betaTesters").get("data", [])
    if not any(t["attributes"].get("email", "").lower() == email.lower() for t in testers):
        r = call("POST", "/v1/betaTesters", {"data": {"type": "betaTesters", "attributes": {"email": email},
                 "relationships": {"betaGroups": {"data": [{"type": "betaGroups", "id": internal["id"]}]}}}})
        print("tester", email, "added" if "error" not in r else f"not added: {r}")
    else: print("tester", email, "already in Internal")
    friends = group("Friends", {"isInternalGroup": False, "publicLinkEnabled": True, "publicLinkLimitEnabled": True, "publicLinkLimit": 100, "feedbackEnabled": True})
    fa = get(f"/v1/betaGroups/{friends['id']}")["data"]["attributes"]
    if not fa.get("publicLinkEnabled"):
        call("PATCH", f"/v1/betaGroups/{friends['id']}", {"data": {"type": "betaGroups", "id": friends["id"], "attributes": {"publicLinkEnabled": True, "publicLinkLimitEnabled": True, "publicLinkLimit": 100}}})
        fa = get(f"/v1/betaGroups/{friends['id']}")["data"]["attributes"]
    print("Friends:", friends["id"], "public link:", fa.get("publicLink"), "limit", fa.get("publicLinkLimit"))
    desc = ("OptChat is a chat over your own Claude Code and Codex agents. An engine on your computer runs the agents "
            "and keeps one endless conversation per task, with a memory that compacts itself; this app connects to that "
            "engine (over a secure tunnel) to list your tasks, read and continue their chats, and start new turns from "
            "your phone or iPad.")
    locs = get(f"/v1/apps/{app}/betaAppLocalizations").get("data", [])
    attrs = {"description": desc, "feedbackEmail": email}
    if locs:
        r = call("PATCH", f"/v1/betaAppLocalizations/{locs[0]['id']}", {"data": {"type": "betaAppLocalizations", "id": locs[0]["id"], "attributes": attrs}})
    else:
        r = call("POST", "/v1/betaAppLocalizations", {"data": {"type": "betaAppLocalizations", "attributes": {"locale": "en-US", **attrs},
                 "relationships": {"app": {"data": {"type": "apps", "id": app}}}}})
    print("beta app info:", "ok" if "error" not in r else r)
    b = get(f"/v1/builds?filter[app]={app}&sort=-uploadedDate&limit=1")["data"][0]
    notes = ("Placeholder build: connect to your engine (endpoint, workspace, client token; tunnel access key for "
             "*.tunnel.reframe.sh), open a task, send a message and check the reply arrives. The real UI comes in later builds.")
    bl = get(f"/v1/builds/{b['id']}/betaBuildLocalizations").get("data", [])
    if bl:
        r = call("PATCH", f"/v1/betaBuildLocalizations/{bl[0]['id']}", {"data": {"type": "betaBuildLocalizations", "id": bl[0]["id"], "attributes": {"whatsNew": notes}}})
    else:
        r = call("POST", "/v1/betaBuildLocalizations", {"data": {"type": "betaBuildLocalizations", "attributes": {"locale": "en-US", "whatsNew": notes},
                 "relationships": {"build": {"data": {"type": "builds", "id": b["id"]}}}}})
    print("what to test on build", b["attributes"]["version"], ":", "ok" if "error" not in r else r)
    rd = get(f"/v1/apps/{app}/betaAppReviewDetail")
    print("beta review detail (needed before the first external build):", {k: rd.get("data", {}).get("attributes", {}).get(k) for k in ("contactFirstName", "contactLastName", "contactPhone", "contactEmail", "demoAccountRequired")})
    print("privacy policy url:", (get(f"/v1/apps/{app}/appInfos").get("data") or [{}])[0].get("id") and "see App Information")
