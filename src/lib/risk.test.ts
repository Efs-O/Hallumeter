import { describe, it, expect } from "vitest";
import {
  stateToColor,
  stateToMessage,
  fillPctToDashOffset,
  AMBER_THRESHOLD,
  RED_THRESHOLD,
} from "./risk";

describe("stateToColor", () => {
  it("green returns correct hex", () => expect(stateToColor("green")).toBe("#22c55e"));
  it("amber returns correct hex", () => expect(stateToColor("amber")).toBe("#f59e0b"));
  it("red returns correct hex",   () => expect(stateToColor("red")).toBe("#ef4444"));
  it("stale returns grey",        () => expect(stateToColor("stale")).toBe("#6b7280"));
  it("unavailable returns grey",  () => expect(stateToColor("unavailable")).toBe("#6b7280"));
});

describe("stateToMessage", () => {
  it("green",       () => expect(stateToMessage("green")).toBe("Functioning normally"));
  it("amber",       () => expect(stateToMessage("amber")).toBe("Logic degrading"));
  it("red",         () => expect(stateToMessage("red")).toBe("Clanker mode activated"));
  it("stale",       () => expect(stateToMessage("stale")).toBe("Waiting..."));
  it("unavailable", () => expect(stateToMessage("unavailable")).toBe("Claude not found"));
});

describe("fillPctToDashOffset", () => {
  const C = 100; // simple circumference for easy math
  it("0% fill = full circumference offset (ring empty)",    () => expect(fillPctToDashOffset(0, C)).toBe(100));
  it("100% fill = 0 offset (ring complete)",                () => expect(fillPctToDashOffset(100, C)).toBe(0));
  it("50% fill = half circumference offset",                () => expect(fillPctToDashOffset(50, C)).toBe(50));
  it("25% fill = 75% of circumference offset",              () => expect(fillPctToDashOffset(25, C)).toBe(75));
});

describe("threshold constants", () => {
  it("AMBER_THRESHOLD is 0.15", () => expect(AMBER_THRESHOLD).toBe(0.15));
  it("RED_THRESHOLD is 0.30",   () => expect(RED_THRESHOLD).toBe(0.30));
});
