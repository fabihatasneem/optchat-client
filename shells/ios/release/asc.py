#!/usr/bin/env python3
"""Minimal App Store Connect API client (read-only queries) with the team's API key from .secrets/asc.json.
The JWT is signed with openssl (ES256); neither the token nor the key is ever printed.
  release/asc.py app <bundle id>          the app record (id, name) or "missing"
  release/asc.py builds <bundle id>       recent builds: version, processing state, uploaded date
  release/asc.py get <path>               raw GET (path like /v1/apps?limit=5)"""
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
