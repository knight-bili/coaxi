// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/component-a/LICENSE.
import { describe, expect, it } from "vitest"

import {
  buildEmbeddedReferenceUri,
  embeddedReferenceUriFor,
  isEmbeddedReferenceUri,
  parseCoaxiReferenceUri,
  refOfEmbeddedReferenceUri,
} from "./reference-uri"

describe("parseCoaxiReferenceUri", () => {
  it("returns null for non-reference schemes", () => {
    expect(parseCoaxiReferenceUri("https://example.com", "x")).toBeNull()
    expect(parseCoaxiReferenceUri("data:text/plain,abc", "x")).toBeNull()
    expect(parseCoaxiReferenceUri("coaxi://unknown/1", "x")).toBeNull()
  })

  it("parses a file uri, falling back to the basename when label is empty", () => {
    expect(
      parseCoaxiReferenceUri("file:///repo/deep/name.ts", "")
    ).toMatchObject({
      refType: "file",
      id: "name.ts",
      label: "name.ts",
      uri: "file:///repo/deep/name.ts",
      meta: { fileKind: "file" },
    })
  })

  it("parses an agent uri, stripping a leading @ from the label", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://agent/codex", "@Codex")
    ).toMatchObject({
      refType: "agent",
      id: "codex",
      label: "Codex",
      uri: "coaxi://agent/codex",
      meta: { agentType: "codex" },
    })
  })

  it("falls back to the agent type when the agent label is empty", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://agent/claude_code", "")
    ).toMatchObject({
      refType: "agent",
      id: "claude_code",
      label: "claude_code",
      meta: { agentType: "claude_code" },
    })
  })

  it("parses a new-format session uri, recovering the agent type", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://session/codex_abc123", "My chat")
    ).toMatchObject({
      refType: "session",
      id: "codex_abc123",
      label: "My chat",
      uri: "coaxi://session/codex_abc123",
      meta: { agentType: "codex" },
    })
  })

  it("never splits an agent type on its first underscore", () => {
    // claude_code / open_code / open_claw contain underscores; a naive first-`_`
    // split would yield "claude" / "open". The whole `<type>_<external_id>` is
    // the id and the full type is recovered by prefix match.
    expect(
      parseCoaxiReferenceUri("coaxi://session/claude_code_sess-9", "")
    ).toMatchObject({
      id: "claude_code_sess-9",
      meta: { agentType: "claude_code" },
    })
    expect(
      parseCoaxiReferenceUri("coaxi://session/open_code_x", "")?.meta
    ).toEqual({ agentType: "open_code" })
    expect(
      parseCoaxiReferenceUri("coaxi://session/open_claw_y", "")?.meta
    ).toEqual({ agentType: "open_claw" })
  })

  it("treats a legacy numeric session id as opaque (no agent icon)", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://session/123", "Login")
    ).toMatchObject({
      refType: "session",
      id: "123",
      label: "Login",
      uri: "coaxi://session/123",
      meta: null,
    })
  })

  it("treats a non-agent-prefixed token as a plain session id", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://session/randomtoken", "")
    ).toMatchObject({ refType: "session", id: "randomtoken", meta: null })
  })

  it("falls back to #id for an empty session label", () => {
    expect(parseCoaxiReferenceUri("coaxi://session/123", "")?.label).toBe(
      "#123"
    )
  })

  it("parses a commit uri, deriving the short hash", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://commit/%2Frepo@abc1234def5678", "abc1234")
    ).toMatchObject({
      refType: "commit",
      id: "abc1234def5678",
      label: "abc1234",
      uri: "coaxi://commit/%2Frepo@abc1234def5678",
      meta: { shortHash: "abc1234" },
    })
  })

  it("parses a skill uri, moving the label's leading `/`·`$` into the prefix", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://skill/review", "/review")
    ).toMatchObject({
      refType: "skill",
      id: "review",
      label: "review",
      uri: "coaxi://skill/review",
      // The stripped trigger is kept so re-serializing the badge emits the
      // same `/review` token it was parsed from.
      meta: { invocationPrefix: "/" },
    })
    // The `$` prefix ($skill / Codex expert) is stripped — and kept — the same
    // way, so a `$deploy` token never re-serializes to `/deploy`.
    expect(
      parseCoaxiReferenceUri("coaxi://skill/deploy", "$deploy")
    ).toMatchObject({
      label: "deploy",
      meta: { invocationPrefix: "$" },
    })
  })

  it("falls back to the bare id for an empty skill label", () => {
    expect(parseCoaxiReferenceUri("coaxi://skill/deploy", "")).toMatchObject({
      label: "deploy",
      // No literal token to read a trigger from → serializer's `/` default.
      meta: null,
    })
  })

  it("parses an embedded-attachment uri as an inert file badge", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://embedded/9f3c-uuid", "report.pdf")
    ).toMatchObject({
      refType: "file",
      label: "report.pdf",
      uri: "coaxi://embedded/9f3c-uuid",
      meta: { fileKind: "file" },
    })
  })

  it("falls back to a generic label for an empty embedded-attachment label", () => {
    expect(
      parseCoaxiReferenceUri("coaxi://embedded/9f3c-uuid", "")?.label
    ).toBe("resource")
  })

  it("recognizes a freshly minted embedded reference uri", () => {
    const uri = buildEmbeddedReferenceUri()
    expect(isEmbeddedReferenceUri(uri)).toBe(true)
    expect(isEmbeddedReferenceUri("file:///coaxi-embedded/real.ts")).toBe(false)
    expect(isEmbeddedReferenceUri("coaxi://session/abc")).toBe(false)
  })
})

describe("an embedded reference uri that carries its ref", () => {
  const page = "https://shop.test/a b/(1)?q=x&y=%20#top"

  it("gives back the ref it was minted with, whatever is in it", () => {
    const uri = buildEmbeddedReferenceUri(page)
    expect(isEmbeddedReferenceUri(uri)).toBe(true)
    expect(refOfEmbeddedReferenceUri(uri)).toBe(page)
  })

  // Two badges for one page each hold their own block until send, keyed by
  // their display uri — the same uri twice would hand both the second block.
  it("still mints a uri of its own for every badge", () => {
    const first = buildEmbeddedReferenceUri(page)
    const second = buildEmbeddedReferenceUri(page)
    expect(first).not.toBe(second)
    expect(first.startsWith(`${embeddedReferenceUriFor(page)}#`)).toBe(true)
    expect(second.startsWith(`${embeddedReferenceUriFor(page)}#`)).toBe(true)
  })

  // What the transcript rebuilds from an agent's record names it the same way.
  it("reads the ref out of the form a transcript rebuilds", () => {
    expect(refOfEmbeddedReferenceUri(embeddedReferenceUriFor(page))).toBe(page)
  })

  it("carries no ref for a badge minted without one", () => {
    expect(refOfEmbeddedReferenceUri(buildEmbeddedReferenceUri())).toBeNull()
    expect(refOfEmbeddedReferenceUri(buildEmbeddedReferenceUri(""))).toBeNull()
    expect(refOfEmbeddedReferenceUri("coaxi://embedded/9f3c-uuid")).toBeNull()
    expect(refOfEmbeddedReferenceUri("coaxi://embedded/%E0%A4%A")).toBeNull()
    expect(refOfEmbeddedReferenceUri("coaxi://session/abc")).toBeNull()
  })

  it("is still an inert file badge with the label it was given", () => {
    const uri = buildEmbeddedReferenceUri(page)
    expect(parseCoaxiReferenceUri(uri, "Page screenshot")).toMatchObject({
      refType: "file",
      label: "Page screenshot",
      uri,
    })
  })
})
