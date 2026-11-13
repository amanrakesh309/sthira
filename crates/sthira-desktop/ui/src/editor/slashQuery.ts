// Keep detection, dismissal and removal consistent, including /new page and
// custom type names with spaces or non-ASCII letters.
export const SLASH_QUERY_PATTERN = /(?:^|\s)\/([\p{L}\p{N}_-]*(?:[ \t]+[\p{L}\p{N}_-]*)*)$/u;
