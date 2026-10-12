"""Juno decision-model benchmark harness.
Adapter contract: decide(state, question) -> {option: prob}.
  state    = {"text": str, "running_task": str (midrun only)}
  question = {"name", "instructions", "options": [(option, description), ...]}  (order already shuffled)
Optional adapter hooks: set_fold(k, train_items), warmup().
Usage: python -I bench.py CAND   (CAND in A_potion A_bge B_kev C_laya D_jev)
Private data stays local: real text is never written to any output file (records carry ids only).
"""
import json, os, sys, time, random, hashlib, math, urllib.request, collections
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
SCR = os.path.dirname(HERE)
RES = os.path.join(SCR, "decision-research")
SEED = 20261008
N_FOLDS = 5

# ---------------------------------------------------------------- questions
def _o(*pairs): return list(pairs)
QUESTIONS = {
 "route": {"instructions": "How should the voice assistant handle this request?", "options": _o(
   ("chat", "talk or answer from knowledge, no tools"),
   ("tools", "use an app, script, music, timer, calendar or web lookup"),
   ("computer_use", "click, type or operate an app window on screen"),
   ("escalate", "start a coding job or long multi-step build"))},
 "addressed": {"instructions": "Is the speaker talking to the assistant? Mic checks, greetings and 'can you hear me' are for the assistant.", "options": _o(
   ("for_juno", "request, question, greeting or mic check for the assistant"),
   ("not_for_juno", "side talk to another person, noise, or a fragment with no request"))},
 "settings": {"instructions": "Which settings pane does the person ask to open or change? Questions about settings or unrelated talk are none.", "options": _o(
   ("juno_general", "Juno general, launch at login"), ("juno_voice", "Juno speaking voice"), ("juno_triggers", "Juno hotkeys, push to talk"),
   ("juno_ai_provider", "AI provider, API key"), ("juno_models", "which AI or speech model"), ("juno_tools", "which tools Juno may use"),
   ("juno_automations", "scheduled automations"), ("juno_notifications", "Juno alerts"), ("juno_permissions", "what Juno may do"),
   ("juno_advanced", "developer and reset options"),
   ("mac_wifi", "Mac Wi-Fi network"), ("mac_bluetooth", "Mac Bluetooth devices"), ("mac_sound", "Mac sound input output"),
   ("mac_displays", "Mac display, monitor"), ("mac_keyboard", "Mac keyboard shortcuts"), ("mac_notifications", "Mac app notifications"),
   ("mac_privacy_security", "Mac privacy, security, app access"), ("mac_battery", "Mac battery, energy"), ("mac_appearance", "Mac dark mode, accent color"),
   ("mac_software_update", "macOS updates"), ("mac_accessibility", "Mac accessibility"), ("mac_desktop_dock", "Mac desktop and Dock"),
   ("mac_focus", "Mac Focus, do not disturb"), ("none", "no settings pane requested"))},
 "component": {"instructions": "Which answer card should the assistant show for this request, if any?", "options": _o(
   ("now_playing", "current song"), ("timer", "timer or countdown"), ("weather", "weather forecast"), ("calendar_day", "calendar agenda"),
   ("reminders", "reminders list"), ("calculation", "math result"), ("app_opened", "open an app"), ("web_answer", "web fact or score"),
   ("directions", "route or travel time"), ("system_status", "Mac battery, storage, wifi status"), ("none", "no card needed"))},
 "midrun": {"instructions": "The assistant is running a task. What is the person's new utterance?", "options": _o(
   ("stop", "cancel or stop the running task"), ("add_to_task", "extra detail that changes the running task"),
   ("new_request", "a separate new request"), ("not_for_juno", "side talk to someone else"))},
}
for q in QUESTIONS.values(): q["names"] = [o for o, _ in q["options"]]

# ---------------------------------------------------------------- data
def load_items():
    raw = {}
    for l in open(os.path.join(HERE, "raw.jsonl")):
        d = json.loads(l); raw[d["id"]] = d["text"]
    items = []
    for l in open(os.path.join(HERE, "labels.jsonl")):
        d = json.loads(l)
        items.append({"uid": d["id"], "source": "real", "group": d["id"].rsplit("-", 1)[0], "text": raw[d["id"]],
          "labels": {"route": d["route"], "settings": d["settings"].replace("mac:notifications", "mac_notifications"),
                     "component": d["component"], "addressed": "not_for_juno" if d["not_for_juno"] else "for_juno"},
          "eval_q": ["route", "addressed", "settings", "component"], "near_miss": False, "confidence": d["confidence"]})
    for l in open(os.path.join(HERE, "synthetic.jsonl")):
        d = json.loads(l); lab = {d["question"]: d["label"]}
        for k in ("addressed", "settings", "component"):
            if k in d: lab[k] = d[k]
        items.append({"uid": d["id"], "source": "syn", "group": d["id"], "text": d["text"], "running_task": d.get("running_task"),
                      "labels": lab, "eval_q": [d["question"]], "near_miss": d.get("near_miss", False), "confidence": "high"})
    # folds: real by session group (no session split across folds), synthetic stratified
    rng = random.Random(SEED)
    groups = sorted({i["group"] for i in items if i["source"] == "real"})
    rng.shuffle(groups); gf = {g: n % N_FOLDS for n, g in enumerate(groups)}
    cnt = collections.defaultdict(int)
    syn = [i for i in items if i["source"] == "syn"]; rng.shuffle(syn)
    syn.sort(key=lambda i: (i["eval_q"][0], i["labels"][i["eval_q"][0]]))
    for n, i in enumerate(syn):
        key = (i["eval_q"][0], i["labels"][i["eval_q"][0]]); i["fold"] = (cnt[key] + hash_int(key)) % N_FOLDS; cnt[key] += 1
    for i in items:
        if i["source"] == "real": i["fold"] = gf[i["group"]]
    return items

def hash_int(x): return int(hashlib.md5(repr(x).encode()).hexdigest(), 16) % N_FOLDS

def shuffled_options(uid, q, rep):
    opts = list(QUESTIONS[q]["options"])
    random.Random(f"{SEED}|{uid}|{q}|{rep}").shuffle(opts)
    return opts

def sensitivity_set(items):
    pick = lambda src, q, n: sorted([i for i in items if i["source"] == src and q in i["eval_q"]], key=lambda i: hashlib.md5(i["uid"].encode()).hexdigest())[:n]
    return pick("real", "route", 10) + pick("syn", "settings", 10) + pick("syn", "midrun", 10)

# ---------------------------------------------------------------- adapters
def softmax(z):
    z = np.asarray(z, float); z = z - z.max(); e = np.exp(z); return e / e.sum()

def state_of(it): 
    s = {"text": it["text"]}
    if it.get("running_task"): s["running_task"] = it["running_task"]
    return s

class AdapterA:
    """Static or small-transformer embedding + one LR head per question, nested temperature calibration."""
    def __init__(self, kind):
        self.kind = kind; self.heads = {}
        if kind == "potion":
            from model2vec import StaticModel
            self.m = StaticModel.from_pretrained("minishlab/potion-base-8M")
            self.emb = lambda xs: np.asarray(self.m.encode(list(xs)), dtype=np.float32)
        else:
            from sentence_transformers import SentenceTransformer
            self.m = SentenceTransformer("BAAI/bge-small-en-v1.5", device="cpu")
            self.emb = lambda xs: self.m.encode(list(xs), normalize_embeddings=False, show_progress_bar=False)
        self.cache = {}
    def _e(self, texts):
        need = [t for t in dict.fromkeys(texts) if t not in self.cache]
        if need:
            E = self.emb(need); E = E / (np.linalg.norm(E, axis=1, keepdims=True) + 1e-9)
            for t, e in zip(need, E): self.cache[t] = e
        return np.stack([self.cache[t] for t in texts])
    def _fresh(self, texts):
        E = self.emb(list(texts)); return E / (np.linalg.norm(E, axis=1, keepdims=True) + 1e-9)
    def feats(self, texts, tasks, q, fresh=False):
        enc = self._fresh if fresh else self._e
        E = enc(texts)
        if q != "midrun": return E
        K = enc([t or "" for t in tasks])
        return np.hstack([E, E * K, (E * K).sum(1, keepdims=True)])
    def warmup(self): self._e(["warm up"])
    def set_fold(self, k, items):
        from sklearn.linear_model import LogisticRegression
        from sklearn.model_selection import KFold
        from scipy.optimize import minimize_scalar
        self.heads = {}; self.temps = {}
        for q, spec in QUESTIONS.items():
            tr = [i for i in items if i["fold"] != k and q in i["labels"]]
            if not tr: continue
            X = self.feats([i["text"] for i in tr], [i.get("running_task") for i in tr], q); y = np.array([i["labels"][q] for i in tr])
            names = spec["names"]
            def fit(Xa, ya):
                c = LogisticRegression(C=10, max_iter=3000); c.fit(Xa, ya); return c
            def logp(c, Xb, names=names):
                out = np.full((len(Xb), len(names)), -12.0); lp = c.predict_log_proba(Xb)
                for j, cl in enumerate(c.classes_): out[:, names.index(cl)] = lp[:, j]
                return out
            oof = np.zeros((len(y), len(names)))
            for a, b in KFold(4, shuffle=True, random_state=0).split(X):
                oof[b] = logp(fit(X[a], y[a]), X[b])
            yi = np.array([names.index(v) for v in y])
            def nll(lt):
                z = oof / math.exp(lt); z = z - z.max(1, keepdims=True); lg = z - np.log(np.exp(z).sum(1, keepdims=True))
                return -lg[np.arange(len(yi)), yi].mean()
            T = math.exp(minimize_scalar(nll, bounds=(math.log(0.2), math.log(8)), method="bounded").x)
            c = fit(X, y); self.heads[q] = (c, logp, T)
    def decide(self, state, question):
        q = question["name"]; c, logp, T = self.heads[q]
        x = self.feats([state["text"]], [state.get("running_task")], q, fresh=True)
        p = softmax(logp(c, x)[0] / T); p = np.maximum(p, 1e-6); p /= p.sum()
        names = QUESTIONS[q]["names"]
        return {o: float(p[names.index(o)]) for o, _ in question["options"]}

def _crit(question): return {o: d for o, d in question["options"]}
def _zs_state(state):
    if "running_task" in state: return {"running_task": state["running_task"], "new_utterance": state["text"]}
    return {"utterance": state["text"]}
def _zs_instr(question, state):
    return question["instructions"]

class KevAdapter:
    def __init__(self, url="http://127.0.0.1:8009/v1/systemone", model="kev-latest"): self.url, self.model = url, model
    def warmup(self):
        for _ in range(3): self.decide({"text": "hello there"}, {"name": "route", **QUESTIONS["route"], "options": QUESTIONS["route"]["options"]})
    def decide(self, state, question):
        body = {"state": _zs_state(state), "model": self.model, "questions": {"q": {"type": "choice", "instructions": _zs_instr(question, state), "criteria": _crit(question)}}}
        r = urllib.request.Request(self.url, json.dumps(body).encode(), {"content-type": "application/json"})
        o = json.load(urllib.request.urlopen(r, timeout=120))
        return o["answers"]["q"]["probabilities"]

class LayaAdapter:
    def __init__(self, device="mps"):
        import laya
        self.a = laya.load("convaiinnovations/laya", device=device)
    def warmup(self):
        for _ in range(3): self.decide({"text": "hello there"}, {"name": "route", **QUESTIONS["route"]})
    def decide(self, state, question):
        r = self.a.predict(_zs_state(state), {"q": {"type": "choice", "instructions": _zs_instr(question, state), "criteria": _crit(question)}})
        return r["answers"]["q"]["probabilities"]

class JevAdapter:
    """POST /v1/systemone (jev.md). Key from ~/.paperclip/secrets/{typesafe*,openrouter*}. SYNTHETIC rows only: never sends real text."""
    synthetic_only = True
    def __init__(self):
        d = os.path.expanduser("~/.paperclip/secrets"); self.key = None
        for pat, url, model in (("typesafe", "https://api.typesafe.ai/v1/systemone", "jev-1.13.0"), ("openrouter", "https://openrouter.ai/api/v1/systemone", "typesafe/jev-1.13")):
            for f in sorted(os.listdir(d)) if os.path.isdir(d) else []:
                if f.lower().startswith(pat):
                    k = open(os.path.join(d, f)).read().strip().split("=")[-1].strip().strip('"')
                    if k: self.key, self.url, self.model = k, url, model; return
        raise RuntimeError("no Jev key")
    def warmup(self): pass
    def decide(self, state, question):
        body = {"state": _zs_state(state), "model": self.model, "questions": {"q": {"type": "choice", "instructions": _zs_instr(question, state), "criteria": _crit(question)}}}
        r = urllib.request.Request(self.url, json.dumps(body).encode(), {"content-type": "application/json", "Authorization": "Bearer " + self.key})
        return json.load(urllib.request.urlopen(r, timeout=30))["answers"]["q"]["probabilities"]

# ---------------------------------------------------------------- runner
def run(adapter, items, out_path, with_sensitivity=True, only_synthetic=False):
    sens = {i["uid"] for i in sensitivity_set(items)}
    recs = []
    if hasattr(adapter, "warmup"): adapter.warmup()
    folds = range(N_FOLDS) if hasattr(adapter, "set_fold") else [None]
    f = open(out_path, "w")
    t0 = time.time(); n = 0
    for k in folds:
        if k is not None: adapter.set_fold(k, items)
        for it in items:
            if k is not None and it["fold"] != k: continue
            if only_synthetic and it["source"] != "syn": continue
            for q in it["eval_q"]:
                reps = [0] + ([1, 2] if (with_sensitivity and it["uid"] in sens and (q in ("route", "settings", "midrun")) and not (it["source"] == "real" and q != "route")) else [])
                for rep in reps:
                    opts = shuffled_options(it["uid"], q, rep)
                    question = {"name": q, "instructions": QUESTIONS[q]["instructions"], "options": opts}
                    t = time.perf_counter(); probs = adapter.decide(state_of(it), question); ms = (time.perf_counter() - t) * 1000
                    s = sum(probs.values()) or 1.0; probs = {o: float(probs.get(o, 0.0)) / s for o, _ in opts}
                    top = max(probs, key=probs.get)
                    rec = {"uid": it["uid"], "source": it["source"], "question": q, "label": it["labels"][q], "near_miss": it["near_miss"], "rep": rep,
                           "order": [o for o, _ in opts], "probs": probs, "top": top, "p_top": probs[top], "ms": ms, "correct": top == it["labels"][q], "fold": it["fold"],
                           "conf": it["confidence"]}
                    f.write(json.dumps(rec) + "\n"); f.flush(); n += 1
                    if n % 100 == 0: print(f"{n} calls {time.time()-t0:.0f}s", flush=True)
    f.close()

if __name__ == "__main__":
    cand = sys.argv[1]; items = load_items()
    out = os.path.join(HERE, f"raw_{cand}.jsonl")
    if cand == "A_potion": ad = AdapterA("potion")
    elif cand == "A_bge": ad = AdapterA("bge")
    elif cand == "B_kev": ad = KevAdapter()
    elif cand == "C_laya": ad = LayaAdapter()
    elif cand == "D_jev":
        try: ad = JevAdapter()
        except Exception as e: print("Jev skipped:", e); sys.exit(0)
    else: sys.exit("unknown")
    run(ad, items, out, only_synthetic=getattr(ad, "synthetic_only", False))
    print("done", cand)
