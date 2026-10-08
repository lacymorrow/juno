"""Metrics, results.json, results.md, charts. Run with venv-plot: venv-plot/bin/python -I analyze.py"""
import json, os, glob, collections
import numpy as np
import matplotlib; matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = os.path.dirname(os.path.abspath(__file__))
NAMES = {"A_potion": "A1 potion-8M + LR", "A_bge": "A2 bge-small + LR", "B_kev": "B Kev 0.8B zero-shot", "C_laya": "C Laya (EN) zero-shot", "D_jev": "D Jev"}
COL = {"A_potion": "#2a6fdb", "A_bge": "#8fb4ee", "B_kev": "#d9822b", "C_laya": "#2f9e6e", "D_jev": "#777777"}
THR = [0.5, 0.6, 0.7, 0.8, 0.9, 0.95]
FINE = [float(round(x, 2)) for x in np.arange(0.5, 1.0001, 0.01)]
QS = ["route", "addressed", "settings", "component", "midrun"]
SHOW = {"route": "real", "addressed": "real", "settings": "syn", "component": "syn", "midrun": "syn"}
SUB = {"real": "real set (n=169)", "syn": "synthetic"}

def load(c):
    p = os.path.join(HERE, f"raw_{c}.jsonl")
    return [json.loads(l) for l in open(p)] if os.path.exists(p) else None

def ece(conf, ok, bins=10):
    conf, ok = np.asarray(conf), np.asarray(ok, float); e = 0.0
    for b in range(bins):
        lo, hi = b / bins, (b + 1) / bins
        m = (conf >= lo) & ((conf < hi) if b < bins - 1 else (conf <= hi))
        if m.any(): e += m.mean() * abs(conf[m].mean() - ok[m].mean())
    return float(e)

def curve(recs, thr):
    out = []
    for t in thr:
        m = [r for r in recs if r["p_top"] >= t]
        out.append({"t": t, "coverage": len(m) / len(recs), "acc": (sum(r["correct"] for r in m) / len(m)) if m else None, "n": len(m)})
    return out

def metrics(recs):
    R = {}
    for q in QS:
        for s in ("real", "syn"):
            rs = [r for r in recs if r["question"] == q and r["source"] == s and r["rep"] == 0]
            if not rs: continue
            d = {"n": len(rs), "acc": float(np.mean([r["correct"] for r in rs])), "ece": ece([r["p_top"] for r in rs], [r["correct"] for r in rs]),
                 "above": curve(rs, THR), "curve": curve(rs, FINE), "lat_p50": float(np.percentile([r["ms"] for r in rs], 50)), "lat_p95": float(np.percentile([r["ms"] for r in rs], 95))}
            if q in ("settings", "component"):
                neg = [r for r in rs if r["label"] == "none"]
                d["wrong_action"] = {str(t): {"n_neg": len(neg), "rate": float(np.mean([(r["top"] != "none" and r["p_top"] >= t) for r in neg])) if neg else None} for t in (0.5, 0.7, 0.9)}
                if s == "syn":
                    nm = [r for r in rs if r["near_miss"]]
                    d["wrong_action_near_miss"] = {str(t): {"n_neg": len(nm), "rate": float(np.mean([(r["top"] != "none" and r["p_top"] >= t) for r in nm])) if nm else None} for t in (0.5, 0.7, 0.9)}
                pos = [r for r in rs if r["label"] != "none"]
                d["recall_nonnone_acc"] = float(np.mean([r["correct"] for r in pos])) if pos else None
            R[f"{q}|{s}"] = d
    # wrong-ignore: real addressed; and synthetic midrun
    def wi(rs, truth_ok, ign):
        ok = [r for r in rs if r["label"] != ign]; bad = [r for r in rs if r["label"] == ign]; res = []
        for t in FINE:
            wrong = sum(r["probs"][ign] >= t for r in ok)
            res.append({"t": t, "wrong_ignore": wrong, "wrong_ignore_rate": wrong / len(ok), "ignored_correctly": sum(r["probs"][ign] >= t for r in bad) / max(1, len(bad)),
                        "coverage": sum(r["p_top"] >= t for r in rs) / len(rs)})
        zero = [x for x in res if x["wrong_ignore"] == 0]
        return {"n_for_juno": len(ok), "n_not_for_juno": len(bad), "grid": res, "first_zero": zero[0] if zero else None}
    ar = [r for r in recs if r["question"] == "addressed" and r["source"] == "real" and r["rep"] == 0]
    if ar: R["wrong_ignore_real"] = wi(ar, None, "not_for_juno")
    mr = [r for r in recs if r["question"] == "midrun" and r["rep"] == 0]
    if mr: R["wrong_ignore_midrun_syn"] = wi(mr, None, "not_for_juno")
    allr = [r for r in recs if r["rep"] == 0]
    R["latency"] = {"p50": float(np.percentile([r["ms"] for r in allr], 50)), "p95": float(np.percentile([r["ms"] for r in allr], 95)), "n": len(allr)}
    g = collections.defaultdict(list)
    for r in recs: g[(r["uid"], r["question"])].append(r)
    full = [v for v in g.values() if len(v) == 3]
    if full:
        R["order_sensitivity"] = {"n_items": len(full), "top_changed_frac": float(np.mean([len({r["top"] for r in v}) > 1 for v in full])),
                                  "mean_ptop_range": float(np.mean([max(r["p_top"] for r in v) - min(r["p_top"] for r in v) for v in full])),
                                  "acc_by_shuffle": [float(np.mean([next(r for r in v if r["rep"] == k)["correct"] for v in full])) for k in range(3)]}
    R["ece_pooled"] = ece([r["p_top"] for r in allr], [r["correct"] for r in allr])
    R["acc_pooled"] = float(np.mean([r["correct"] for r in allr]))
    return R

def main():
    cands = {c: load(c) for c in NAMES}; cands = {c: r for c, r in cands.items() if r}
    res = {"candidates": {c: metrics(r) for c, r in cands.items()}, "names": NAMES,
           "skipped": [c for c in NAMES if c not in cands], "notes": {"real_n": 169, "sensitivity_items": 30}}
    json.dump(res, open(os.path.join(HERE, "results.json"), "w"), indent=1)
    # ---- charts
    plt.rcParams.update({"font.size": 10, "axes.spines.top": False, "axes.spines.right": False, "axes.grid": True, "grid.color": "#e6e6e6", "axes.edgecolor": "#888", "figure.facecolor": "white"})
    fig, ax = plt.subplots(1, 5, figsize=(20, 3.8))
    for a, q in zip(ax, QS):
        s = SHOW[q]
        for c in cands:
            cv = res["candidates"][c].get(f"{q}|{s}")
            if not cv: continue
            pts = [(p["coverage"], p["acc"]) for p in cv["curve"] if p["acc"] is not None]
            a.plot([p[0] for p in pts], [p[1] for p in pts], color=COL[c], lw=2, label=NAMES[c])
        a.set_title(f"{q} ({SUB[s]})", fontsize=10); a.set_xlabel("coverage (share answered)"); a.set_ylim(0, 1.02); a.set_xlim(0, 1.02)
    ax[0].set_ylabel("accuracy above threshold"); ax[0].legend(fontsize=7, loc="lower left")
    fig.tight_layout(); fig.savefig(os.path.join(HERE, "chart_accuracy_coverage.png"), dpi=160); plt.close(fig)
    # latency
    fig, a = plt.subplots(figsize=(7, 3.6)); cs = list(cands); x = np.arange(len(cs))
    p50 = [res["candidates"][c]["latency"]["p50"] for c in cs]; p95 = [res["candidates"][c]["latency"]["p95"] for c in cs]
    a.bar(x - 0.2, p50, 0.4, color=[COL[c] for c in cs], label="p50"); a.bar(x + 0.2, p95, 0.4, color=[COL[c] for c in cs], alpha=0.45, label="p95")
    for i, (u, v) in enumerate(zip(p50, p95)): a.text(i - 0.2, u * 1.1, f"{u:,.3g}".replace(",",""), ha="center", fontsize=8); a.text(i + 0.2, v * 1.1, f"{v:,.3g}".replace(",",""), ha="center", fontsize=8)
    a.set_yscale("log"); a.set_xticks(x); a.set_xticklabels([NAMES[c].replace(" + ", "\n+ ").replace(" zero-shot", "\nzero-shot") for c in cs], fontsize=8)
    a.set_ylabel("ms per decision (log scale)"); a.set_title("Latency per decision, one question, loaded M-series Mac"); a.legend(fontsize=8)
    fig.tight_layout(); fig.savefig(os.path.join(HERE, "chart_latency.png"), dpi=160); plt.close(fig)
    # calibration
    fig, a = plt.subplots(figsize=(4.8, 4.6)); a.plot([0, 1], [0, 1], color="#999", ls="--", lw=1)
    for c, recs in cands.items():
        rs = [r for r in recs if r["rep"] == 0]; conf = np.array([r["p_top"] for r in rs]); ok = np.array([r["correct"] for r in rs], float)
        bx, by = [], []
        for b in range(10):
            m = (conf >= b / 10) & (conf < (b + 1) / 10 + (1e-9 if b == 9 else 0))
            if m.sum() >= 5: bx.append(conf[m].mean()); by.append(ok[m].mean())
        a.plot(bx, by, "o-", color=COL[c], lw=1.6, ms=4, label=f"{NAMES[c]} (ECE {res['candidates'][c]['ece_pooled']:.2f})")
    a.set_xlabel("stated confidence"); a.set_ylabel("observed accuracy"); a.set_title("Calibration, all questions pooled", fontsize=10); a.legend(fontsize=7, loc="upper left")
    a.set_xlim(0, 1); a.set_ylim(0, 1); fig.tight_layout(); fig.savefig(os.path.join(HERE, "chart_calibration.png"), dpi=160); plt.close(fig)
    write_md(res)

def f(x, p=1): return "n/a" if x is None else f"{100*x:.{p}f}%"
def write_md(res):
    C = res["candidates"]; cs = list(C); L = []
    L.append("# Juno decision-model benchmark\n")
    L.append("Run date 2026-10-08, one Mac, Apple Silicon. Caveats: the machine was under heavy load from other agents (load average 50 to 90 during runs), so all latencies are inflated and noisy. The real set is small (169 requests, 5 folds grouped by session), so one item is 0.6 points and most differences under about 5 points are noise. The real set has almost no settings requests (1 of 169) and 19 component requests; midrun has no real data (synthetic only). Synthetic rows are hand-written by one author and the A heads trained on synthetic from the same author, and A sees only about 4 training examples per settings pane per fold, so synthetic scores for A are low-data numbers, not a ceiling. Fine-tuning Kev and Laya on the same folds was skipped (it would exceed the 1 hour compute budget on a loaded Mac). Jev was skipped: no key in ~/.paperclip/secrets. The JevAdapter exists in bench.py and would only ever send synthetic rows. Kev and Laya are zero-shot (not trained on any of this data). No real text appears in this file or the charts.\n")
    L.append("## Verdict\n"); 
    vp = os.path.join(HERE, "verdict.txt"); L.append(open(vp).read().strip() + "\n" if os.path.exists(vp) else "(verdict pending)\n")
    L.append("## Accuracy and ECE\n")
    L.append("| Question | Subset | n | " + " | ".join(NAMES[c] for c in cs) + " |\n|---|---|---|" + "---|" * len(cs))
    for q in QS:
        for s in ("real", "syn"):
            if f"{q}|{s}" not in C[cs[0]]: continue
            row = f"| {q} | {s} | {C[cs[0]][f'{q}|{s}']['n']} | " + " | ".join(f"{f(C[c][f'{q}|{s}']['acc'])} (ECE {C[c][f'{q}|{s}']['ece']:.2f})" for c in cs if f'{q}|{s}' in C[c]) + " |"
            L.append(row)
    L.append("\n## Accuracy above threshold (coverage in brackets)\n")
    for q in QS:
        s = SHOW[q]
        L.append(f"**{q} ({s})**\n\n| Candidate | " + " | ".join(f">={t}" for t in THR) + " |\n|---|" + "---|" * len(THR))
        for c in cs:
            d = C[c].get(f"{q}|{s}")
            if d: L.append(f"| {NAMES[c]} | " + " | ".join(f"{f(p['acc'],0)} ({f(p['coverage'],0)})" for p in d["above"]) + " |")
        L.append("")
    L.append("## Wrong-ignore (real for-Juno items predicted not_for_juno)\n")
    L.append("Rate = real for-Juno items with P(not_for_juno) >= t, as a share of for-Juno items. Zero threshold = lowest t on a 0.01 grid (0.50 to 1.00) with no wrong-ignores on real data. Coverage = share of all real items with top probability >= t. Ignore recall = share of real not_for_juno items that would be ignored at t.\n")
    L.append("| Candidate | rate @0.5 | @0.7 | @0.9 | zero threshold | coverage there | ignore recall there |\n|---|---|---|---|---|---|---|")
    for c in cs:
        w = C[c].get("wrong_ignore_real")
        if not w: continue
        g = {x["t"]: x for x in w["grid"]}; z = w["first_zero"]
        L.append(f"| {NAMES[c]} | {f(g[0.5]['wrong_ignore_rate'])} ({g[0.5]['wrong_ignore']}) | {f(g[0.7]['wrong_ignore_rate'])} ({g[0.7]['wrong_ignore']}) | {f(g[0.9]['wrong_ignore_rate'])} ({g[0.9]['wrong_ignore']}) | " +
                 (f"{z['t']:.2f} | {f(z['coverage'],0)} | {f(z['ignored_correctly'],0)} |" if z else "none | n/a | n/a |"))
    L.append(f"\nReal set: {C[cs[0]]['wrong_ignore_real']['n_for_juno']} for-Juno items (includes all mic checks and greetings), {C[cs[0]]['wrong_ignore_real']['n_not_for_juno']} not_for_juno.\n")
    L.append("Synthetic midrun, same measure (non-side-talk utterances predicted not_for_juno):\n\n| Candidate | rate @0.5 | @0.9 | zero threshold | coverage there |\n|---|---|---|---|---|")
    for c in cs:
        w = C[c].get("wrong_ignore_midrun_syn")
        if not w: continue
        g = {x["t"]: x for x in w["grid"]}; z = w["first_zero"]
        L.append(f"| {NAMES[c]} | {f(g[0.5]['wrong_ignore_rate'])} | {f(g[0.9]['wrong_ignore_rate'])} | " + (f"{z['t']:.2f} | {f(z['coverage'],0)} |" if z else "none | n/a |"))
    L.append("\n## Wrong-action (non-none answer above threshold where truth is none)\n")
    L.append("| Question | Subset | Candidate | @0.5 | @0.7 | @0.9 | n truth-none |\n|---|---|---|---|---|---|---|")
    for q in ("settings", "component"):
        for key, sub in (("wrong_action", "real (all none)"), ("wrong_action_near_miss", "synthetic near-misses")):
            for c in cs:
                s = "real" if key == "wrong_action" else "syn"; d = C[c].get(f"{q}|{s}")
                if d and key in d:
                    w = d[key]; L.append(f"| {q} | {sub} | {NAMES[c]} | {f(w['0.5']['rate'])} | {f(w['0.7']['rate'])} | {f(w['0.9']['rate'])} | {w['0.5']['n_neg']} |")
    L.append("\n## Latency (ms per decision, warm, loaded machine)\n\n| Candidate | p50 | p95 | calls |\n|---|---|---|---|")
    for c in cs: L.append(f"| {NAMES[c]} | {C[c]['latency']['p50']:.1f} | {C[c]['latency']['p95']:.1f} | {C[c]['latency']['n']} |")
    L.append("\nA1 and A2 time the full path: embed one utterance, then the head. Kev is a loopback HTTP call to a local MLX server. Laya runs in-process on MPS.\n")
    L.append("## Option-order sensitivity (30 items, 3 shuffles)\n\n| Candidate | top answer changed | mean spread of top probability | accuracy per shuffle |\n|---|---|---|---|")
    for c in cs:
        o = C[c].get("order_sensitivity")
        if o: L.append(f"| {NAMES[c]} | {f(o['top_changed_frac'],0)} | {o['mean_ptop_range']:.2f} | {', '.join(f(a,0) for a in o['acc_by_shuffle'])} |")
    L.append("\nA heads are order-invariant by construction (they read labels, not positions), so 0 is expected, not an achievement.\n")
    L.append("## Charts\n\n![accuracy vs coverage](chart_accuracy_coverage.png)\n\n![latency](chart_latency.png)\n\n![calibration](chart_calibration.png)\n")
    pf = os.path.join(HERE, "publishable.txt")
    if os.path.exists(pf): L.append(open(pf).read())
    open(os.path.join(HERE, "results.md"), "w").write("\n".join(L))

if __name__ == "__main__": main()
