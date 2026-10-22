import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export function MarkdownMessage({ content }: { content: string }) {
  return <div data-content className="markdown-body">
    <Markdown remarkPlugins={[remarkGfm]} skipHtml components={{
      a: ({ href, children }) => href && /^https?:\/\//i.test(href)
        ? <a href={href} target="_blank" rel="noopener noreferrer">{children}</a>
        : <span>{children}</span>,
      img: ({ alt }) => <span className="markdown-image-alt">[이미지{alt ? `: ${alt}` : ''}]</span>,
      table: ({ children }) => <div className="markdown-table-scroll"><table>{children}</table></div>,
    }}>{content}</Markdown>
  </div>;
}
