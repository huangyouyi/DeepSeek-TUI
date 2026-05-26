import DOMPurify from "dompurify";
import { marked } from "marked";

marked.setOptions({ breaks: true });

export function renderMarkdown(text?: string | null): string {
  if (!text) {
    return "";
  }

  return DOMPurify.sanitize(marked.parse(text) as string);
}
