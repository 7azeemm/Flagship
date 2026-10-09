import { useCallback, useEffect, useLayoutEffect, useMemo, useState } from 'react';
import { ArrowRight, Search, SlidersHorizontal, Store } from 'lucide-react';
import type { CatalogItem, Product } from './types';
import { useCatalog } from './hooks/useCatalog';
import { groupingEnabled, hasFilterChanges, money, sectionLabel } from './lib/catalog';
import { Footer, Header } from './components/Layout';
import { ActiveFilters, Filters } from './components/Filters';
import { ProductCard } from './components/ProductCard';
import { Dialog } from './components/Dialog';
import { Pagination } from './components/Pagination';
import { SortSelect } from './components/SortSelect';
import { HomePage } from './components/HomePage';
import { ProductPage } from './components/ProductPage';
import { StatisticsPage } from './components/StatisticsPage';
import { AuthPage, SettingsPage, VerifyEmailPage, WishlistPage } from './components/AccountPages';
import { AlertsPage } from './components/AlertsPage';
import { ActivityPage } from './components/ActivityPage';
import { InformationPage } from './components/InformationPages';
import './alerts.css';
import { routeMetadata } from './lib/metadata';

const authRoutes = {
  '/login': 'login',
  '/register': 'register',
  '/forgot-password': 'forgot',
  '/reset-password': 'reset',
} as const;
const accountRoutes: Record<string, React.ReactNode> = {
  '/verify-email': <VerifyEmailPage />,
  '/activity': <ActivityPage />,
  '/wishlist': <WishlistPage />,
  '/alerts': <AlertsPage />,
  '/settings': <SettingsPage />,
};

export default function App() {
  const [location, setLocation] = useState(() => window.location.pathname + window.location.search);
  useEffect(() => {
    if (!/^\/(product|variant)\//.test(window.location.pathname)) routeMetadata(window.location.pathname);
  }, [location]);
  const detailMatch = window.location.pathname.match(/^\/(product|variant)\/([^/]+)\/?$/);
  const standaloneDetail = !!detailMatch && !window.history.state?.detailBackground;
  const background = detailMatch ? window.history.state?.detailBackground || '/' : location;
  const pathname = new URL(background, window.location.origin).pathname;
  const setPathname = (_path: string) =>
    setLocation(window.location.pathname + window.location.search);
  useEffect(() => {
    const onLocationChange = () => setPathname(window.location.pathname);
    window.addEventListener('popstate', onLocationChange);
    return () => window.removeEventListener('popstate', onLocationChange);
  }, []);
  const isLegacySearch = pathname === '/search';
  const isHome = pathname === '/' || isLegacySearch;
  const routeAuthMode = authRoutes[pathname as keyof typeof authRoutes] || null;
  const showHome = isHome || !!routeAuthMode;
  const isStatistics = pathname === '/statistics';
  const accountPage = accountRoutes[pathname] || null;
  const informationPage =
    pathname === '/support' || pathname === '/future-plans' || pathname === '/legal'
      ? (pathname.slice(1) as 'support' | 'future-plans' | 'legal')
      : null;
  const productMatch = detailMatch;
  const productId = (() => {
    if (!productMatch) return null;
    try {
      return decodeURIComponent(productMatch[2]);
    } catch {
      return productMatch[2];
    }
  })();
  const {
    query,
    categories,
    catalogPage,
    baselineSectionInfo,
    loading,
    error,
    update,
    retry,
    changeSection,
    reset,
  } = useCatalog(
    !standaloneDetail &&
      !showHome &&
      !isStatistics &&
      !accountPage &&
      !informationPage,
    background,
  );
  const [signInOpen, setSignInOpen] = useState(false);
  const [mobileFilters, setMobileFilters] = useState(false);
  useLayoutEffect(() => {
    const openSignIn = () => setSignInOpen(true);
    window.addEventListener('flagship:signin', openSignIn);
    return () => window.removeEventListener('flagship:signin', openSignIn);
  }, []);
  useEffect(() => {
    if (!showHome && !productId && !isStatistics && !accountPage && !informationPage)
      document.title = `${sectionLabel(query.section)} — Flagship`;
  }, [query.section, showHome, productId, isStatistics, accountPage, informationPage]);
  useEffect(() => {
    if (!isHome && !isStatistics && !productId) {
      window.scrollTo({ top: 0, left: 0, behavior: 'auto' });
    }
  }, [query.section, query.page, isHome, isStatistics]);
  const fields = catalogPage?.section_info?.components || baselineSectionInfo?.components || [];
  const filterProps = {
    query,
    info: catalogPage?.section_info || null,
    baselineSectionInfo,
    update,
    reset,
    disabled: loading,
  };
  const entries = useMemo(
    () =>
      (catalogPage?.items || []).map((item) => {
        if (item.type === 'product')
          return { id: item.data.id, item, product: item.data, variant: undefined };
        const variant = item.data;
        const product: Product = {
          id: variant.id,
          title: variant.title,
          section: variant.section,
          image: variant.image,
          images: variant.images,
          price: variant.price,
          original_price: variant.original_price,
          components: variant.components,
          site: `${variant.retailer_count} retailers`,
          status: variant.in_stock_count ? 'InStock' : 'OutOfStock',
          url: '',
          description: null,
          history: { entries: [] },
          updated_at: null,
          added_at: null,
          views: variant.views,
          missing_group_fields: [],
        };
        return { id: variant.id, item, product, variant };
      }),
    [catalogPage?.items],
  );
  const catalogEmpty =
    !!baselineSectionInfo &&
    !hasFilterChanges(query) &&
    baselineSectionInfo.statuses.every((status) => status.count === 0);
  const chooseSection = useCallback(
    (section: string) => {
      changeSection(section);
      setPathname(window.location.pathname);
      setMobileFilters(false);
    },
    [changeSection],
  );
  const navigatePath = useCallback((path: string) => {
    const isDetail = /^\/(product|variant)\//.test(path);
    const detailBackground = /^\/(product|variant)\//.test(window.location.pathname)
      ? window.history.state?.detailBackground
      : window.location.pathname + window.location.search;
    window.history.pushState(isDetail && detailBackground ? { detailBackground } : null, '', path);
    window.dispatchEvent(new PopStateEvent('popstate'));
    if (!isDetail) window.scrollTo({ top: 0, behavior: 'auto' });
  }, []);
  useEffect(() => {
    if (isLegacySearch) {
      window.history.replaceState(null, '', '/');
      window.dispatchEvent(new PopStateEvent('popstate'));
    }
  }, [isLegacySearch]);
  useEffect(() => {
    // Ordinary detail links anywhere in the app retain the current page. Modified
    // clicks and new-tab links keep the browser's native standalone navigation.
    const onDetailLink = (event: MouseEvent) => {
      if (
        event.defaultPrevented ||
        event.button !== 0 ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey ||
        event.altKey
      )
        return;
      const link =
        event.target instanceof Element ? event.target.closest<HTMLAnchorElement>('a[href]') : null;
      if (!link || link.target || link.hasAttribute('download')) return;
      const url = new URL(link.href);
      if (
        url.origin !== window.location.origin ||
        !/^\/(product|variant)\/[^/]+\/?$/.test(url.pathname)
      )
        return;
      event.preventDefault();
      navigatePath(url.pathname + url.search + url.hash);
    };
    document.addEventListener('click', onDetailLink);
    return () => document.removeEventListener('click', onDetailLink);
  }, [navigatePath]);
  const openItem = useCallback(
    (item: CatalogItem) => {
      const selection =
        item.type === 'variant' && item.data.representative_product_id
          ? '?' + new URLSearchParams({ product: item.data.representative_product_id })
          : '';
      navigatePath('/' + item.type + '/' + encodeURIComponent(item.data.id) + selection);
    },
    [navigatePath],
  );
  const closeDetail = useCallback(() => {
    if (window.history.state?.detailBackground) window.history.back();
    else {
      window.history.replaceState(null, '', '/');
      window.dispatchEvent(new PopStateEvent('popstate'));
    }
  }, []);
  const chooseHome = useCallback(() => navigatePath('/'), [navigatePath]);
  const openProduct = useCallback(
    (id: string) => navigatePath(`/product/${encodeURIComponent(id)}`),
    [navigatePath],
  );
  const skipTarget =
    pathname === '/activity'
      ? '#activity-content'
      : informationPage
        ? '#information-content'
        : pathname === '/alerts'
          ? '#alerts-content'
          : accountPage
              ? '#account-content'
              : showHome
                ? '#home-content'
                : isStatistics
                  ? '#statistics-content'
                  : productId
                    ? '#product-content'
                    : '#catalog';
  const skipLabel =
    pathname === '/activity'
      ? 'catalog activity'
      : informationPage
        ? 'page content'
        : pathname === '/alerts'
          ? 'alerts'
          : accountPage
              ? 'account content'
              : showHome
                ? 'homepage'
                : isStatistics
                  ? 'statistics'
                  : productId
                    ? 'product details'
                    : 'catalog';

  return (
    <>
      <a href={skipTarget} className="skip-link">
        Skip to {skipLabel}
      </a>
      <Header
        categories={categories}
        section={
          showHome || isStatistics || productId || accountPage || informationPage
            ? ''
            : query.section
        }
        locationKey={location}
        filters={query.filters}
        onSection={chooseSection}
        onProduct={openProduct}
        onHome={chooseHome}
        onNavigate={navigatePath}
        onSignIn={() => setSignInOpen(true)}
      />
      {standaloneDetail ? (
        <main className="standalone-product-background" aria-label="Product page" />
      ) : informationPage ? (
        <InformationPage page={informationPage} />
      ) : accountPage ? (
        accountPage
      ) : showHome ? (
        <HomePage onNavigate={navigatePath} categories={categories} />
      ) : isStatistics ? (
        <StatisticsPage categories={categories} onNavigate={navigatePath} />
      ) : (
        <main className="main-container" id="catalog" tabIndex={-1}>
          <h1 className="sr-only">{sectionLabel(query.section)}</h1>
          <div className="catalog-layout">
            <aside className="desktop-filters" aria-label="Product filters">
              <Filters key={query.section} {...filterProps} />
            </aside>
            <section className="results-section" aria-label="Catalog results">
              <div className="results-toolbar compact-toolbar">
                <div className="toolbar-summary">
                  <div role="status" className="result-count">
                    {loading ? (
                      'Finding your next upgrade…'
                    ) : error ? (
                      'Catalog unavailable'
                    ) : (
                      <>
                        <strong>{money(catalogPage?.total_items ?? 0)}</strong>{' '}
                        {catalogPage?.total_items === 1 ? 'result' : 'results'}
                        {groupingEnabled(query, baselineSectionInfo) &&
                          catalogPage &&
                          catalogPage.total_items < catalogPage.total_products && (
                            <span> · {money(catalogPage.total_products)} listings</span>
                          )}
                      </>
                    )}
                  </div>
                  <ActiveFilters query={query} update={update} reset={reset} />
                </div>
                <div className="toolbar-controls">
                  <button
                    className="mobile-filter-button secondary-button"
                    onClick={() => setMobileFilters(true)}
                  >
                    <SlidersHorizontal size={16} /> Filters
                  </button>
                  <SortSelect value={query.sort} onChange={(sort) => update({ sort })} />
                  {!loading && !error && (
                    <Pagination
                      page={query.page}
                      total={catalogPage?.total_pages || 0}
                      onChange={(value) => update({ page: value })}
                    />
                  )}
                </div>
              </div>
              <div className="product-grid" aria-busy={loading}>
                {loading ? (
                  Array.from({ length: 6 }, (_, index) => (
                    <div key={index} className="card-skeleton" aria-hidden="true">
                      <div />
                      <i />
                      <i />
                      <span />
                      <section>
                        <i />
                        <i />
                        <i />
                        <i />
                      </section>
                    </div>
                  ))
                ) : error ? (
                  <div className="empty-state" role="alert">
                    <Store size={30} />
                    <h2>We couldn’t load the catalog.</h2>
                    <p>{error}</p>
                    <button className="primary-button" onClick={retry}>
                      Try again <ArrowRight size={16} />
                    </button>
                  </div>
                ) : !entries.length ? (
                  <div className="empty-state">
                    <Search size={30} />
                    <h2>
                      {catalogEmpty
                        ? 'The catalog is getting ready.'
                        : 'No matches. More possibilities.'}
                    </h2>
                    <p>
                      {catalogEmpty
                        ? 'No products have been loaded for this category yet.'
                        : 'Try a broader search or remove a filter to see more products.'}
                    </p>
                    {hasFilterChanges(query) && (
                      <button className="primary-button" onClick={reset}>
                        Clear filters <ArrowRight size={16} />
                      </button>
                    )}
                  </div>
                ) : (
                  entries.map((entry) => (
                    <ProductCard
                      key={entry.id}
                      product={entry.product}
                      variant={entry.variant}
                      fields={fields}
                      onOpen={() => void openItem(entry.item)}
                    />
                  ))
                )}
              </div>
              {!loading && !error && (
                <Pagination
                  page={query.page}
                  total={catalogPage?.total_pages || 0}
                  onChange={(value) => update({ page: value })}
                />
              )}
              <p className="results-note">Your next upgrade is a few good choices away.</p>
            </section>
          </div>
        </main>
      )}
      {!standaloneDetail && <Footer onSection={chooseSection} />}
      {productId && detailMatch && (
        <ProductPage
          computerSections={(categories.Computers || []).map((category) =>
            new URL(category.url, window.location.origin).pathname.replace(/^\/+|\/+$/g, ''),
          )}
          key={detailMatch[1] + productId}
          id={productId}
          kind={detailMatch[1] as 'product' | 'variant'}
          initialProductId={new URLSearchParams(window.location.search).get('product') || undefined}
          onClose={closeDetail}
        />
      )}
      {mobileFilters && (
        <Dialog
          title="Product filters"
          className="filters-dialog"
          onClose={() => setMobileFilters(false)}
        >
          <Filters {...filterProps} />
          <button
            className="primary-button apply-mobile-filters"
            onClick={() => setMobileFilters(false)}
          >
            Show results <ArrowRight size={16} />
          </button>
        </Dialog>
      )}
      {(routeAuthMode || signInOpen) && (
        <AuthPage
          key={routeAuthMode || 'header-login'}
          mode={routeAuthMode || 'login'}
          modal
          onClose={() => {
            setSignInOpen(false);
            if (routeAuthMode) navigatePath('/');
          }}
        />
      )}
    </>
  );
}
