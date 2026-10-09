import { Fragment, useEffect, useState } from 'react';
import {
  ArrowDown,
  ArrowUp,
  ArrowUpRight,
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  PackagePlus,
  RefreshCw,
  Search,
  Trash2,
} from 'lucide-react';
import { request } from '../lib/api';
import { fieldLabel, money, sectionLabel, stockLabels } from '../lib/catalog';
import '../activity.css';

type Change = { field: string; old_value: unknown; new_value: unknown };
type Entry = {
  sequence: number;
  observed_at: string;
  kind: string;
  section: string;
  retailer: string;
  product: { id: string; title: string; price: number; status: string; changes: Change[] };
};
type Activity = {
  totals: { added: number; edited: number; removed: number };
  items: Entry[];
  next_cursor: number | null;
  retailers: string[];
  sections: string[];
};
function relativeTime(timestamp: string, now: number) {
  const minutes = Math.floor(Math.max(0, now - Date.parse(timestamp)) / 60000);
  if (!Number.isFinite(minutes)) return '';
  if (minutes < 1) return 'Just now';
  if (minutes < 60) return minutes + (minutes === 1 ? ' minute ago' : ' minutes ago');
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return hours + (hours === 1 ? ' hour ago' : ' hours ago');
  const days = Math.floor(hours / 24);
  return days + (days === 1 ? ' day ago' : ' days ago');
}
const today = () => new Date().toISOString().slice(0, 10);
const readFilters = () => {
  const params = new URLSearchParams(window.location.search);
  return {
    date: params.get('date') || today(),
    retailer: params.get('retailer') || '',
    section: params.get('section') || '',
    kind: params.get('kind') || '',
    search: params.get('search') || '',
  };
};
const display = (value: unknown) =>
  typeof value === 'string' ? stockLabels[value] || value : JSON.stringify(value);

export function ActivityPage() {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60000);
    return () => clearInterval(timer);
  }, []);
  const [filters, setFilters] = useState(readFilters);
  const [data, setData] = useState<Activity | null>(null);
  const [entries, setEntries] = useState<Entry[]>([]);
  const [cursor, setCursor] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    document.title = 'Catalog activity — Flagship';
    const pop = () => {
      if (window.location.pathname !== '/activity') return;
      document.title = 'Catalog activity — Flagship';
      const next = readFilters();
      if (JSON.stringify(next) !== JSON.stringify(filters)) {
        setFilters(next);
        setCursor(null);
        setEntries([]);
      }
    };
    window.addEventListener('popstate', pop);
    return () => window.removeEventListener('popstate', pop);
  }, [filters]);
  const update = (patch: Partial<typeof filters>) => {
    const next = { ...filters, ...patch };
    setFilters(next);
    setCursor(null);
    setEntries([]);
    const params = new URLSearchParams(Object.entries(next).filter(([, value]) => value !== ''));
    window.history.replaceState(window.history.state, '', '/activity?' + params);
  };
  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError('');
    const timer = setTimeout(() => {
      const params = new URLSearchParams(
        Object.entries(filters).filter(([, value]) => value !== ''),
      );
      if (cursor !== null) params.set('before', String(cursor));
      void request('/catalog-activity?' + params, controller.signal)
        .then((raw) => {
          if (controller.signal.aborted) return;
          const result = raw as Activity;
          setData(result);
          setEntries((previous) =>
            cursor === null
              ? result.items
              : [
                  ...previous,
                  ...result.items.filter(
                    (entry) => !previous.some((p) => p.sequence === entry.sequence),
                  ),
                ],
          );
        })
        .catch((error) => {
          if (!controller.signal.aborted)
            setError(error instanceof Error ? error.message : 'Could not load activity.');
        })
        .finally(() => {
          if (!controller.signal.aborted) setLoading(false);
        });
    }, 220);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [filters, cursor, retry]);
  const moveDate = (days: number) => {
    const date = new Date(filters.date + 'T12:00:00Z');
    if (!Number.isFinite(date.getTime())) return;
    date.setUTCDate(date.getUTCDate() + days);
    update({ date: date.toISOString().slice(0, 10) });
  };
  return (
    <main className="activity-page" id="activity-content" tabIndex={-1}>
      <header className="activity-intro">
        <span className="eyebrow">THE CATALOG, IN MOTION</span>
        <h1>Every change. One timeline.</h1>
        <p>New arrivals, price moves and disappearing listings across the stores you follow.</p>
      </header>
      <div className="activity-controls">
        <div className="activity-calendar">
          <button type="button" aria-label="Previous day" onClick={() => moveDate(-1)}>
            <ChevronLeft size={18} />
          </button>
          <CalendarDays size={18} />
          <input
            type="date"
            aria-label="Activity date (UTC)"
            value={filters.date}
            max={today()}
            onChange={(event) => {
              if (event.target.value) update({ date: event.target.value });
            }}
          />
          <button
            type="button"
            aria-label="Next day"
            disabled={filters.date >= today()}
            onClick={() => moveDate(1)}
          >
            <ChevronRight size={18} />
          </button>
          <button type="button" onClick={() => update({ date: today() })}>
            Today
          </button>
        </div>
        <label className="activity-search">
          <Search size={17} />
          <input
            type="search"
            aria-label="Search activity"
            placeholder="Find a product…"
            maxLength={120}
            value={filters.search}
            onChange={(event) => update({ search: event.target.value })}
          />
        </label>
        <div className="activity-filters">
          <select
            aria-label="Retailer"
            value={filters.retailer}
            onChange={(event) => update({ retailer: event.target.value })}
          >
            <option value="">All retailers</option>
            {[...new Set([...(data?.retailers || []), filters.retailer])]
              .filter(Boolean)
              .sort()
              .map((value) => (
                <option key={value}>{value}</option>
              ))}
          </select>
          <select
            aria-label="Category"
            value={filters.section}
            onChange={(event) => update({ section: event.target.value })}
          >
            <option value="">All categories</option>
            {[...new Set([...(data?.sections || []), filters.section])]
              .filter(Boolean)
              .sort()
              .map((value) => (
                <option key={value} value={value}>
                  {sectionLabel(value)}
                </option>
              ))}
          </select>
          <select
            aria-label="Change type"
            value={filters.kind}
            onChange={(event) => update({ kind: event.target.value })}
          >
            <option value="">All changes</option>
            <option value="added">Added</option>
            <option value="edited">Updated</option>
            <option value="removed">Removed</option>
          </select>
        </div>
      </div>
      <div className="activity-day">
        <div className="activity-day-summary">
          <h2>
            {new Date(filters.date + 'T12:00:00Z').toLocaleDateString('en-GB', {
              day: 'numeric',
              month: 'long',
              year: 'numeric',
            })}
          </h2>
          {data && !loading && !error && (
            <div className="activity-totals" aria-label="Day totals for current filters">
              <span className="activity-total-added">
                <strong>{money(data.totals.added)}</strong> Added
              </span>
              <span className="activity-total-edited">
                <strong>{money(data.totals.edited)}</strong> Edited
              </span>
              <span className="activity-total-removed">
                <strong>{money(data.totals.removed)}</strong> Removed
              </span>
            </div>
          )}
        </div>
        <span>Calendar days in UTC · {entries.length} loaded</span>
      </div>
      {error && (
        <p role="alert">
          {error} <button onClick={() => setRetry((value) => value + 1)}>Retry</button>
        </p>
      )}
      <div className="activity-timeline" aria-busy={loading}>
        {entries.map((entry, entryIndex) => {
          const price = entry.product.changes.find((change) => change.field === 'price');
          const drop =
            price && typeof price.old_value === 'number' && typeof price.new_value === 'number'
              ? price.new_value < price.old_value
              : null;
          const Icon =
            entry.kind === 'added'
              ? PackagePlus
              : entry.kind === 'removed'
                ? Trash2
                : drop === true
                  ? ArrowDown
                  : drop === false
                    ? ArrowUp
                    : RefreshCw;
          return (
            <Fragment key={entry.sequence}>
              {(entryIndex === 0 || entries[entryIndex - 1].observed_at !== entry.observed_at) && (
                <div className="activity-scan-heading">
                  <span>
                    Catalog update ·{' '}
                    {new Date(entry.observed_at).toLocaleTimeString('en-GB', {
                      hour: '2-digit',
                      minute: '2-digit',
                      timeZone: 'UTC',
                    })}{' '}
                    UTC
                  </span>
                  <small>
                    {filters.kind ? 'Filtered changes · same scan' : 'All change types · same scan'}
                  </small>
                </div>
              )}
              <article className={'activity-entry activity-' + entry.kind}>
                <span className="activity-marker">
                  <Icon size={18} />
                </span>
                <div className="activity-entry-body">
                  <div className="activity-entry-meta">
                    <strong>
                      {entry.kind === 'added'
                        ? 'New listing'
                        : entry.kind === 'removed'
                          ? 'Listing removed'
                          : 'Listing updated'}
                    </strong>
                    <span>
                      {entry.retailer} · {sectionLabel(entry.section)}
                    </span>
                    <time
                      dateTime={entry.observed_at}
                      title={new Date(entry.observed_at).toLocaleString()}
                    >
                      <b>{relativeTime(entry.observed_at, now)}</b>
                      <span>
                        {new Date(entry.observed_at).toLocaleTimeString('en-GB', {
                          hour: '2-digit',
                          minute: '2-digit',
                          timeZone: 'UTC',
                        })}{' '}
                        UTC
                      </span>
                    </time>
                  </div>
                  <h3>
                    <a href={'/product/' + encodeURIComponent(entry.product.id)}>
                      {entry.product.title}
                    </a>
                  </h3>
                  {entry.kind !== 'removed' && (
                    <p className="activity-price">
                      {money(entry.product.price)} TND{' '}
                      <small>{stockLabels[entry.product.status] || entry.product.status}</small>
                    </p>
                  )}
                  {entry.product.changes.length > 0 && (
                    <div className="activity-changes">
                      {entry.product.changes.map((change, index) => (
                        <div key={index}>
                          <b>{fieldLabel(change.field)}</b>
                          <span>
                            <del>{display(change.old_value)}</del> → {display(change.new_value)}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                  {entry.kind === 'removed' && (
                    <p className="muted">Last listed at {money(entry.product.price)} TND.</p>
                  )}
                  <a
                    className="activity-view"
                    href={'/product/' + encodeURIComponent(entry.product.id)}
                    aria-label={`View ${entry.product.title}`}
                  >
                    View <ArrowUpRight size={14} />
                  </a>
                </div>
              </article>
            </Fragment>
          );
        })}
      </div>
      {loading && (
        <p role="status" className="activity-empty">
          Loading catalog changes…
        </p>
      )}
      {!loading && !error && entries.length === 0 && (
        <div className="activity-empty">
          <CalendarDays size={30} />
          <h3>No recorded changes for this view</h3>
          <p>
            Try another date or broaden your filters. Activity starts with scans recorded after this
            feature was enabled.
          </p>
        </div>
      )}
      {!loading && !error && data?.next_cursor && (
        <button
          className="secondary-button activity-more"
          onClick={() => setCursor(data.next_cursor)}
        >
          Load earlier changes
        </button>
      )}
    </main>
  );
}
