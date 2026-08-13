import type {
  ReviewDiffTheme,
  ReviewHighlightedToken,
} from "../review/shikiReviewHighlighter";

export type MarkdownHighlightedCode = ReadonlyArray<ReadonlyArray<ReviewHighlightedToken>>;

export interface MarkdownCodeHighlightInput {
  readonly code: string;
  readonly enabled: boolean;
  readonly language: string;
  readonly theme: ReviewDiffTheme;
}

export function useMarkdownCodeHighlight(input: {
  readonly code: string;
  readonly enabled: boolean;
  readonly language: string | null | undefined;
  readonly theme: ReviewDiffTheme;
}): MarkdownHighlightedCode | null {
  return null;
}
