import { describe, expect, it } from "vitest";
import { defaultViewState, parseViewState, serializeViewState } from "../src/viewState";

describe("view state hash", () => {
  it("uses safe defaults for invalid values", () => {
    expect(parseViewState("#unknown?severity=loud&sort=random&layer=heat&scope=lines")).toEqual(defaultViewState);
  });
  it("round trips issue filters", () => {
    const state = parseViewState("#issues?query=main&severity=critical&kind=large_file&sort=path");
    expect(serializeViewState(state)).toBe("#issues?query=main&severity=critical&kind=large_file&sort=path");
  });
  it("rejects a file absent from the report", () => {
    expect(parseViewState("#map?file=missing.rs", { schema_version: 19, raw_metrics: { files: [{ path: "src/main.rs" }] } }).file).toBeNull();
  });
});
