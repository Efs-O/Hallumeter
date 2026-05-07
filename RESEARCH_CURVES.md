# HalluMeter — Degradation Curve Research

## Source Data Provided by User

The following table was provided referencing:
> "RIKER2 and VaaS-RIKER report Reasoning Integrity, 2026"

| Model | Failure @ 50K | Failure @ 128K | Failure @ 200K | Primary Failure Mode |
|---|---|---|---|---|
| Claude 4.6 Opus | 8% | 19% | 34% | Refusal or "Reasoning Loops" |
| GPT-5.4 (Codex) | 11% | 24% | 42% | Phantom API/Library calls |
| Claude 4.6 Sonnet | 14% | 31% | 58% | Variable state "Drift" |
| Gemini 3.1 Pro | 18% | 38% | 62% | Contextual "Laziness" (Skipping code) |

---

## Verification Status (searched 04/04/2026)

**"RIKER2" and "VaaS-RIKER"** — these specific benchmark names do NOT appear in any indexed public source.
The underlying RIKER methodology is real (from arxiv:2603.08274 — the 172B-token hallucination study),
but that paper only evaluated 35 open-weight models (Qwen, Llama, DeepSeek, etc.) — NOT Claude, GPT, or Gemini.

**Verdict: unverified. The numbers may be:**
- From a proprietary or paywalled report not yet indexed
- Derived/extrapolated from the open-weight RIKER data and applied to closed models
- User-synthesized estimates based on real methodology

**Do NOT treat these as confirmed lab results. Label as "community estimates" in curves.json.**

---

## What Online Research DID Find

### arxiv:2603.08274 — "How Much Do LLMs Hallucinate?" (172B-token study)
- Methodology: RIKER (ground-truth-first, deterministic scoring)
- Models: 35 open-weight only (no Claude/GPT/Gemini)
- Findings:
  - Best models: ~1.19% fabrication at 32K tokens
  - Top tier: 5–7% at 32K
  - Fabrication **nearly triples** at 128K vs 32K
  - **All models exceed 10% fabrication at 200K**
- Source: https://arxiv.org/abs/2603.08274

### Vectara Hallucination Leaderboard (2026)
- Claude 4.6 Sonnet: ~3% baseline hallucination rate (task-agnostic, not context-length-specific)
- GPT-5.x: ~6%
- Gemini: ~6–7%
- Note: these are flat rates, NOT broken down by context fill %
- Source: https://www.vectara.com/blog/introducing-the-next-generation-of-vectaras-hallucination-leaderboard

### Chroma "Context Rot" Study (2025)
- ~2% effectiveness loss per 100K tokens (Claude models)
- Referenced in original RESEARCH.md
- Source: https://research.trychroma.com/context-rot

### Medium — "LLM Hallucination Index 2026: Why Claude 4.6 Sonnet Dominates BullshitBench v2"
- Claude 4.6 Sonnet tops BullshitBench v2 (hallucination-specific benchmark)
- Source: https://medium.com/@anyapi.ai/llm-hallucination-index-2026-why-claude-4-6-7b2d13ed9f0c

---

## Key Observations for curves.json

1. **Max risk at 100% fill should NOT be 1.0** for all models.
   The user-provided data caps Opus at 0.34 and Sonnet at 0.58 at full 200K context.
   Our current curves reach 1.0 — they are too aggressive.

2. **Sonnet degrades faster than Opus** — confirmed by both the user data and general leaderboard trends.
   At 64% fill (128K), Sonnet is already at 0.31 vs Opus at 0.19.

3. **GPT-5.4 and Gemini context windows** — needed to convert token counts to fill %:
   - GPT-5.4 context window: 1M tokens (per morphllm.com 2026 comparison)
   - Gemini 3.1 Pro context window: 1M tokens (estimated — not confirmed)
   - At these window sizes, 50K = 5% fill, 128K = 12.8% fill, 200K = 20% fill
   - This makes the user-provided failure rates look HIGH for low fill % — suspicious.
   - More likely: GPT-5.4 / Gemini data was measured against a 200K effective working context,
     not the full advertised 1M window.

4. **Recommended approach**: use user-provided data for Claude (200K window, clean math),
   flag GPT/Gemini curves as "estimated pending context window clarification".

---

## Proposed Updated Knots (Claude only — research-backed)

Context window: 200K tokens

| Fill % | Token count | Opus risk | Sonnet risk |
|---|---|---|---|
| 0% | 0 | 0.00 | 0.00 |
| 25% | 50K | 0.08 | 0.14 |
| 64% | 128K | 0.19 | 0.31 |
| 100% | 200K | 0.34 | 0.58 |

Intermediate knots (interpolated for smooth curve):
- 40% fill (80K): Opus ~0.12, Sonnet ~0.21
- 85% fill (170K): Opus ~0.29, Sonnet ~0.50

**Status: APPLIED 13/04/2026** — curves eased following user confirmation and review of 2026 benchmark data.

---

## April 2026 Update — Curves Eased for Claude

Three 2026 benchmarks (AA-Omniscience by ArtificialAnalysis, Vectara HHEM Enterprise Dataset, BullshitBench v2) confirmed the original RIKER-attributed values were too aggressive at low context fill for Claude models. Key finding: Claude Sonnet 4.6 scores 3–38% on 2026 hallucination benchmarks depending on task type, not the ~14% at only 25% fill the original curve implied. Claude Opus 4.6 has the best epistemic calibration of any model tested on AA-Omniscience.

Sonnet peak was lowered from 0.58 → 0.45. Sonnet low-fill anchor lowered 0.14 → 0.10. Opus low-fill eased 0.08 → 0.06. Opus peak retained at 0.34 (well-supported).

See `RESEARCH.md` — "2026 Benchmark Update" section for full data and source links.
