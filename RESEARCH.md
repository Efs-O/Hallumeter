# HalluMeter — Research Findings & Sources

## Context Window Sizes (confirmed April 2026)

| Model | Context Window | Source |
|---|---|---|
| Claude Sonnet 4.6 | 200,000 tokens | Confirmed live via `claude /usage` — 1M announced but not yet available in Claude Code as of April 2026 |
| Claude Opus 4.6 | 200,000 tokens | Confirmed live via `claude /usage` — 1M announced but not yet available in Claude Code as of April 2026 |
| GPT-5.4 | 1,000,000 tokens | OpenAI launch March 2026 |
| GPT-5.4 Mini | 400,000 tokens | OpenAI launch 2026 |

Sources:
- [Claude 1M Context Window Guide 2026](https://karozieminski.substack.com/p/claude-1-million-context-window-guide-2026)
- [Anthropic 1M Context GA Announcement — Cursor Forum](https://forum.cursor.com/t/anthropic-just-announced-1m-context-ga-at-standard-pricing-for-opus-4-6-sonnet-4-6-when-will-cursor-reflect-this/154701)
- [GPT-5.4 — OpenAI](https://openai.com/index/introducing-gpt-5-4/)
- [GPT-5.4 Mini — OpenAI](https://openai.com/index/introducing-gpt-5-4-mini-and-nano/)
- [GPT-5.4 Complete Guide — NxCode](https://www.nxcode.io/resources/news/gpt-5-4-complete-guide-features-pricing-models-2026)

---

## Hallucination / Degradation Research

### Key Findings

**Chroma "Context Rot" Study (2025)**
- Tested 18 frontier models including GPT-4.1, Claude Opus 4, Gemini 2.5 Pro
- Every single model degrades as context increases — no exceptions
- Accuracy drops from 95% → 60-70% as context fills, even on trivially simple tasks
- Claude Opus 4.6 falls ~14 percentage points over a 750K token span
- Working rule of thumb: ~2% effectiveness loss per 100K tokens added (Claude Code)
- A 1M-token window still shows degradation at 50K tokens (5% fill)

**Lost in the Middle — Stanford/Berkeley (2023, updated 2025)**
- Information at start/end of context: 70-75% accuracy
- Information in the middle: 55-60% accuracy — a 15-20 percentage point gap
- In multi-document QA with 20 documents: 30%+ accuracy drop when relevant doc is in positions 5-15 vs position 1 or 20
- Effect compounds as context fills

**172-Billion-Token Study (arxiv 2603.08274)**
- Even with perfect retrieval (100% exact match), performance degrades as input length increases
- Tested across math, question answering, and code generation
- Degradation is architectural, not a capability gap — inherent to transformer attention

**General Benchmarks**
- At 32K tokens, 11 of 13 models dropped to half their short-context performance
- GPT models hallucinate at higher rates than Claude at equivalent fill levels
- Three compounding mechanisms: lost-in-the-middle effect, attention dilution (quadratic), distractor interference

Sources:
- [Context Rot — Chroma Research](https://research.trychroma.com/context-rot)
- [Context Rot — Morph Complete Guide](https://www.morphllm.com/context-rot)
- [LLM Context Rot — Cobus Greyling / Medium](https://cobusgreyling.medium.com/llm-context-rot-28a6d0399655)
- [Lost in the Middle — Stanford Lecture PDF](https://teapot123.github.io/files/CSE_5610_Fall25/Lecture_12_Long_Context.pdf)
- [Lost in the Middle — ResearchGate](https://www.researchgate.net/publication/378284067_Lost_in_the_Middle_How_Language_Models_Use_Long_Contexts)
- [172B Token Study — arxiv](https://arxiv.org/html/2603.08274)
- [Context Length Alone Hurts LLM Performance — arxiv](https://arxiv.org/html/2510.05381v1)
- [Context Window Limits: Why Your LLM Still Hallucinates](https://pr-peri.github.io/llm/2026/02/13/why-hallucination-happens.html)
- [When More Becomes Less — Medium](https://medium.com/design-bootcamp/when-more-becomes-less-why-llms-hallucinate-in-long-contexts-fc903be6f025)

---

## 2026 Benchmark Update — Why Claude Curves Were Eased (13/04/2026)

Real-world testing suggested the original RIKER-attributed degradation values for Claude were too aggressive, particularly at low context fill. Three 2026 benchmarks were consulted.

### AA-Omniscience — ArtificialAnalysis (Nov 2025 – Apr 2026)

6,000 questions across 42 topics / 6 domains. Penalises wrong answers, rewards abstention. Hallucination rate = incorrect / (incorrect + abstentions + partial). Only 4 of 40 models scored a positive index — all but three are more likely to hallucinate than answer correctly on hard questions.

| Model | Hallucination Rate | Notes |
|---|---|---|
| Claude 4.5 Haiku | 25% | Lowest of any model in snapshot |
| Claude Sonnet 4.6 | ~38% | Strong calibration — prefers abstention |
| Claude Opus 4.1 | ~0–28% | Best overall index score (4.8) — best calibration of all models tested |
| Gemini 3 Pro | ~88% | Highest raw accuracy but worst calibration — guesses aggressively |
| Gemini 3.1 Pro | ~50% | 3.1 update dramatically improved refusal behaviour |
| GPT-5 (high) | ~81% | High accuracy; hallucinates rather than abstaining |
| GPT-5.2 (xhigh) | ~78% | Slight improvement over GPT-5 |

Source: https://artificialanalysis.ai/evaluations/omniscience | Paper: https://arxiv.org/abs/2511.13029

### Vectara HHEM Enterprise Dataset (Feb 2026)

Enterprise-length documents (law, medicine, finance, tech). Grounded summarisation — model must faithfully summarise a provided document. Rates jumped 3–10x vs short-document original dataset.

| Model | Hallucination Rate |
|---|---|
| Gemini 2.5 Flash Lite | 3.3% |
| GPT-4.1 | 5.6% |
| Gemini 2.5 Pro | 7.0% |
| GPT-5 | >10% |
| Claude Sonnet 4.6 | **10.6%** |
| Claude Opus 4.6 | **12.2%** |
| Gemini 3 Pro | 13.6% |

Source: https://github.com/vectara/hallucination-leaderboard

### BullshitBench v2 (April 2026)

100 plausible-sounding nonsense prompts across software, finance, legal, medical, physics. Measures whether models reject false premises vs accept and engage. Reasoning models perform *worse* — trained to find paths to answers, they rationalise false premises rather than reject them.

| Model | Hallucination Rate | Detection Rate |
|---|---|---|
| Claude Sonnet 4.6 (High Reasoning) | **3.0%** | 91% |
| Claude Opus 4.5 (High Reasoning) | **8.0%** | 90% |
| GPT-5.2 / Gemini 3.1 Pro | ~55–65% | <40% |

Source: https://ai.gopubby.com/bullshitbench-v2-llm-hallucination-benchmark-837096e14476

### Context-Length Degradation Anchor (Reuters / Vectara, Apr 2026)

| Context size | Hallucination rate (best models, grounded task) |
|---|---|
| 32K words | ~1.2% |
| 128K words | ~3.2% |

A ~2.7x increase from 32K → 128K. Supports a rising curve shape but confirms the original low-fill anchors were over-estimated for Claude.

### What Changed in curves.json

| Model | Knot | Old value | New value | Reason |
|---|---|---|---|---|
| Sonnet | 25% fill (50K) | 0.14 | **0.10** | AA-Omniscience / BullshitBench show lower baseline |
| Sonnet | 64% fill (128K) | 0.31 | **0.24** | Scaled proportionally |
| Sonnet | 100% fill (200K) | 0.58 | **0.45** | Original peak unsupported; Vectara enterprise ~10.6% flat |
| Opus | 25% fill (50K) | 0.08 | **0.06** | Best-calibrated model of all tested; low fill eased |
| Opus | 64% fill (128K) | 0.19 | **0.15** | Scaled proportionally |
| Opus | 100% fill (200K) | 0.34 | **0.34** | Retained — well-supported by AA-Omniscience |

GPT and Gemini curves were not changed — no new context-fill-specific data available.

---

## Reasoning Effort

No specific research data found on how reasoning level (medium/high/extended thinking) affects degradation curves at different fill percentages.

**Decision:** All curves assume **medium reasoning** as baseline. Users running high or extended thinking sessions will consume context faster — their actual fill % will rise more quickly than a standard session. Treat all thresholds as conservative for high-reasoning workflows.

---

## Derived Degradation Curves (used in curves.json)

### Claude Sonnet 4.6 & Opus 4.6 (1M tokens)
Basis: 2% loss per 100K, 14pp drop over 750K span, early degradation confirmed

| Fill % | Tokens | Risk Score |
|---|---|---|
| 0% | 0 | 0.0 |
| 25% | 250K | 0.15 |
| 50% | 500K | 0.35 |
| 70% | 700K | 0.55 |
| 85% | 850K | 0.75 |
| 100% | 1M | 1.0 |

### GPT-5.4 (1M tokens)
Basis: same context window as Claude but higher baseline hallucination rate per Chroma

| Fill % | Tokens | Risk Score |
|---|---|---|
| 0% | 0 | 0.0 |
| 25% | 250K | 0.2 |
| 50% | 500K | 0.45 |
| 70% | 700K | 0.65 |
| 85% | 850K | 0.8 |
| 100% | 1M | 1.0 |

### GPT-5.4 Mini (400K tokens)
Basis: smaller window, degrades faster proportionally (32K → half performance benchmark)

| Fill % | Tokens | Risk Score |
|---|---|---|
| 0% | 0 | 0.0 |
| 25% | 100K | 0.25 |
| 50% | 200K | 0.5 |
| 70% | 280K | 0.7 |
| 85% | 340K | 0.85 |
| 100% | 400K | 1.0 |

---

## Local / Open-Source Models (Continue extension)

HalluMeter supports sessions run through the [Continue](https://continue.dev) VS Code / JetBrains extension, which covers locally-hosted open-source models (Llama, Qwen, Gemma, Mistral, and others served via llama.cpp, Ollama, or any OpenAI-compatible endpoint).

**Why accuracy is lower for these models:**

All degradation research used to calibrate HalluMeter's curves (Chroma 2025, Stanford/Berkeley Lost in the Middle, the 172B-token study) was conducted on frontier cloud models under controlled benchmarks. No equivalent large-scale study exists for most locally-hosted open-source models.

As a result:
- Local model sessions use HalluMeter's **generic fallback curve** — a conservative average, not a model-specific calibration.
- The risk score and colour thresholds are directional indicators only. They will not be as precisely calibrated as the Claude or Codex curves.
- Context window size is read from `contextLength` in `~/.continue/config.yaml`. If this field is missing, the session is skipped entirely.
- Token fill is derived from a best-effort correlation between Continue's `chatInteraction.jsonl` and `tokensGenerated.jsonl` telemetry files. The correlation window is ±120 seconds — if a token event and its chat event are more than 2 minutes apart, the session is not shown.

**If you find research-backed degradation data for a specific local model**, contributions to `src-tauri/assets/curves.json` are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) and link your primary source in the PR.

---

## Notes for Future Research

- Verify reasoning effort impact on degradation (extended thinking / high reasoning modes)
- Investigate Cursor CLI equivalent to `claude /usage` for token reading
- Investigate Codex context window exposure method (the 7-day quota shown in UI is NOT the per-session context fill)
- Check if newer Chroma research has model-specific curves for GPT-5.4
- Seek degradation benchmarks for open-source models (Llama 3, Qwen 3, Gemma 4) to replace the generic fallback curve
