import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowUpRight,
  ChevronDown,
  Cpu,
  Info,
  LoaderCircle,
  Moon,
  Search,
  Sun,
} from 'lucide-react';
import type { ServerInfo } from '../types';
import { readQuery, sectionLabel } from '../lib/catalog';
import { parseSearchSuggestions, request } from '../lib/api';
import '../header.css';

export function Brand() {
  return (
    <span className="brand">
      <img src="/favicon.svg" alt="" width="34" height="34" />
      mythos<span className="brand-period">.</span>
    </span>
  );
}

export const Header = memo(function Header({
  section,
  locationKey,
  filters,
  categories,
  onSection,
  onProduct,
  onHome,
  onAbout,
}: {
  section: string;
  locationKey: string;
  filters: Record<string, string[]>;
  categories: ServerInfo['categories'];
  onSection: (url: string) => void;
  onProduct: (id: string) => void;
  onHome: () => void;
  onAbout: () => void;
}) {
  const [theme, setTheme] = useState(document.documentElement.dataset.theme || 'dark');
  const [open, setOpen] = useState<string | null>(null);
  const [hidden, setHidden] = useState(false);
  const [left, setLeft] = useState(12);
  const [search, setSearch] = useState('');
  const [suggestions, setSuggestions] = useState<ReturnType<typeof parseSearchSuggestions>>([]);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState(false);
  const [activeSuggestion, setActiveSuggestion] = useState(-1);
  const root = useRef<HTMLElement>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastScrollY = useRef(window.scrollY);
  const scrollDistance = useRef(0);
  const cancelClose = () => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  };
  const close = () => {
    cancelClose();
    setOpen(null);
  };
  const scheduleClose = () => {
    cancelClose();
    timer.current = setTimeout(() => {
      if (!panel.current?.contains(document.activeElement)) setOpen(null);
    }, 180);
  };
  useEffect(() => {
    setHidden(false);
    setOpen(null);
    lastScrollY.current = window.scrollY;
    scrollDistance.current = 0;
  }, [locationKey]);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) {
        setOpen(null);
        setSearchOpen(false);
      }
    };
    document.addEventListener('pointerdown', outside);
    return () => {
      cancelClose();
      document.removeEventListener('pointerdown', outside);
    };
  }, []);
  useEffect(() => {
    const value = search.trim();
    setActiveSuggestion(-1);
    setSearchError(false);
    if (value.length < 2) {
      setSuggestions([]);
      setSearchLoading(false);
      return;
    }
    const controller = new AbortController();
    setSearchLoading(true);
    const timer = window.setTimeout(() => {
      request(`/search/suggestions?q=${encodeURIComponent(value)}`, controller.signal)
        .then(parseSearchSuggestions)
        .then((items) => {
          if (!controller.signal.aborted) setSuggestions(items);
        })
        .catch(() => {
          if (!controller.signal.aborted) {
            setSuggestions([]);
            setSearchError(true);
          }
        })
        .finally(() => {
          if (!controller.signal.aborted) setSearchLoading(false);
        });
    }, 220);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [search]);
  useEffect(() => {
    let frame = 0;
    const onScroll = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        const next = Math.max(0, window.scrollY);
        const delta = next - lastScrollY.current;
        const headerHeight = root.current?.offsetHeight || 0;
        if (Math.sign(delta) !== Math.sign(scrollDistance.current)) scrollDistance.current = 0;
        scrollDistance.current += delta;
        if (next <= headerHeight) {
          scrollDistance.current = 0;
          setHidden(false);
        } else if (scrollDistance.current > 18) {
          setHidden(true);
          setOpen(null);
          scrollDistance.current = 0;
        } else if (scrollDistance.current < -18) {
          setHidden(false);
          scrollDistance.current = 0;
        }
        lastScrollY.current = next;
        frame = 0;
      });
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      window.removeEventListener('scroll', onScroll);
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);
  useLayoutEffect(() => {
    if (!open) return;
    const position = () => {
      const bounds = root.current?.getBoundingClientRect();
      const anchor = trigger.current?.getBoundingClientRect();
      if (bounds && anchor)
        setLeft(
          Math.max(
            12,
            Math.min(
              anchor.left - bounds.left,
              bounds.width - (panel.current?.offsetWidth || 320) - 12,
            ),
          ),
        );
    };
    position();
    window.addEventListener('resize', position);
    const scroller = root.current?.querySelector('.masthead-scroll');
    scroller?.addEventListener('scroll', position);
    return () => {
      window.removeEventListener('resize', position);
      scroller?.removeEventListener('scroll', position);
    };
  }, [open]);
  const activeUrls = useMemo(() => {
    const result = new Set<string>();
    for (const item of Object.values(categories).flat()) {
      const target = readQuery(new URL(item.url, window.location.origin));
      if (
        target.section === section &&
        Object.entries(target.filters).every(
          ([key, values]) =>
            values.length === filters[key]?.length &&
            values.every((value) => filters[key].includes(value)),
        )
      )
        result.add(item.url);
    }
    return result;
  }, [categories, filters, section]);
  const active = (url: string) => activeUrls.has(url);
  const navigate = (event: React.MouseEvent<HTMLAnchorElement>, url: string) => {
    if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
      event.preventDefault();
      close();
      onSection(url);
    }
  };
  const changeTheme = () => {
    const next = theme === 'dark' ? 'light' : 'dark';
    setTheme(next);
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem('mythos-theme', next);
    } catch {
      /* Storage is optional. */
    }
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute(
        'content',
        getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(),
      );
  };
  const submitSearch = (item?: (typeof suggestions)[number]) => {
    const value = search.trim();
    if (!value) return;
    if (item) {
      setSearch(item.title);
      setSearchOpen(false);
      onProduct(item.id);
      return;
    }
    const target = section || 'GamingLaptop';
    setSearchOpen(false);
    onSection(`/${target}?search=${encodeURIComponent(value)}`);
  };
  const children = open ? categories[open] || [] : [];
  return (
    <header
      className={`masthead ${hidden ? 'masthead-hidden' : ''}`}
      ref={root}
      onFocusCapture={() => setHidden(false)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          close();
          setSearchOpen(false);
        }
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          close();
          trigger.current?.focus();
        }
      }}
    >
      <div className="masthead-row">
        <a
          className="masthead-brand"
          href="/"
          aria-label="Mythos home"
          onClick={(event) => {
            if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
              event.preventDefault();
              close();
              setSearchOpen(false);
              onHome();
            }
          }}
        >
          <img src="/favicon.svg" alt="" width="30" height="30" />
          <span>
            mythos<b>.</b>
          </span>
        </a>
        <form
          className="masthead-search"
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            submitSearch(activeSuggestion >= 0 ? suggestions[activeSuggestion] : undefined);
          }}
        >
          <Search size={18} aria-hidden="true" />
          <input
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              setSearchOpen(true);
            }}
            onFocus={() => {
              setHidden(false);
              setOpen(null);
              setSearchOpen(true);
            }}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setActiveSuggestion((value) => Math.min(suggestions.length - 1, value + 1));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setActiveSuggestion((value) => Math.max(-1, value - 1));
              } else if (event.key === 'Escape') {
                setSearchOpen(false);
              }
            }}
            placeholder="Search products…"
            aria-label="Search the Mythos catalog"
            aria-autocomplete="list"
            aria-expanded={searchOpen && search.trim().length >= 2}
            aria-controls="masthead-search-results"
            aria-activedescendant={
              activeSuggestion >= 0 ? `search-result-${activeSuggestion}` : undefined
            }
          />
          {searchLoading && (
            <LoaderCircle className="masthead-search-spinner" size={16} aria-label="Searching" />
          )}
          {searchOpen && search.trim().length >= 2 && (
            <div className="masthead-search-panel" id="masthead-search-results" role="listbox">
              {suggestions.length ? (
                <>
                  <p>Products</p>
                  {suggestions.map((item, index) => (
                    <button
                      type="button"
                      id={`search-result-${index}`}
                      role="option"
                      aria-selected={activeSuggestion === index}
                      className={activeSuggestion === index ? 'active' : ''}
                      key={item.id}
                      onPointerDown={(event) => event.preventDefault()}
                      onClick={() => submitSearch(item)}
                    >
                      <span className="masthead-result-image">
                        {item.image && <img src={item.image} alt="" />}
                      </span>
                      <span className="masthead-result-copy">
                        <strong>{item.title}</strong>
                        <small>
                          {sectionLabel(item.section)} · {item.price.toLocaleString('en-TN')} DT
                        </small>
                      </span>
                      <ArrowUpRight size={15} />
                    </button>
                  ))}
                  <button
                    type="button"
                    className="masthead-search-all"
                    onClick={() => submitSearch()}
                  >
                    See results for “{search.trim()}” <ArrowUpRight size={15} />
                  </button>
                </>
              ) : searchError ? (
                <div className="masthead-search-empty">
                  <strong>Search is temporarily unavailable</strong>
                  <span>Please try again in a moment.</span>
                </div>
              ) : !searchLoading ? (
                <div className="masthead-search-empty">
                  <strong>No exact matches yet</strong>
                  <span>Try a model, brand, or component name.</span>
                </div>
              ) : null}
            </div>
          )}
        </form>
        <div className="masthead-actions">
          <button
            role="switch"
            aria-checked={theme === 'dark'}
            aria-label={theme === 'dark' ? 'Use light mode' : 'Use dark mode'}
            onClick={changeTheme}
          >
            {theme === 'dark' ? <Sun size={17} /> : <Moon size={17} />}
          </button>
          <button onClick={onAbout} aria-label="About Mythos">
            <Info size={17} />
          </button>
        </div>
      </div>
      <div className="masthead-category-row">
        <div className="masthead-scroll">
          <nav className="masthead-categories" aria-label="Product categories">
            {Object.entries(categories).map(([parent, items], index) => (
              <button
                key={parent}
                id={`masthead-trigger-${index}`}
                className={`masthead-trigger ${items.some((child) => active(child.url)) ? 'active' : ''}`}
                aria-expanded={open === parent}
                aria-controls={open === parent ? 'masthead-menu' : undefined}
                onPointerEnter={(event) => {
                  if (event.pointerType === 'mouse') {
                    cancelClose();
                    trigger.current = event.currentTarget;
                    setOpen(parent);
                  }
                }}
                onPointerLeave={(event) => {
                  if (event.pointerType === 'mouse') scheduleClose();
                }}
                onClick={(event) => {
                  trigger.current = event.currentTarget;
                  cancelClose();
                  const touch =
                    'pointerType' in event.nativeEvent && event.nativeEvent.pointerType === 'touch';
                  setOpen(open === parent && (event.detail === 0 || touch) ? null : parent);
                }}
                onKeyDown={(event) => {
                  if (event.key === 'ArrowDown') {
                    event.preventDefault();
                    trigger.current = event.currentTarget;
                    setOpen(parent);
                    requestAnimationFrame(() => panel.current?.querySelector('a')?.focus());
                  }
                }}
              >
                {parent}
                <ChevronDown size={13} />
              </button>
            ))}
          </nav>
          <a className="masthead-statistics-link" href="/statistics">Statistics <ArrowUpRight size={13} /></a>
        </div>
      </div>
      {open && (
        <div
          ref={panel}
          id="masthead-menu"
          className={`masthead-menu ${children.length > 6 ? 'masthead-menu-wide' : ''}`}
          style={{
            left,
            width: Math.min(720, Math.max(290, Math.ceil(children.length / 4) * 220)),
          }}
          aria-labelledby={trigger.current?.id}
          onPointerEnter={cancelClose}
          onPointerLeave={(event) => {
            if (event.pointerType === 'mouse') scheduleClose();
          }}
        >
          <div
            className="masthead-links"
            style={{
              gridTemplateRows: `repeat(${Math.min(children.length, 4)}, minmax(40px, auto))`,
            }}
          >
            {children.map((child) => (
              <a
                href={child.url}
                key={child.url}
                aria-current={active(child.url) ? 'page' : undefined}
                onClick={(event) => navigate(event, child.url)}
              >
                <span>{child.name}</span>
                <ArrowUpRight size={14} />
              </a>
            ))}
          </div>
        </div>
      )}
    </header>
  );
});
export function Footer({
  onSection,
  onAbout,
}: {
  onSection: (section: string) => void;
  onAbout: () => void;
}) {
  return (
    <footer className="site-footer">
      <div className="footer-main">
        <div className="footer-brand">
          <Brand />
          <p>Tech, in perspective.</p>
          <span>Find your next upgrade across Tunisian retailers.</span>
        </div>
        <div className="footer-links">
          <h2>Explore the catalog</h2>
          <a href="/statistics">Statistics <ArrowUpRight size={13} /></a>
          {['GamingLaptop', 'GPU', 'Monitor'].map((section) => (
            <a
              href={`/${section}`}
              key={section}
              onClick={(event) => {
                if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
                  event.preventDefault();
                  onSection(section);
                  window.scrollTo({ top: 0 });
                }
              }}
            >
              {sectionLabel(section)} <ArrowUpRight size={13} />
            </a>
          ))}
        </div>
        <div className="footer-about">
          <Cpu size={21} />
          <h2>A little clarity goes a long way.</h2>
          <p>
            Compare prices and specifications in one place. Purchase directly from the retailer you
            choose.
          </p>
          <button className="text-button" onClick={onAbout}>
            How Mythos works <ArrowUpRight size={14} />
          </button>
        </div>
      </div>
      <div className="footer-bottom">
        <span>© {new Date().getFullYear()} Mythos</span>
        <p>Prices and availability may change. Confirm details with the retailer.</p>
        <span>Tunisia / TND</span>
      </div>
    </footer>
  );
}
