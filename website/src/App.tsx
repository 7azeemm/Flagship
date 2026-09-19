import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowRight, ArrowUpRight, Search, SlidersHorizontal, Store } from 'lucide-react';
import type { CatalogItem, Product, VariantChart } from './types';
import { useCatalog } from './hooks/useCatalog';
import { groupingEnabled, hasFilterChanges, money, sectionLabel } from './lib/catalog';
import { Footer, Header } from './components/Layout';
import { ActiveFilters, Filters } from './components/Filters';
import { ProductCard } from './components/ProductCard';
import { ProductDialog } from './components/ProductDialog';
import { Dialog } from './components/Dialog';
import { parseChart, parseProduct, request } from './lib/api';
import { Pagination } from './components/Pagination';
import { SortSelect } from './components/SortSelect';
import { HomePage } from './components/HomePage';
import { ProductPage } from './components/ProductPage';
import { StatisticsPage } from './components/StatisticsPage';

export default function App() {
  const [pathname, setPathname] = useState(window.location.pathname);
  useEffect(() => {
    const onLocationChange = () => setPathname(window.location.pathname);
    window.addEventListener('popstate', onLocationChange);
    return () => window.removeEventListener('popstate', onLocationChange);
  }, []);
  const isHome = pathname === '/';
  const isStatistics = pathname === '/statistics';
  const productMatch = pathname.match(/^\/product\/([^/]+)\/?$/);
  const productId = productMatch ? decodeURIComponent(productMatch[1]) : null;
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
  } =
    useCatalog(!isHome && !productId && !isStatistics);
  const [selected, setSelected] = useState<{
    offers: Product[];
    chart: VariantChart | null;
    detailsEndpoint: string;
    initialProductId?: string;
  } | null>(null);
  const detailRequest = useRef<AbortController | null>(null);
  const [detailStatus, setDetailStatus] = useState('');
  useEffect(() => () => detailRequest.current?.abort(), []);
  const [about, setAbout] = useState(false);
  const [mobileFilters, setMobileFilters] = useState(false);
  useEffect(() => {
    if (!isHome && !productId && !isStatistics) document.title = `${sectionLabel(query.section)} — Mythos`;
    detailRequest.current?.abort();
    setDetailStatus('');
    setSelected(null);
  }, [query.section, isHome, productId, isStatistics]);
  useEffect(() => {
    if (!isHome && !isStatistics && !productId) {
      window.scrollTo({ top: 0, left: 0, behavior: 'auto' });
    }
  }, [query.section, query.page, isHome, isStatistics, productId]);
  const fields = catalogPage?.section_info?.components || baselineSectionInfo?.components || [];
  const filterProps = {
    query,
    info: catalogPage?.section_info || null,
    baselineSectionInfo,
    update,
    reset,
    disabled: loading,
  };
  const entries = useMemo(() => (catalogPage?.items || []).map((item) => {
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
  }), [catalogPage?.items]);
  const catalogEmpty =
    !!baselineSectionInfo &&
    !hasFilterChanges(query) &&
    baselineSectionInfo.statuses.every((status) => status.count === 0);
  const openItem = useCallback(async (item: CatalogItem) => {
    detailRequest.current?.abort();
    const controller = new AbortController();
    detailRequest.current = controller;
    setDetailStatus('');
    const endpoint =
      item.type === 'variant'
        ? `/variants/${encodeURIComponent(item.data.id)}`
        : `/products/${encodeURIComponent(item.data.id)}`;
    try {
      const raw = await request(endpoint, controller.signal);
      if (controller.signal.aborted) return;
      if (!raw || typeof raw !== 'object') throw new Error('Product details are unavailable.');
      const record = raw as Record<string, unknown>;
      const products =
        item.type === 'variant'
          ? Array.isArray(record.products)
            ? record.products.map(parseProduct)
            : []
          : record.product
            ? [parseProduct(record.product)]
            : [];
      if (!products.length) throw new Error('No retailer listings are available.');
      setSelected({
        offers: products,
        chart: parseChart(record.chart),
        detailsEndpoint: endpoint,
        initialProductId:
          item.type === 'variant' ? item.data.representative_product_id : item.data.id,
      });
      setDetailStatus('');
    } catch (error) {
      if (!controller.signal.aborted)
        setDetailStatus(
          error instanceof Error ? error.message : 'Product details could not be loaded.',
        );
    }
  }, []);
  const chooseSection = useCallback((section: string) => {
    changeSection(section);
    setPathname(window.location.pathname);
    setMobileFilters(false);
  }, [changeSection]);
  const navigatePath = useCallback((path: string) => {
    window.history.pushState(null, '', path);
    window.dispatchEvent(new PopStateEvent('popstate'));
    window.scrollTo({ top: 0, behavior: 'auto' });
  }, []);
  const chooseHome = useCallback(() => navigatePath('/'), [navigatePath]);
  const showAbout = useCallback(() => setAbout(true), []);
  const openProduct = useCallback(
    (id: string) => navigatePath(`/product/${encodeURIComponent(id)}`),
    [navigatePath],
  );

  return (
    <>
      <a
        href={isHome ? '#home-content' : isStatistics ? '#statistics-content' : productId ? '#product-content' : '#catalog'}
        className="skip-link"
      >
        Skip to {isHome ? 'homepage' : isStatistics ? 'statistics' : productId ? 'product details' : 'catalog'}
      </a>
      <Header
        categories={categories}
        section={isHome || isStatistics || productId ? '' : query.section}
        locationKey={pathname}
        filters={query.filters}
        onSection={chooseSection}
        onProduct={openProduct}
        onHome={chooseHome}
        onAbout={showAbout}
      />
      {isHome ? (
        <HomePage onNavigate={navigatePath} onAbout={showAbout} />
      ) : isStatistics ? (
        <StatisticsPage categories={categories} onNavigate={navigatePath} />
      ) : productId ? (
        <ProductPage
          id={productId}
          fields={baselineSectionInfo?.components || []}
          onNavigate={navigatePath}
        />
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
                        {groupingEnabled(query, baselineSectionInfo) && catalogPage && catalogPage.total_items < catalogPage.total_products && (
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
      <Footer onSection={chooseSection} onAbout={showAbout} />
      {detailStatus && (
        <div className="detail-status" role="status">
          {detailStatus}
          <button
            onClick={() => {
              detailRequest.current?.abort();
              setDetailStatus('');
            }}
          >
            Dismiss
          </button>
        </div>
      )}
      {selected && (
        <ProductDialog
          key={selected.detailsEndpoint}
          detailsEndpoint={selected.detailsEndpoint}
          offers={selected.offers}
          chart={selected.chart}
          fields={fields}
          initialProductId={selected.initialProductId}
          onClose={() => setSelected(null)}
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
      {about && (
        <Dialog title="About Mythos" className="about-dialog" onClose={() => setAbout(false)}>
          <span className="eyebrow">A CLEARER PERSPECTIVE</span>
          <h2>Good choices start with good information.</h2>
          <p>
            Mythos brings tech listings from Tunisian retailers into one searchable catalog, so you
            can explore specifications and compare prices.
          </p>
          <ol>
            <li>
              <strong>Find your category.</strong> Browse computers, components, peripherals, and
              more.
            </li>
            <li>
              <strong>Make it specific.</strong> Filter by your budget, preferred retailer, and the
              specifications that matter.
            </li>
            <li>
              <strong>Take a closer look.</strong> Review the details, then visit the retailer to
              confirm the offer and purchase.
            </li>
          </ol>
          <p className="muted">
            Listings can change between updates. Similar offers may have different configurations;
            verify the product details with the retailer.
          </p>
          <h3>How the Price Index works</h3>
          <p>
            For each interval, Mythos compares recorded prices for products observed at both ends,
            takes the median change, and chains those changes into an index starting at 100. A
            smaller set of comparable products makes the trend less representative.
          </p>
          <button className="primary-button" onClick={() => setAbout(false)}>
            Back to exploring <ArrowUpRight size={17} />
          </button>
        </Dialog>
      )}
    </>
  );
}
