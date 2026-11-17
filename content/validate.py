#!/usr/bin/env python3
"""Validate car-study content. Usage: python3 validate.py [--strict]
Checks: JSON parses; required fields; answer key among choices; ids unique per exam and well-formed;
lesson check ids exist; question lesson_ids appear in LESSON_PLAN.md (or a lesson file);
section matches file; counts per section. Exit 1 on errors. --strict turns warnings into errors."""
import json, glob, os, re, sys
ROOT = os.path.dirname(os.path.abspath(__file__))
EXAMS = {"sie": ("SIE", "sie"), "s7": ("S7", "s7")}
Q_REQ = ["id", "exam", "section", "topic", "lesson_id", "difficulty", "type", "stem", "choices", "answer",
         "spoken_stem", "explanation", "spoken_explanation", "tags", "outline_ref"]
F_REQ = ["id", "exam", "section", "front", "back", "spoken"]
L_REQ = ["id", "exam", "section", "section_title", "title", "order", "est_minutes", "segments", "key_terms", "mnemonics", "summary"]
errors, warnings = [], []
def err(m): errors.append(m)
def warn(m): warnings.append(m)

def load(path):
    try:
        with open(path, encoding="utf-8") as fh: return json.load(fh)
    except Exception as e:
        err(f"{path}: JSON parse error: {e}"); return None

meta = load(os.path.join(ROOT, "meta.json")) if os.path.exists(os.path.join(ROOT, "meta.json")) else None
if meta is None: warn("meta.json missing or invalid")
report = {}
for d, (EX, pfx) in EXAMS.items():
    base = os.path.join(ROOT, d)
    if not os.path.isdir(base): continue
    sec_ids = None
    if meta and EX in meta.get("exams", {}):
        sec_ids = [s["id"] for s in meta["exams"][EX]["sections"]]
    plan_ids = set()
    plan = os.path.join(base, "LESSON_PLAN.md")
    if os.path.exists(plan):
        plan_ids = set(re.findall(rf"\b({pfx}-\d+-\d+)\b", open(plan, encoding="utf-8").read()))
    lessons = {}
    for f in sorted(glob.glob(os.path.join(base, "lessons", "*.json"))):
        L = load(f)
        if L is None: continue
        for k in L_REQ:
            if k not in L: err(f"{f}: missing {k}")
        if L.get("id") and os.path.basename(f) != f"{L['id']}.json": err(f"{f}: filename != id {L.get('id')}")
        if L.get("exam") != EX: err(f"{f}: exam {L.get('exam')} != {EX}")
        if L.get("id") in lessons: err(f"{f}: duplicate lesson id {L['id']}")
        lessons[L.get("id")] = L
        words = sum(len(s.get("text", "").split()) for s in L.get("segments", []) if s.get("type") == "narration")
        if not 700 <= words <= 1600: warn(f"{f}: narration {words} words (target 750-1500)")
        for s in L.get("segments", []):
            if s.get("type") not in ("narration", "check"): err(f"{f}: bad segment type {s.get('type')}")
    qids, qsec = {}, {}
    for f in sorted(glob.glob(os.path.join(base, "questions", "*.json"))):
        arr = load(f)
        if arr is None: continue
        if not isinstance(arr, list): err(f"{f}: not an array"); continue
        fsec = os.path.splitext(os.path.basename(f))[0]
        if sec_ids and fsec not in sec_ids: err(f"{f}: file section {fsec} not in meta sections")
        for q in arr:
            qid = q.get("id", "?")
            for k in Q_REQ:
                if k not in q or q[k] in ("", None, []): err(f"{f}:{qid}: missing/empty {k}")
            if not re.fullmatch(rf"{pfx}-q-\d{{4}}", str(qid)): err(f"{f}:{qid}: bad id format")
            if qid in qids: err(f"{f}:{qid}: duplicate id (also in {qids[qid]})")
            qids[qid] = f
            if q.get("exam") != EX: err(f"{f}:{qid}: exam {q.get('exam')}")
            if q.get("section") != fsec: err(f"{f}:{qid}: section {q.get('section')} != file {fsec}")
            if q.get("type") not in ("mc", "tf"): err(f"{f}:{qid}: bad type")
            if q.get("difficulty") not in (1, 2, 3): err(f"{f}:{qid}: bad difficulty")
            keys = [c.get("key") for c in q.get("choices", [])]
            if len(keys) != len(set(keys)): err(f"{f}:{qid}: duplicate choice keys")
            if q.get("answer") not in keys: err(f"{f}:{qid}: answer {q.get('answer')} not among {keys}")
            if q.get("type") == "mc" and len(keys) != 4: warn(f"{f}:{qid}: mc with {len(keys)} choices")
            if q.get("type") == "tf" and len(keys) != 2: err(f"{f}:{qid}: tf must have 2 choices")
            if not isinstance(q.get("tags", []), list): err(f"{f}:{qid}: tags not list")
            lid = q.get("lesson_id")
            if lid and lid not in lessons and lid not in plan_ids: err(f"{f}:{qid}: lesson_id {lid} not in lessons or LESSON_PLAN.md")
            sw = len(str(q.get("spoken_explanation", "")).split())
            if sw and not 40 <= sw <= 130: warn(f"{f}:{qid}: spoken_explanation {sw} words (target ~20-40 s)")
            if re.search(r"[|<>{}\[\]]", str(q.get("spoken_stem", ""))): warn(f"{f}:{qid}: spoken_stem has table/markup symbols")
            if ("EXCEPT" in q.get("stem", "") or " NOT " in q.get("stem", "")) and not re.search(r"EXCEPT|NOT", q.get("spoken_stem", "")):
                warn(f"{f}:{qid}: EXCEPT/NOT not emphasized in spoken_stem")
            qsec[fsec] = qsec.get(fsec, 0) + 1
    for lid, L in lessons.items():
        for s in L.get("segments", []):
            if s.get("type") == "check" and s.get("question_id") not in qids:
                err(f"lesson {lid}: check question_id {s.get('question_id')} not found")
    fids, fsec_c = set(), {}
    for f in sorted(glob.glob(os.path.join(base, "flashcards", "*.json"))):
        arr = load(f)
        if arr is None: continue
        fsec = os.path.splitext(os.path.basename(f))[0]
        for c in arr:
            cid = c.get("id", "?")
            for k in F_REQ:
                if not c.get(k): err(f"{f}:{cid}: missing/empty {k}")
            if not re.fullmatch(rf"{pfx}-f-\d{{4}}", str(cid)): err(f"{f}:{cid}: bad id format")
            if cid in fids: err(f"{f}:{cid}: duplicate flashcard id")
            fids.add(cid)
            if c.get("section") != fsec: err(f"{f}:{cid}: section mismatch")
            fsec_c[fsec] = fsec_c.get(fsec, 0) + 1
    lsec = {}
    for L in lessons.values(): lsec[L.get("section")] = lsec.get(L.get("section"), 0) + 1
    report[EX] = dict(questions=qsec, flashcards=fsec_c, lessons=lsec,
                      totals=dict(questions=len(qids), flashcards=len(fids), lessons=len(lessons)))

for EX, r in report.items():
    print(f"== {EX}: {r['totals']}")
    for sec in sorted(set(r["questions"]) | set(r["flashcards"]) | set(r["lessons"])):
        print(f"   {sec}: questions={r['questions'].get(sec,0)} flashcards={r['flashcards'].get(sec,0)} lessons={r['lessons'].get(sec,0)}")
for w in warnings: print("WARN ", w)
for e in errors: print("ERROR", e)
strict = "--strict" in sys.argv
print(f"RESULT: {len(errors)} errors, {len(warnings)} warnings")
sys.exit(1 if errors or (strict and warnings) else 0)
