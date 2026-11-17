# SIE lesson plan (covers every FINRA outline subsection)

Source: FINRA SIE Content Outline (finra.org, 2025-10 PDF, "© 2025 FINRA"). 75 scored items + 5 unscored pretest items, 1 h 45 min:
- SIE-1 Knowledge of Capital Markets: 16%, 12 items
- SIE-2 Understanding Products and Their Risks: 44%, 33 items
- SIE-3 Understanding Trading, Customer Accounts and Prohibited Activities: 31%, 23 items
- SIE-4 Overview of the Regulatory Framework: 9%, 7 items

Conventions:
- Lesson ids are `sie-<section#>-<nn>`. Question ids `sie-q-NNNN` and flashcard ids `sie-f-NNNN` are sequential across all SIE files, so always continue from the current maximum.
- Every question's `lesson_id` must be a lesson in this table.
- Authoring source lives in `/workspace/car-study/build_sie/`. Run `python3 build.py` there to regenerate the content. Later waves should add to the q_*.py, flash.py and lessons.py files, or write new modules and import them in build.py, appending only so existing ids stay stable.
- Lessons run 750 to 1,500 narration words, with 2 to 4 check segments using real question ids.
- Status: done means the file exists in lessons/.

Question targets (600 total) by weight: SIE-1 ≈ 96, SIE-2 ≈ 264, SIE-3 ≈ 186, SIE-4 ≈ 54. Flashcards (300): ≈ 48 / 132 / 93 / 27.

| Lesson id | Title | Outline coverage | Status |
|---|---|---|---|
| sie-1-01 | Who regulates the markets: SEC, SROs, states, the Fed and SIPC | 1.1.1 SEC; 1.1.2 SROs (FINRA, CBOE, MSRB); 1.1.3 Treasury/IRS, NASAA, Fed, SIPC, FDIC | **done (wave 1)** |
| sie-1-02 | Market participants | 1.1.4 investor types (accredited, institutional, retail), introducing/clearing/prime brokers, IAs, municipal advisors, issuers and underwriters, traders and market makers, custodians, transfer agents, DTCC, OCC | |
| sie-1-03 | Market structure | 1.2.1 primary, secondary (exchange, OTC, electronic), third and fourth markets | |
| sie-1-04 | The Fed and monetary vs. fiscal policy | 1.3.1 FOMC, open market ops, discount rate, fed funds, prime rate, money supply | |
| sie-1-05 | Business economics | 1.3.2 financial statements, business cycle, leading, lagging and coincident indicators, inflation and deflation, cyclical, defensive and growth stocks, Keynesian vs. monetarist | |
| sie-1-06 | International economics | 1.3.3 balance of payments, GDP vs. GNP, exchange rates | |
| sie-1-07 | Underwriting and offerings | 1.4 investment bankers, syndicates, IPO, secondary and follow-on offerings, firm commitment and best efforts types, spread, Rules 2269 and 5250, G-11, G-32, G-34 | |
| sie-1-08 | Registration, prospectus and exemptions | 1.4 '33 Act Sections 7, 8, 10 and 23, cooling-off, red herring, final and summary prospectus, free writing prospectus (164), shelf (415), Reg D and accredited investor (215), 144, 144A, 145, 147, blue-sky, official statement, program disclosure document | |
| sie-2-01 | Common and preferred stock | 2.1.1 ownership, liquidation priority, voting, preferred types | **done (wave 1)** |
| sie-2-02 | Rights, warrants, ADRs and restricted and control stock | 2.1.1 rights, warrants, ADRs, Rule 144, 3a11-1, 10b-18 buybacks | |
| sie-2-03 | Bond fundamentals | 2.1.2 par, coupon, yields (nominal, CY, YTM, YTC), price and yield, ratings, call and convertible features, short- vs. long-term | |
| sie-2-04 | Treasuries, agencies and MBS | 2.1.2 bills, notes, bonds, TIPS, STRIPS and receipts, auctions, GNMA, FNMA, FHLMC, CMOs and ABS | |
| sie-2-05 | Corporate debt and money market instruments | 2.1.2 secured and unsecured, debentures, commercial paper, BAs, CDs, negotiated vs. competitive underwriting | |
| sie-2-06 | Municipal securities | 2.1.2 GO, revenue, special types, taxable munis, short-term notes, tax-equivalent yield | |
| sie-2-07 | Options basics | 2.1.3 calls and puts, equity vs. index, premium, strike, expiration, physical vs. cash settlement, American vs. European, breakeven, max gain and loss | |
| sie-2-08 | Options in practice | 2.1.3 ITM, ATM and OTM, covered vs. uncovered, hedging vs. speculation, exercise and assignment, ODD, OCC, Rule 2360 | |
| sie-2-09 | Investment companies | 2.1.4 1940 Act types, open-end, closed-end, UITs, NAV, Sections 3, 4 and 5 | |
| sie-2-10 | Fund costs and sales charges | 2.1.4 loads, share classes, 12b-1, breakpoints, ROA, LOI, Rule 2342 | |
| sie-2-11 | Variable annuities | 2.1.4 separate account, accumulation and annuity units, surrender charges, 1035 exchanges, taxation, Rule 2330 | |
| sie-2-12 | Municipal fund securities | 2.1.5 529 prepaid and savings plans, ABLE, LGIPs, owner vs. beneficiary, tax advantages, direct vs. adviser-sold, D-12, G-45 | |
| sie-2-13 | DPPs, REITs and hedge funds | 2.1.6 to 2.1.8 LPs, TICs, pass-through, illiquidity, REIT types and distribution rule, hedge fund structure and minimums | |
| sie-2-14 | Exchange-traded products | 2.1.9 ETFs vs. ETNs, active vs. passive, fees, leveraged and inverse | |
| sie-2-15 | Investment risks and mitigation | 2.2 capital, credit, currency, inflation, interest rate and reinvestment, liquidity, systematic and non-systematic, political, prepayment risk; diversification, rebalancing, hedging | |
| sie-3-01 | Order types, trade capacity and sentiment | 3.1.1 market, limit, stop, GTC, discretionary, solicited, bid and ask, principal vs. agency, long and short, bullish and bearish | **done (wave 1)** |
| sie-3-02 | Investment returns | 3.1.2 interest, dividends (cash and stock, key dates), realized and unrealized gains, yields, total return, basis points, cost basis, benchmarks | |
| sie-3-03 | Settlement and corporate actions | 3.1.3 and 3.1.4 settlement cycles, book entry; splits and reverse splits, buybacks, tender and exchange offers, rights offerings, M&A, proxies (Rule 2251) | |
| sie-3-04 | Account types | 3.2.1 cash, margin (Reg T, 4210, 2264), options, discretionary (3260), fee-based vs. commission, educational | |
| sie-3-05 | Account registrations | 3.2.2 individual, joint, corporate, trust, custodial (UTMA), partnership | |
| sie-3-06 | Retirement and education accounts | 3.2.2 IRAs, Roth, qualified plans, contributions, RMDs, Coverdell (changes-yearly figures) | |
| sie-3-07 | Anti-money laundering | 3.2.3 stages, AML program (3310), CIP, SAR, CTR, FinCEN, OFAC SDN, PATRIOT Act 314, 326, 352 | |
| sie-3-08 | Books, records, privacy and customer protection | 3.2.4 17a-3, 17a-4, confirmations (10b-10, 2232), statements (2231), hold mail (3150), BCP (4370), customer protection and custody, Reg S-P | |
| sie-3-09 | Communications, telemarketing and best interest | 3.2.5 Rule 2210 classes, approvals, telemarketing (3230), do-not-call, Reg BI, Form CRS, KYC (2090), suitability (2111) | |
| sie-3-10 | Market manipulation | 3.3.1 rumors, pump and dump, front running (5270), excessive trading, marking the close and open, backing away, freeriding, trading ahead (5320) | |
| sie-3-11 | Insider trading | 3.3.2 MNPI, tippers and tippees, 10b-5, 10b5-1, 10b5-2, ITSFEA, Sections 20A and 21A penalties | |
| sie-3-12 | Other prohibited activities | 3.3.3 Rule 5130 IPO restrictions, borrowing and lending (3240), sharing and guarantees (2150), seniors (2165, trusted contact), unregistered persons (2040), signatures of convenience, falsified records | |
| sie-4-01 | Registration, qualification and CE | 4.1.1 registered vs. non-registered persons, ineligibility, statutory disqualification, fingerprinting (17f-2), background checks, blue-sky registration, CE Regulatory and Firm Element (1240), exam windows and retake waits | |
| sie-4-02 | Forms U4 and U5, complaints and red flags | 4.2.1 U4 and U5 purpose and timing, misleading filings (1122), complaints (4513, 4530), red flags, BrokerCheck (8312), arbitration disclosure (2263) | |
| sie-4-03 | Reportable events and outside activities | 4.2.2 OBAs (3270), PSTs (3280), Rule 3290 transition, political contributions (G-37), gifts (3220, G-20), non-cash compensation, entertainment, liens, bankruptcy | |

## Facts that change: re-verify every wave

- 2026 IRA limit is $7,500, with a $1,100 catch-up at 50 or older. The 2026 401(k) limit is $24,500, with an $8,000 catch-up and $11,250 for ages 60 to 63. Source: IRS IR-2025-111.
- The 2026 gift tax annual exclusion is $19,000, unchanged from 2025, so 529 five-year superfunding is $95,000 per donor. Source: IRS.
- SIPC covers $500,000 per capacity, including $250,000 cash. Source: sipc.org, checked November 2026.
- Since March 30, 2026, the FINRA Rule 3220 gift limit is $300 (Regulatory Notice 26-05). MSRB G-20 moved to $300 on June 1, 2026 for FINRA-member dealers, and on December 1, 2026 for bank dealers and municipal advisors.
- The pattern day trader rule and its $25,000 minimum were REPLACED by intraday margin standards (Rule 4210 amendments, Regulatory Notice 26-10). They took effect June 4, 2026, with a phase-in through October 2027. Do not write PDT $25k questions.
- Rule 3290, Outside Activities, replaces Rules 3270 and 3280 effective October 25, 2027. It was approved September 2026 (Regulatory Notice 26-20).
- Exam retake waits are 30 days after the 1st and 2nd failures and 180 days after the 3rd. These drop to 15 and 60 days on January 19, 2027 (Regulatory Notice 26-18).
- Settlement has been T+1 since May 28, 2024, so the ex-dividend date equals the record date.
- AVOID until verified: the 529 K-12 tuition cap (changed by the 2025 tax law), ABLE onset-age changes, and Coverdell details beyond the $2,000 limit.
