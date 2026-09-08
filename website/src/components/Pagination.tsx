import { ArrowLeft, ArrowRight } from 'lucide-react';
import { pageNumbers } from '../lib/catalog';
export function Pagination({
  page,
  total,
  onChange,
}: {
  page: number;
  total: number;
  onChange: (page: number) => void;
}) {
  if (total < 1) return null;
  const pages = pageNumbers(page, total);
  return (
    <nav className="pagination" aria-label="Result pages">
      <div>
        <button disabled={page <= 1} onClick={() => onChange(page - 1)} aria-label="Previous page">
          <ArrowLeft size={16} />
        </button>
        {pages.map((value, index) => (
          <span key={value} className="page-item">
            {index > 0 && value - pages[index - 1] > 1 && <span className="page-gap">…</span>}
            <button
              aria-label={`Page ${value}`}
              aria-current={page === value ? 'page' : undefined}
              onClick={() => onChange(value)}
            >
              {value}
            </button>
          </span>
        ))}
        <button disabled={page >= total} onClick={() => onChange(page + 1)} aria-label="Next page">
          <ArrowRight size={16} />
        </button>
      </div>
    </nav>
  );
}
