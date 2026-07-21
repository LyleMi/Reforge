import { describe, expect, it } from "vitest";
import { defaultViewState, parseViewState, serializeViewState } from "../src/viewState";
describe("evidence view state",()=>{
  it("uses safe defaults for removed filters and layers",()=>expect(parseViewState("#unknown?severity=critical&layer=severity&sort=priority")).toEqual(defaultViewState));
  it("round trips evidence filters",()=>{const state=parseViewState("#evidence?query=main&kind=large_file&sort=path");expect(serializeViewState(state)).toBe("#evidence?query=main&kind=large_file&sort=path")});
  it("rejects unknown files",()=>{const report={schema_version:23,raw_metrics:{files:[{path:"src/main.rs"}]},findings:[],issues:[]} as never;expect(parseViewState("#map?file=missing.rs",report).file).toBeNull()});
});
