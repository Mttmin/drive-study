
## S7 wave 1 (2026-11-16, S7 content worker)
Landed in content/s7/:
- questions/: 111 total. S7-1 7, S7-2 11, S7-3 74, S7-4 19 (ids s7-q-0001..0111). S7-3 is mostly options (38: basics, hedges, spreads, straddles, index, OCC/adjustments, tax), plus munis (13), debt/governments (8), equity and tax (5), packaged products (5), and analysis/records (5). S7-4 is margin (14: Reg T, $2,000 minimum, long/short equity, excess equity, SMA, buying power, maintenance call prices, restricted accounts), plus orders/settlement/Reg SHO/arbitration (5).
- flashcards/: 70 (S7-1 5, S7-2 7, S7-3 44, S7-4 14), ids s7-f-0001..0070.
- lessons/: s7-3-01 Options foundations (~1,000 words), s7-3-02 Hedging and income with options (~800), s7-4-05 Margin I: Reg T and long accounts (~940). Each has 4-5 check segments.
- LESSON_PLAN.md: 38 lessons mapped to every FINRA outline subsection. Later waves follow it.
- Sections and weights follow FINRA's Series 7 outline (2025-10 PDF): F1 7% (9 items), F2 9% (11), F3 73% (91), F4 11% (14). In FINRA's outline, margin is 4.4, so margin items sit in S7-4, and options are 3.2 (S7-3).
- Validation: S7 checker (/workspace/s7build/validate_s7.py). All JSON parses, required fields are present, each answer key is among the choices, ids are unique and contiguous, lesson_ids are in the plan, check ids exist, and spoken fields contain no symbols. All OK. The source scripts that regenerate the JSON are in /workspace/s7build/.
- Items tagged changes-yearly, with the year stated: 2026 IRA limit $7,500 (+$1,100 catch-up), 2026 gift exclusion $19,000 ($38,000 split), 2026 lifetime exemption $15M (mentioned in an explanation). These were verified on irs.gov.
- Recent rule changes: T+1 regular-way settlement since May 28, 2024 (the ex-date now equals the record date, and the Reg T payment period is T+3). FINRA replaced the PDT rule and its $25,000 minimum with intraday margin standards, effective June 4, 2026, with a phase-in through Oct 20, 2027 (Reg Notice 26-10). Only a flashcard covers this, and no question is built on PDT, because exam prep may lag.
- Deliberately not tested yet (could not verify current figures): the FINRA gift limit (Rule 3220), Rule 2165 hold durations, the 529 K-12 annual cap (raised under 2025 law), and LEAPS max term.

## SIE wave 1 + meta.json (2026-11-16, Homework/SIE worker)
- **meta.json**: blueprints for SIE and S7 from FINRA exam pages and the 2025-10 content outline PDFs. SIE: 75 scored + 5 unscored items, 105 min, passing 70, sections SIE-1..4 at 16/44/31/9% (12/33/23/7 items). S7: 125 + 5, 225 min, passing 72, S7-1..4 = F1..F4 at 7/9/73/11% (9/11/91/14 items). All figures were verified, so `unverified` is empty.
- **SIE questions: 104** (sie-q-0001..0104). By section: SIE-1 19, SIE-2 45, SIE-3 31, SIE-4 9. Mostly mc with some tf. True/false items use choices A=True and B=False, the same as S7.
- **SIE flashcards: 60** (sie-f-0001..0060). By section: 10, 26, 18, 6.
- **SIE lessons: 3**: sie-1-01 (regulators/SIPC), sie-2-01 (common and preferred stock), sie-3-01 (order types and capacity). Each runs 830 to 1,160 narration words and has 3 or 4 check segments.
- **sie/LESSON_PLAN.md**: 38 lessons covering every outline subsection, plus the list of facts to re-verify.
- **validate.py** checks both exams. Run `python3 content/validate.py`, or add `--strict` to treat warnings as errors. Current result: 0 errors, 0 warnings.
- The SIE source is in `/workspace/car-study/build_sie/`. Run `python3 build.py` to regenerate the SIE JSON. Edit the source there, not the JSON.
- **Yearly or recently changed facts, tagged `changes-yearly` and verified November 2026:**
  - 2026 IRA limit: $7,500 plus $1,100 catch-up.
  - 2026 gift exclusion: $19,000, so 529 superfunding is $95,000.
  - SIPC: $500,000 / $250,000 cash.
  - FINRA 3220 gift limit: now $300 (since 2026-03-30). MSRB G-20 also moved to $300.
- **Heads-up for both exams:**
  - The pattern day trader rule and its $25,000 minimum were replaced by intraday margin standards effective 2026-06-04 (Regulatory Notice 26-10). Avoid PDT questions.
  - Rule 3290 replaces 3270/3280 on 2027-10-25.
  - Exam retake waits drop to 15/60 days on 2027-01-19.
