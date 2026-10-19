// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/codeg/LICENSE.
// @vitest-environment jsdom
import { describe, it, expect } from "vitest"
import { buildVisualizeDocument } from "./codex-visualize-card"
describe("buildVisualizeDocument", () => {
  const assets = { css: ":root{--x:1}", kit: null, calendar: null }

  it("wraps a fragment in a themed, CSP-protected document", () => {
    const doc = buildVisualizeDocument({
      fragment: "<p>hi</p>",
      assets,
      title: "A <b>",
      dark: true,
      themeOverrides: ":root{--background:red}",
    })
    expect(doc).toContain('style="color-scheme:dark"')
    expect(doc).toContain("<title>A &lt;b&gt;</title>")
    expect(doc).toContain(":root{--x:1}")
    expect(doc).toContain(":root{--background:red}")
    expect(doc).toContain("<p>hi</p>")
    expect(doc).toMatch(/Content-Security-Policy.*default-src 'none'/)
    // The frame paints no page background of its own, so the card's surface
    // (and the workspace background behind it) shows through — on `:root`,
    // which the skill's own `:root{…!important}` background would otherwise
    // outrank.
    expect(doc).toContain(":root,body{background:transparent !important")
    expect(doc).toContain("html>body{padding:1rem 1.25rem}")
    // In-page links stay in the frame instead of loading codeg into it.
    expect(doc).toContain('<base href="about:srcdoc">')
    expect(doc).toContain("base-uri about:")
  })

  it("gives fragments the window.openai state API before they run", () => {
    const doc = buildVisualizeDocument({
      fragment: "<script>draw(window.openai.widgetState)</script>",
      assets,
      title: "t",
      dark: false,
      themeOverrides: "",
    })
    const stub = doc.indexOf("window.openai=openai")
    expect(stub).toBeGreaterThan(-1)
    expect(stub).toBeLessThan(doc.indexOf("draw(window.openai.widgetState)"))
    expect(doc).toContain('theme:"light"')
  })

  it("puts the fragment into the plugin kit's slot when one is available", () => {
    const doc = buildVisualizeDocument({
      fragment: "<p>hi</p>",
      assets: {
        css: "",
        kit: "<!--__INLINE_VISUALIZATION_FRAGMENT__--><script>tooltips()</script>",
        calendar: "registerCalendar()",
      },
      title: "t",
      dark: false,
      themeOverrides: "",
    })
    expect(doc).toContain("<p>hi</p><script>tooltips()</script>")
    // calendar.js is only injected for fragments that use <viz-calendar>.
    expect(doc).not.toContain("registerCalendar()")
  })

  it("injects the calendar runtime for fragments that use it", () => {
    const doc = buildVisualizeDocument({
      fragment: "<viz-calendar></viz-calendar>",
      assets: { css: "", kit: null, calendar: "registerCalendar()" },
      title: "t",
      dark: false,
      themeOverrides: "",
    })
    expect(doc).toContain("<script>registerCalendar()</script><viz-calendar>")
  })
})
