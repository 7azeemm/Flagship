import { useState } from 'react';
import { Share2 } from 'lucide-react';

export function ShareButton({ title, path }: { title: string; path: string }) {
  const [notice, setNotice] = useState('');
  const [fallback, setFallback] = useState('');
  const share = async () => {
    const url = new URL(path, window.location.origin).href;
    setNotice('');
    setFallback('');
    if (navigator.share) {
      try {
        await navigator.share({ title, url });
        return;
      } catch (error) {
        if (error instanceof Error && error.name === 'AbortError') return;
      }
    }
    try {
      await navigator.clipboard.writeText(url);
      setNotice('Link copied');
    } catch {
      setFallback(url);
      setNotice('Copy this link to share');
    }
  };
  return (
    <div className="product-share">
      <button className="secondary-button" type="button" onClick={() => void share()}>
        <Share2 size={16} /> Share
      </button>
      {notice && <span role="status">{notice}</span>}
      {fallback && (
        <input
          aria-label="Share link"
          readOnly
          value={fallback}
          onFocus={(event) => event.target.select()}
        />
      )}
    </div>
  );
}
