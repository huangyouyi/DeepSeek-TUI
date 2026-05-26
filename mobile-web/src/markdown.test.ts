import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders markdown with line breaks", () => {
    expect(renderMarkdown("**Hello**\nworld")).toContain("<strong>Hello</strong><br>world");
  });

  it("sanitizes script content from rendered markdown", () => {
    const html = renderMarkdown("Safe<script>alert('xss')</script>");

    expect(html).toContain("Safe");
    expect(html).not.toContain("<script");
    expect(html).not.toContain("alert");
  });
});
