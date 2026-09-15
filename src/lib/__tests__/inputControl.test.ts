import { describe, expect, it } from "vitest";

import {
  REOPEN_ESCALATION_THRESHOLD,
  describeRequest,
  drivingLabel,
  reopenNoticeFor,
  shouldShowMouseControlOffer,
} from "../inputControl";

describe("reopenNoticeFor", () => {
  it("says nothing when nobody has tried to reopen Juno", () => {
    expect(reopenNoticeFor(0)).toBe("none");
  });

  it("ignores a nonsense count rather than guessing", () => {
    expect(reopenNoticeFor(-1)).toBe("none");
    expect(reopenNoticeFor(Number.NaN)).toBe("none");
  });

  it("shows the quiet hint for the first tries", () => {
    expect(reopenNoticeFor(1)).toBe("hint");
    expect(reopenNoticeFor(REOPEN_ESCALATION_THRESHOLD - 1)).toBe("hint");
  });

  it("escalates once the tries look like scrambling", () => {
    expect(reopenNoticeFor(REOPEN_ESCALATION_THRESHOLD)).toBe("escalated");
    expect(reopenNoticeFor(REOPEN_ESCALATION_THRESHOLD + 5)).toBe("escalated");
  });

  it("escalates at three, the documented threshold", () => {
    expect(REOPEN_ESCALATION_THRESHOLD).toBe(3);
  });
});

describe("shouldShowMouseControlOffer", () => {
  const base = { mode: "ask" as const, dismissed: false, offeredThisSession: false };

  it("offers while Juno still asks every time", () => {
    expect(shouldShowMouseControlOffer(base)).toBe(true);
  });

  it("stays quiet once Juno is already allowed to take the mouse", () => {
    expect(shouldShowMouseControlOffer({ ...base, mode: "always" })).toBe(false);
  });

  it("stays quiet after the person said not to ask again", () => {
    expect(shouldShowMouseControlOffer({ ...base, dismissed: true })).toBe(false);
  });

  it("never offers twice in one session", () => {
    expect(shouldShowMouseControlOffer({ ...base, offeredThisSession: true })).toBe(
      false,
    );
  });
});

describe("describeRequest", () => {
  it("names the app the work happens in", () => {
    expect(
      describeRequest({
        request_id: "r1",
        tool: "computer",
        reason: "drag the file onto the timeline",
        target_app: "Final Cut Pro",
      }),
    ).toBe("drag the file onto the timeline in Final Cut Pro");
  });

  it("drops the app when the backend did not name one", () => {
    expect(
      describeRequest({
        request_id: "r1",
        tool: "computer",
        reason: "drag the file onto the timeline",
      }),
    ).toBe("drag the file onto the timeline");
  });

  it("still reads like a sentence when there is no reason", () => {
    expect(
      describeRequest({ request_id: "r1", tool: "computer", reason: "" }),
    ).toBe("finish what you asked for");
  });
});

describe("drivingLabel", () => {
  it("names the app Juno has the pointer in", () => {
    expect(drivingLabel({ active: true, target_app: "Safari" })).toBe(
      "using the mouse in Safari",
    );
  });

  it("falls back to the plain fact", () => {
    expect(drivingLabel({ active: true })).toBe("using the mouse");
  });
});
