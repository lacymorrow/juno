import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS } from "@/lib/constants.generated";
import { MOUTH_CLOSED, MOUTH_OPEN, MOUTH_STILL, mouthScale, type HeadLook } from "../avatarModel";

const { eventHandlers } = vi.hoisted(() => ({
  eventHandlers: new Map<string, (payload: unknown) => void>(),
}));

vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    eventHandlers.set(event, handler);
  },
}));
vi.mock("@/components/ai-elements/persona", () => ({
  Persona: () => <div data-testid="rive" />,
}));

import { AvatarHead } from "../AvatarHead";

const CALM: HeadLook = { rive: "idle", gesture: "calm", cue: null };
const TALKING: HeadLook = { rive: "speaking", gesture: "talk", cue: null };

function emit(event: string, payload: unknown) {
  act(() => {
    eventHandlers.get(event)?.(payload);
  });
}

const mouth = () => screen.getByTestId("avatar-mouth");
const opening = () => Number(mouth().getAttribute("data-open"));

beforeEach(() => {
  eventHandlers.clear();
});

describe("the Avatar's mouth follows Juno's real voice", () => {
  it("stays closed in a speaking state until sound actually starts", () => {
    render(<AvatarHead look={TALKING} facingUp={false} reducedMotion={false} />);
    expect(mouth().getAttribute("data-talking")).toBe("false");
    expect(opening()).toBe(MOUTH_CLOSED);
    // No CSS loop stands in for the voice.
    expect(mouth().getAttribute("style")).toBeNull();
  });

  it("opens with the level and closes on silence", () => {
    render(<AvatarHead look={TALKING} facingUp={false} reducedMotion={false} />);
    emit(EVENTS.TTS_SPEECH_STARTED, { session: 4 });
    expect(mouth().getAttribute("data-talking")).toBe("true");
    emit(EVENTS.TTS_SPEECH_LEVEL, { session: 4, level: 1 });
    expect(opening()).toBeCloseTo(MOUTH_OPEN, 2);
    expect(mouth().style.transform).toBe(`scaleY(${MOUTH_OPEN})`);
    emit(EVENTS.TTS_SPEECH_LEVEL, { session: 4, level: 0.5 });
    const half = opening();
    expect(half).toBeGreaterThan(MOUTH_CLOSED);
    expect(half).toBeLessThan(MOUTH_OPEN);
    emit(EVENTS.TTS_SPEECH_LEVEL, { session: 4, level: 0 });
    expect(opening()).toBe(MOUTH_CLOSED);
  });

  it("moves with the voice whatever the gesture says, and stops when the sound ends", () => {
    render(<AvatarHead look={CALM} facingUp={false} reducedMotion={false} />);
    emit(EVENTS.TTS_SPEECH_LEVEL, { session: 9, level: 0.8 });
    expect(opening()).toBeGreaterThan(MOUTH_CLOSED);
    emit(EVENTS.TTS_SPEECH_ENDED, { session: 9 });
    expect(mouth().getAttribute("data-talking")).toBe("false");
    expect(opening()).toBe(MOUTH_CLOSED);
  });

  it("ignores the end of an older utterance", () => {
    render(<AvatarHead look={TALKING} facingUp={false} reducedMotion={false} />);
    emit(EVENTS.TTS_SPEECH_LEVEL, { session: 2, level: 0.9 });
    emit(EVENTS.TTS_SPEECH_ENDED, { session: 1 });
    expect(mouth().getAttribute("data-talking")).toBe("true");
  });

  it("holds the mouth open without flapping under Reduce Motion", () => {
    render(<AvatarHead look={TALKING} facingUp={false} reducedMotion />);
    emit(EVENTS.TTS_SPEECH_LEVEL, { session: 1, level: 0.2 });
    expect(opening()).toBe(MOUTH_STILL);
    emit(EVENTS.TTS_SPEECH_LEVEL, { session: 1, level: 0.9 });
    expect(opening()).toBe(MOUTH_STILL);
  });
});

describe("mouthScale", () => {
  it("is closed unless Juno is audibly speaking", () => {
    expect(mouthScale({ speaking: false, level: 1, reducedMotion: false })).toBe(MOUTH_CLOSED);
  });

  it("is linear in the level and clamps bad input", () => {
    expect(mouthScale({ speaking: true, level: 0, reducedMotion: false })).toBe(MOUTH_CLOSED);
    expect(mouthScale({ speaking: true, level: 1, reducedMotion: false })).toBeCloseTo(MOUTH_OPEN);
    expect(mouthScale({ speaking: true, level: 7, reducedMotion: false })).toBeCloseTo(MOUTH_OPEN);
    expect(mouthScale({ speaking: true, level: Number.NaN, reducedMotion: false })).toBe(MOUTH_CLOSED);
  });
});
