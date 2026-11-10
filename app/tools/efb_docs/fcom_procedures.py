"""usage: fcom_procedures.py <FCOM text> <index.html> <out json>

Fills the ECAM procedures FlyByWire left without steps (TODO stubs) from a
local FCOM text, for this machine only: the output is a separate file the EFB
loads when present, kept out of every release (package_release.py)."""

import itertools
import json
import re
import sys
from pathlib import Path

BODY_FROM = "PRO-ABN-ECAM-20 Not-Sensed Procedures"
TOC_FROM, TOC_TO = "PRO-ABN-ECAM-10-21-10 21-Air Conditioning", "PRO-ABN-NECA Non-ECAM Abnormal"
NOISE = re.compile(
    r"FLEET|^FCOM\b|^A380\b|PROCEDURES$|^FLIGHT CREW|^OPERATING MANUAL|ECAM ABNORMAL AND EMERGENCY|PROCEDURES - ALL ECAM|"
    r"^Applicable to|Continued on the following|\(Cont'd\)|^(?:[A-Z]{1,2}\s+)?\d{2} [A-Z]{3} \d{2}$|^[PUAL]{1,2}$|^PRO-[A-Z]")
CONDITION = re.compile(r"^(IF|WHEN|FOR|BEFORE|AFTER|AT|DURING|IN CASE OF|IN ALL OTHER CASES|AS SOON AS)\b.*:$")
LEADER = re.compile(r"^(.*?)\s*\.{3,}\s*(.*)$")
GROUP = re.compile(r"([^\s()]+)((?:\([^\s()]+\))+)")
SPACED = re.compile(r"\b([A-Z0-9+]+) \(([A-Z0-9+]+)\)")
ORS = re.compile(r"\b([A-Z0-9+]+) or \(?([A-Z0-9+]+)\)?")
WIP_TO_FCOM = {
    "FUEL FUEL LEAK (WIP)": "FUEL LEAK DETECTED",
    "FUEL JETTISON (WIP)": "FUEL JETTISON",
    "FUEL MAN BALANCING PROCEDURE (WIP)": "FUEL MAN BALANCING PROCEDURE",
}
STATUS_KEYS = {("LIMITATIONS", "ALL PHASES"): "limitationsAllPhases", ("LIMITATIONS", "APPR & LDG"): "limitationsApprLdg",
               ("LIMITATIONS", "PFD"): "limitationsPfd", ("INOP SYS", "ALL PHASES"): "inopSysAllPhases",
               ("INOP SYS", "APPR & LDG"): "inopSysApprLdg", ("INFO", None): "info", ("REDUND LOSS", None): "redundLoss"}


CONTINUES = re.compile(r"(,| AND| OR| ARE| IS| NOT| THAN| THE| TO| OF| ABOVE| BELOW| WITH| IN| ON| AT| FOR| BY)$")
POUNDS = re.compile(r"\b(\d{1,3}(?: \d{3})+|\d+) ?LBS?\b")


def kilograms(text):
    return POUNDS.sub(lambda m: f"{m.group(1)} LB ({round(int(m.group(1).replace(' ', '')) * 0.45359237 / 10) * 10:,} KG)".replace(",", " "), text)


def tidy(text):
    text = re.sub(r"([A-Z0-9]) s(?=[\s:.,)]|$)", r"\1s", text)
    text = SPACED.sub(r"\1(\2)", text)
    return re.sub(r"\s+", " ", text).strip()


def groups(title):
    out = []
    for m in GROUP.finditer(title):
        alts = [m.group(1)] + re.findall(r"\(([^()]+)\)", m.group(2))
        out.append((m.group(0), alts))
    for m in ORS.finditer(title):
        out.append((m.group(0), [m.group(1), m.group(2)]))
    return out


def expansions(title):
    gs = groups(title)
    if not gs:
        return [(title, {})]
    found = []
    for pick in itertools.islice(itertools.product(*[range(len(a)) for _, a in gs]), 256):
        t = title
        for (raw, alts), k in zip(gs, pick):
            t = t.replace(raw, alts[k], 1)
        found.append((re.sub(r"\s+", " ", t), {raw: (k, len(alts)) for (raw, _), k in zip(gs, pick)}))
    return found


def substitute(text, choice):
    def one(m):
        raw = m.group(0)
        alts = [m.group(1)] + re.findall(r"\(([^()]+)\)", m.group(2))
        if raw in choice:
            return alts[choice[raw][0]]
        same = [k for k, n in choice.values() if n == len(alts)]
        return alts[same[0]] if len(set(same)) == 1 and same else raw
    return GROUP.sub(one, text)


def clean_lines(raw):
    out = []
    for n, line in enumerate(raw):
        s = line.replace("\x0c", "").strip()
        out.append("" if not s or NOISE.search(s) else s)
    return out


def headings(raw, lines, body_start):
    toc = set()
    t0 = next(i for i, l in enumerate(raw) if l.startswith(TOC_FROM))
    t1 = next(i for i, l in enumerate(raw) if l.startswith(TOC_TO))
    for l in raw[t0:t1]:
        m = re.match(r"^\s*([A-Z0-9].*?)\s*\.{5,}", l)
        if m:
            toc.add(tidy(m.group(1)))
    contd = {tidy(m.group(1)) for l in raw[body_start:] for m in [re.match(r"^\s*(.+?) \(Cont'd\)\s*$", l)] if m}
    starts = {}
    for i in range(body_start, len(raw)):
        s = tidy(raw[i])
        if not s or "...." in s or re.search(r"[a-z]{3}", s) or len(s) > 90:
            continue
        nxt = next((x for x in lines[i + 1:i + 8] if x), "")
        annunciated = nxt.startswith(("ANNUNCIATIONS", "Triggering Conditions"))
        if (annunciated or s in contd or s in toc) and s not in starts:
            starts[s] = i
    return starts


STATUS_HEADS = ("LIMITATIONS", "INOP SYS", "INFO", "REDUND LOSS", "MORE INFO", "DEFERRED PROC")
SHORT_CONDITION = re.compile(r"^(On ground|In flight|In all other cases|In case of|If|When|For|After|Before|During|At)\b[^.]{0,50}:$")


def segments(raw_line):
    return [(m.start(), m.group(0)) for m in re.finditer(r"\S+(?: \S+)*", raw_line)]


def status_line(raw_line, choice, state, status):
    for col, seg in segments(raw_line):
        text = tidy(substitute(seg.lstrip("·• "), choice)).upper()
        if not text or NOISE.search(seg.strip()) or text == "STATUS":
            continue
        if text in ("LIMITATIONS ON ECAM", "LIMITATIONS ON PFD", "ANNUNCIATIONS") or ". " in text or "..." in text:
            continue
        if text in STATUS_HEADS:
            state["section"] = None if text in ("MORE INFO", "DEFERRED PROC") else text
            state["phase"], state["split"] = "ALL PHASES", None
            continue
        if text in ("ALL PHASES", "PFD"):
            state["phase"] = text
            continue
        if text == "APPR & LDG":
            state["split"] = col - 8
            continue
        section = state["section"]
        if section is None or re.search(r"[a-z]{3}", seg):
            continue
        phase = "APPR & LDG" if state["split"] is not None and col >= state["split"] else state["phase"]
        key = STATUS_KEYS.get((section, phase if section in ("LIMITATIONS", "INOP SYS") else None))
        if key:
            listed = status.setdefault(key, [])
            if listed and CONTINUES.search(listed[-1]):
                listed[-1] = listed[-1] + " " + text
            else:
                listed.append(text)


def parse(lines, raws, choice):
    items, status = [], {}
    state = {"section": None, "phase": "ALL PHASES", "split": None}
    mode, in_cond, note, l1 = "proc", False, False, False
    started = not any(l.startswith("Flight Phase Inhibition") for l in lines)
    for raw, raw_line in zip(lines, raws):
        if not raw:
            continue
        if not started:
            started = raw.startswith("Flight Phase Inhibition")
            continue
        if mode == "proc" and (any(seg == "STATUS" for _, seg in segments(raw_line)) or raw.strip() in STATUS_HEADS):
            mode = "status"
        if mode == "status":
            if not raw.startswith(("L2", "L3")):
                status_line(raw_line, choice, state, status)
            continue
        if raw.startswith(("L2", "L3")):
            note, l1 = True, False
            continue
        if raw.startswith("L1"):
            note, l1 = False, True
            text = raw[2:].strip()
        else:
            text = raw
            if SHORT_CONDITION.match(text):
                note, l1 = False, True
            elif re.search(r"[a-z]{2}", text) and (note or not l1):
                continue
            note = False
        if text.startswith(("- ", "Note")) and re.search(r"[a-z]", text):
            continue
        if not LEADER.match(text) and not SHORT_CONDITION.match(text.strip()) and len(re.findall(r"\b[a-z]{2,}\b", text)) >= 3:
            continue
        if re.match(r"^(CAUTION|WARNING)\b", text.strip()):
            continue
        short_condition = bool(SHORT_CONDITION.match(text.lstrip("·• ").strip()))
        text = tidy(substitute(text.lstrip("·• ").strip(), choice)).upper()
        if not text or re.search(r"REFER TO|\bMEL\b|^ANNUNCIATIONS$|^TRIGGERING CONDITIONS|^INDICATIONS:", text):
            continue
        if text.endswith(".") and not text.endswith("..."):
            continue
        if items and items[-1][3] == "c" and text.endswith(":") and not short_condition and not CONDITION.match(text) and not re.match(r"^(IF|WHEN) ", text):
            items[-1][0] = items[-1][0] + " " + text.rstrip(":").strip()
            continue
        if items and items[-1][3] == "b" and items[-1][0].endswith(text):
            continue
        if items and items[-1][3] in ("c", "b") and CONTINUES.search(items[-1][0]) and not LEADER.match(text) and " : " not in text and not re.match(r"^(IF|WHEN|FOR|BEFORE|AFTER) ", text):
            items[-1][0] = items[-1][0] + " " + text.rstrip(":").strip()
            continue
        level = 1 if in_cond else 0
        m = LEADER.match(text)
        if short_condition or CONDITION.match(text) or (re.match(r"^(IF|WHEN) ", text) and not m and " : " not in text):
            items.append([text.rstrip(":").strip(), "", 0, "c"])
            in_cond = True
            continue
        if m and not m.group(1):
            if items and items[-1][3] == "" and not items[-1][1]:
                items[-1][1] = m.group(2).strip()
            continue
        if m:
            items.append([m.group(1).strip(), m.group(2).strip(), level, ""])
            continue
        if text == "ASSOCIATED PROCEDURES":
            items.append([text, "", level, "h"])
            continue
        if " : " in text:
            name, value = text.split(" : ", 1)
            items.append([name.strip(), value.strip(), level, ""])
            continue
        if text.startswith("(") and items:
            k = 1 if items[-1][1] else 0
            items[-1][k] = (items[-1][k] + " " + text).strip()
            continue
        flags = "r" if text.startswith("LAND ASAP") else "g" if text.startswith("LAND ANSA") else "b"
        items.append([text, "", level, flags])
    for k, it in enumerate(items):
        if it[3] == "c" and (k + 1 == len(items) or items[k + 1][3] == "c"):
            it[3] = "h"
        it[0], it[1] = kilograms(it[0]), kilograms(it[1])
    status = {key: [kilograms(v) for v in vals] for key, vals in status.items()}
    return items, status


def main(fcom, ui, out):
    raw = Path(fcom).read_text(encoding="cp1252", errors="replace").replace("\r", "").split("\n")
    html = Path(ui).read_text(encoding="utf-8")
    docs = json.loads(re.search(r"window\.__DOCS__=(\{.*?\});?</script>", html, re.S).group(1))
    body_start = next(i for i, l in enumerate(raw) if l.startswith(BODY_FROM))
    lines = clean_lines(raw)
    starts = headings(raw, lines, body_start)
    order = sorted(starts.values())
    ends = {s: next((o for o in order if o > s), len(raw)) for s in order}
    lookup = {}
    for title, at in starts.items():
        for name, choice in expansions(re.sub(r"\s*\((RED|AMBER) ALERT\)$", "", title)):
            lookup.setdefault(name.upper(), (at, choice))
    result, missing = {}, []
    for p in docs.get("sensed", []) + docs.get("nonSensed", []):
        if p.get("i"):
            continue
        want = WIP_TO_FCOM.get(p["t"], p["t"].replace(" (WIP)", ""))
        hit = lookup.get(tidy(want).upper())
        if not hit:
            missing.append(p["t"])
            continue
        at, choice = hit
        items, status = parse(lines[at + 1:ends[at]], raw[at + 1:ends[at]], choice)
        if not items and (not status or p.get("x")):
            aware = any(l.lower().startswith("crew awareness") for l in lines[at + 1:ends[at]])
            missing.append(p["t"] + (" [crew awareness]" if aware else " [no steps in FCOM]"))
            continue
        entry = {"i": items}
        if status and not p.get("x"):
            entry["x"] = status
        if p["t"].endswith(" (WIP)"):
            entry["t"] = want
        result[p["id"]] = entry
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    Path(out).write_text(json.dumps(result, separators=(",", ":")), encoding="utf-8")
    print(f"{len(result)} procedures filled, {len(missing)} left")
    for t in missing:
        print("  -", t)


if __name__ == "__main__":
    main(*sys.argv[1:])
