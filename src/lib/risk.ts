// Pure functions for risk state mapping — extracted for testability.
// Rust owns curve interpolation; these functions map the received payload to UI values.

// Absolute risk score thresholds — model-agnostic. The curve does per-model work;
// these define what "amber" and "red" mean on the 0.0–1.0 risk scale.
// Must stay in sync with AMBER_THRESHOLD / RED_THRESHOLD in src-tauri/src/core.rs.
export const AMBER_THRESHOLD = 0.15;
export const RED_THRESHOLD   = 0.30;

export type RiskState = "green" | "amber" | "red" | "stale" | "unavailable";

export function stateToColor(state: RiskState): string {
  switch (state) {
    case "green":       return "#22c55e";
    case "amber":       return "#f59e0b";
    case "red":         return "#ef4444";
    case "stale":       return "#6b7280";
    case "unavailable": return "#6b7280";
  }
}

export function stateToMessage(state: RiskState): string {
  switch (state) {
    case "green":       return "Functioning normally";
    case "amber":       return "Logic degrading";
    case "red":         return "Clanker mode activated";
    case "stale":       return "Waiting...";
    case "unavailable": return "Claude not found";
  }
}

// Converts fill % to SVG stroke-dashoffset.
// At 0% fill the ring is empty (full offset). At 100% fill the ring is complete (zero offset).
export function fillPctToDashOffset(fillPct: number, circumference: number): number {
  return circumference * (1 - fillPct / 100);
}
