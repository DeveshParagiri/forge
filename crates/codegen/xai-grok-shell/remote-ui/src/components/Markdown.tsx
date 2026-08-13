import type { ComponentPropsWithoutRef } from "react";
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkBreaks from "remark-breaks";
import remarkGfm from "remark-gfm";

function ExternalLink(props: ComponentPropsWithoutRef<"a">) {
  return <a {...props} target="_blank" rel="noreferrer noopener" />;
}

export function Markdown({ text }: { text: string }) {
  return (
    <div className="chat-markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkBreaks]}
        rehypePlugins={[rehypeSanitize]}
        components={{
          a: ExternalLink,
          img: ({ alt }) => <span className="markdown-image-placeholder">[Image: {alt || "attachment"}]</span>,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
