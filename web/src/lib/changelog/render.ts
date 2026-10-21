import { marked } from 'marked';
import sanitizeHtml from 'sanitize-html';

/** Entry bodies are inline Markdown; render once at import time and sanitize. */
export function renderBody(md: string): string {
  const html = marked.parseInline(md, { async: false }) as string;
  return sanitizeHtml(html, { allowedTags: ['a', 'code', 'em', 'strong', 'kbd', 'br'], allowedAttributes: { a: ['href'] }, allowedSchemes: ['http', 'https', 'mailto'] });
}
