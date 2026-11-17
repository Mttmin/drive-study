
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

## S7 wave 2 (2026-11-16): options calculation bank
- +226 questions, s7-q-0112..0337, all in S7-3 (options). S7 totals so far: 337 questions (S7-1 7, S7-2 11, S7-3 300, S7-4 19) and 70 flashcards.
- Strategies (16): long and short calls and puts, protective put, covered call, short stock + long call, short stock + short put, bull and bear call and put spreads, long and short straddles, and long and short combinations. Each is asked as breakeven, max gain, max loss, and P&L at an expiration price. Also: intrinsic/time value, index option cash settlement, exercise/assignment cost basis and proceeds, and OCC split adjustments.
- Every answer is computed by an exact payoff engine (/workspace/s7build/optmath.py). Each distractor carries the specific error it represents, and the explanation names each one.
- validate.py --strict: 0 errors, 0 warnings.

## S7 wave 3 (2026-11-16): margin and fixed-income/equity/fund calculation bank
- +133 questions, s7-q-0338..0470. S7 totals so far: 470 questions (S7-1 7, S7-2 11, S7-3 384, S7-4 68) and 70 flashcards.
- S7-4 margin (49): Reg T deposits, the $2,000 minimum (long vs short), long and short equity, excess equity/SMA, buying power, maintenance call prices (debit/0.75, credit/1.30), cash needed to meet a call, combined accounts, restricted-account sale release, and dividends to SMA.
- S7-3 (84): TEY and tax-free equivalent, current yield, 30/360 accrued interest, convertible parity and ratio, rights (cum and ex), cumulative preferred arrears, EPS, P/E, payout and current yield, current and quick ratios and working capital, sales charge % and POP, cost basis after splits and stock dividends, capital gain/loss netting ($3,000 ordinary-income offset), wash-sale basis, revenue bond coverage, and CAPM.
- All answers are computed in code. Duplicate stems are skipped automatically. validate.py --strict: 0 errors, 0 warnings.

## SIE wave 2 (2026-11-16, SIE worker)
- **SIE-1 and SIE-4 are complete.**
  - Questions: SIE-1 96, SIE-4 54.
  - Flashcards: SIE-1 48, SIE-4 27.
  - Lessons: SIE-1 all 8, SIE-4 all 3.
- **Totals so far:** 226 questions (ids up to sie-q-0226), 119 flashcards (up to sie-f-0119), 13 of 38 lessons.
  - Existing ids are unchanged. build.py now checks that ids stay stable.
- **Validator:** `validate.py --strict` gives 0 errors and 0 warnings. Lessons must have 700 to 1,600 narration words, and spoken explanations 40 to 130 words.
- **Spoken text:** wave 2 and later items generate `spoken_stem` from the stem and choices. Symbols are turned into words, for example "$18.70" becomes "18 dollars and 70 cents" and "%" becomes "percent". Math items have a hand-written step-by-step `spoken_explanation`.
- **New dated facts:**
  - Exam retake waits are 30/30/180 days now and become 15/15/60 on 2027-01-19 (sie-q retake item tagged `changes-yearly`, `recent-rule-change`).
  - The gift limit is now $300 (Rule 3220 plus the conforming non-cash compensation rules). Ticket gifts are valued at the higher of cost or face.

## S7 wave 4 (2026-11-16): S7-1 / S7-2 concept bank
- +152 questions (s7-q-0471..0622) and +125 flashcards. Covers communications (2210, 2216, 2220, 482, 498), the new-issue process, underwriting types, Reg D 506(b) and 506(c), Reg A tiers, Rules 144, 144A, 147 and Reg S, muni syndicates (Western and Eastern accounts, spread components, order priority, G-37, 15c2-12), account registrations, retirement plans, CIP, CTR, SAR, AML stages, Reg S-P, POAs, trusted contact, Reg BI and care obligations, Form CRS, and margin agreement parts.
- Format: term-identification MC (choices are terms, so they work by ear) plus true/false items. Each item also produces a flashcard.

## S7 wave 5 (2026-11-16): S7-3 / S7-4 concept bank
- +316 questions (s7-q-0623..0938) and +274 flashcards. Covers options vocabulary, equity and preferred types, rights and warrants, ADRs, equity tax, bond types, money markets, Treasuries, agencies and CMO tranches, muni bond types, analysis, Bond Buyer indexes, 529s, investment companies, share classes, variable contracts, REITs, DPPs, hedge funds, risk types, fundamental and technical analysis, fees, gift and estate tax (2026 figures, tagged changes-yearly), statements, ACATS, penny stocks and manipulation, orders and quotes, settlement and good delivery, trade reporting, complaints, arbitration, U4 and U5.
- S7 totals: 938 questions (S7-1 79, S7-2 91, S7-3 655, S7-4 113) and 469 flashcards (S7-1 64, S7-2 73, S7-3 278, S7-4 54). validate.py --strict: 0 errors, 0 warnings.
- FINRA Rule 3220 gift limit is $300 (verified: Reg Notice 26-05, effective Mar 30, 2026). It is used in the S7-1 hand-written items in a later wave.

## SIE wave 3 (2026-11-16): SIE-2 equities, debt, munis and options
- +116 questions (sie-q-0227..0342), all SIE-2, covering lessons sie-2-01 to sie-2-08: equity, rights and warrants, ADRs, Rule 144, bond math and features, Treasuries, agencies, MBS and CMOs, corporate and money market debt, munis, and options basics and practice. Math items have step-by-step spoken explanations.
- +53 SIE-2 flashcards (sie-f-0120..0172).
- 7 new lessons: sie-2-02 to sie-2-08. Each has 5 to 7 minutes of narration, and its check segments point to real question ids.
- SIE totals: 342 questions (SIE-1 96, SIE-2 161, SIE-3 31, SIE-4 54), 172 flashcards (48/79/18/27), and 20 lessons. validate.py --strict: 0 errors, 0 warnings. Existing ids are unchanged.
- No new yearly facts. Avoided: specific option position limits and LEAPS terms.

## S7 wave 6–7 (lessons + rebalance), 2026-11-16 — S7 worker
- Lessons: wrote the 35 remaining lessons; all 38 in LESSON_PLAN.md now exist in content/s7/lessons/ (S7-1: 5, S7-2: 5, S7-3: 21, S7-4: 7). Each runs ~720–920 narration words (about 5–6 min), has 4 check segments spread through the narration that point to real questions with the same lesson_id, closes with a spoken key-terms review, and includes key_terms, mnemonics and a summary. Narration has no $ % / + = < > symbols.
- New hand-written items s7-q-0939..0946 (module w6_hand):
  - FINRA Rule 3220 gift limit of $300 per person per year, effective Mar 30, 2026 (Reg Notice 26-05), tagged changes-yearly; no retail-customer applicability; gifts vs business entertainment.
  - Suitability scenarios: senior income, munis for a low-bracket investor, liquidity horizon, VA exchanges under Rule 2330 (36-month lookback), index-put hedge.
- Rebalance: s7-q-0947..1080 (module w7_more) are computed S7-3 options P&L and fixed-income/muni/equity calcs from the existing generators with new seeds (7712, 7715). Duplicate stems are auto-skipped.
- Totals: S7 has 1080 questions (S7-1 82 = 7.6%, S7-2 91 = 8.4%, S7-3 794 = 73.5%, S7-4 113 = 10.5%), 469 flashcards (64/73/278/54) and 38 lessons.
- validate.py --strict: 0 errors. The 2 warnings are SIE lessons sie-2-14 and sie-2-15 (narration under 750 words), which belong to the SIE worker, not S7.
- Lesson facts verified this wave: the Rule 15g-9 established-customer exemption still exists (eCFR); ACATS validates within 1 business day and completes within 3; records retention is 3 or 6 years and complaints 4 years; arbitration eligibility is 6 years, awards are paid within 30 days, simplified arbitration covers claims up to $50K; the Code of Procedure answer is due in 25 days; SIPC covers $500K including $250K cash; Rule 201 applies after a 10% decline.
- Still avoided (unverified or in flux): Rule 2165 hold durations, the 529 K-12 annual cap, the exact LEAPS max term, TRACE/RTRS reporting deadlines, minimum-tick (sub-penny) changes, PDT details (replaced by the 2026 intraday margin rule), the minor rule violation fine cap, and Rule 3260 time/price wording beyond "generally the day granted".

## SIE wave 4 (2026-11-16): SIE-2 packaged products and risk (SIE-2 complete)
- +103 questions (sie-q-0343..0445), all SIE-2, covering lessons sie-2-09 to sie-2-15: investment companies, fund costs and share classes, variable annuities and Rule 2330, 529s, ABLE and LGIPs, DPPs, REITs, hedge funds, ETFs, ETNs, leveraged and inverse ETFs, and investment risks. Math items include NAV, POP and dollar-cost averaging.
- +53 SIE-2 flashcards (sie-f-0173..0225). SIE-2 is at its targets: 264 questions and 132 flashcards.
- 7 new lessons: sie-2-09 to sie-2-15. All 15 SIE-2 lessons are now done.
- SIE totals: 445 questions (96/264/31/54), 225 flashcards (48/132/18/27), and 27 lessons. validate.py --strict: 0 errors, 0 warnings.
- Yearly facts used, tagged `changes-yearly`: the 2026 gift exclusion is $19,000. It is used for the ABLE contribution cap, the 529 gift item and the $95,000 five-year superfunding figure. Avoided: the ABLE age-of-onset change, 529 K-12 and Roth rollover details, and specific REIT asset and income test percentages beyond the 90% distribution rule.
- (S7 follow-up, 2026-11-16) Fixed TTS readings in the lesson key-term reviews ("four oh one k", "five twenty-nine", "Rule four eighty-two"). Re-ran validate.py --strict: 0 errors, 0 warnings across SIE and S7.

## SIE wave 5 (2026-11-16): SIE-3 complete, all SIE targets met
- +155 questions (sie-q-0446..0600), all SIE-3, covering lessons sie-3-01 to sie-3-12: orders, returns and tax basis, settlement and corporate actions, cash and margin accounts, registrations, IRAs and 401(k)s (2026 limits, `changes-yearly`), AML, books and records, Reg S-P, Rule 2210, telemarketing, Reg BI and Form CRS, manipulation, insider trading, 5130/5131, 3240, 2150, 2165, 4512 and 2040.
- +75 SIE-3 flashcards (sie-f-0226..0300).
- 11 new lessons: sie-3-02 to sie-3-12. All 38 lessons in LESSON_PLAN.md are done, at 5 to 8 minutes each, and every check segment resolves to a real question id.
- **Final SIE totals:** 600 questions (SIE-1 96, SIE-2 264, SIE-3 186, SIE-4 54), 300 flashcards (48/132/93/27), and 38 lessons. validate.py --strict: 0 errors, 0 warnings.
- **Answer-key balancing (build_sie/answer_balance.py):** the old keys were about 70% B. Choice order is now deterministically re-shuffled for eligible MC items (B swapped with C or D, and some B or C swapped with A), and the letters in explanation and spoken_explanation are remapped. spoken_stem is regenerated. MC key mix is now about A 28%, B 29%, C 25%, D 18%.
  - **Ids are unchanged, but the answer key and choice order changed for some existing items, including wave 1 and 2 ids.** The app owner should re-pull all SIE question files instead of merging by id.
  - Items with ambiguous letter tokens (Class B, Reg D, D-12, the article "A") were left as they were.
- Yearly and status facts: 2026 IRA $7,500 plus $1,100; 401(k) $24,500, $8,000, and $11,250 for ages 60 to 63; RMD ages 73 and 75. The PDT rule is mentioned only as "replaced", in one distractor tagged `recent-rule-change`. Fannie and Freddie conservatorship was checked in November 2026, and the FNMA item is tagged `changes-yearly`. Avoided: Roth income limits, IRA deduction phase-outs, SIMPLE and SEP limits, Coverdell beyond $2,000, 10b5-1 cooling-off day counts, and the ITSFEA controlling-person dollar cap.
