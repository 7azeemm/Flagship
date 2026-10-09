import test from 'node:test';
import assert from 'node:assert/strict';
import {
  apiQueryParams,
  sectionInfoKey,
  clampRange,
  defaultQuery,
  defaultStockStatuses,
  configureCatalogDefaults,
  nodeStatus,
  productSpecs,
  queryParams,
  queryURL,
  readQuery,
  safeURL,
  toggleNode,
  pageNumbers,
} from './catalog';
import { parseChart, parsePage, parseProduct, parseServerInfo, parseDealEvidence } from './api';
import { DealContext } from '../components/DealContext';
import { RetailerHistoryChart } from '../components/RetailerHistoryChart';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Filters } from '../components/Filters';
import { SortSelect } from '../components/SortSelect';
import { ConfigurationMatches } from '../components/ConfigurationMatches';
import type { HomeProduct } from '../types';
import { ConfigurationStatistics } from '../components/ConfigurationStatistics';
import { parseOfferSummary } from './api';
import { parseComponentDetails } from './api';
import { componentDetailsTarget, componentSpecificationRows } from './componentDetails';
import { Specifications } from '../components/ProductCard';

test('component details use dataset IDs, infer GPU platform and expose only CPU/GPU actions', () => {
  const gpu = { id: 'RTX/GeForce RTX 4060/GeForce RTX 4060 8GB', label: 'GeForce RTX 4060 8GB' };
  for (const section of ['Laptop', 'GamingLaptop', 'MacBook'])
    assert.equal(componentDetailsTarget('gpu', gpu, section)?.platform, 'laptop');
  for (const section of ['PC', 'GamingPC', 'MiniPC', 'AllInOnePC', 'GPU'])
    assert.equal(componentDetailsTarget('gpu', gpu, section)?.platform, 'desktop');
  assert.equal(componentDetailsTarget('gpu', gpu, 'GamingLaptop')?.id, gpu.id);
  assert.equal(componentDetailsTarget('memory', gpu, 'PC'), null);
  assert.equal(componentDetailsTarget('cpu', 'Unknown CPU', 'PC'), null);
  const product = parseProduct({
    id: 'test',
    title: 'Test PC',
    price: 100,
    section: 'PC',
    components: {
      cpu: { id: 'Intel/Intel Core i3/Intel Core i3-10100', label: 'Intel Core i3-10100' },
      gpu,
      memory: '16GB',
    },
  });
  const markup = renderToStaticMarkup(
    createElement(Specifications, { product, fields: ['cpu', 'gpu', 'memory', 'storage'] }),
  );
  assert.equal((markup.match(/aria-haspopup="dialog"/g) || []).length, 2);
  assert.match(markup, /View CPU details for Intel Core i3-10100/);
  assert.match(markup, /View GPU details for GeForce RTX 4060 8GB/);
});

test('component specifications validate responses and format CPU/GPU units without losing zero values', () => {
  const cpu = parseComponentDetails({
    kind: 'CPU',
    id: 'cpu-id',
    label: 'CPU',
    specifications: { base_clock: '3.6', l1_cache: '256', l2_cache: '1' },
  });
  assert.deepEqual(
    componentSpecificationRows(cpu).map((row) => row.value),
    ['3.6 GHz', '256 KB', '1 MB'],
  );
  const gpu = parseComponentDetails({
    kind: 'GPU',
    id: 'gpu-id',
    label: 'GPU',
    specifications: { base_clock: '1545', memory_size: '8', rt_cores: '0', platform: 'laptop' },
  });
  assert.deepEqual(
    componentSpecificationRows(gpu).map((row) => row.value),
    ['1545 MHz', '8 GB', '0', 'Laptop'],
  );
  assert.throws(() => parseComponentDetails({ ...gpu, specifications: { cores: null } }));
  assert.throws(() => parseComponentDetails({ ...gpu, kind: 'Memory' }));
});

test('individual offer summaries count all listings, preserve unpriced stock and validate price totals', () => {
  const summary = {
    offer_count: 6,
    available_offers: 3,
    priced_offers: 3,
    available_priced_offers: 2,
    average_price: 200,
    retailer_count: 5,
    price_bands: [
      { from: 0, to: 200, count: 1 },
      { from: 200, to: null, count: 1 },
    ],
  };
  assert.deepEqual(parseOfferSummary(summary), summary);
  const markup = renderToStaticMarkup(
    createElement(ConfigurationStatistics, { data: summary, section: 'Memory', lowestPrice: 100 }),
  );
  assert.deepEqual(
    [...markup.matchAll(/<article><span>([^<]+)<\/span>/g)].map((match) => match[1]),
    ['Current lowest price', 'Average price', 'All products', 'In-stock products'],
  );
  for (const label of [
    'In-stock products',
    'All products',
    'Average price',
    'Current lowest price',
    'In-stock rate',
    'In-stock prices',
    '50.0%',
  ])
    assert.ok(markup.includes(label));
  for (const oldLabel of [
    'Matching configurations',
    'Across multiple retailers',
    'Average price gap',
    'Price gaps across retailers',
  ])
    assert.ok(!markup.includes(oldLabel));
  for (const patch of [
    { available_offers: 7 },
    { retailer_count: 0 },
    { priced_offers: 1 },
    { average_price: Infinity },
    { available_priced_offers: 3 },
    { price_bands: [{ from: 0, to: null, count: 1 }] },
  ])
    assert.throws(() => parseOfferSummary({ ...summary, ...patch }));
  const empty = {
    offer_count: 0,
    available_offers: 0,
    priced_offers: 0,
    available_priced_offers: 0,
    average_price: null,
    retailer_count: 0,
    price_bands: [],
  };
  assert.deepEqual(parseOfferSummary(empty), empty);
  assert.equal(parseOfferSummary(undefined), null);
  const unavailable = { ...empty, offer_count: 2, retailer_count: 1 };
  assert.ok(
    renderToStaticMarkup(
      createElement(ConfigurationStatistics, {
        data: unavailable,
        section: 'All',
        lowestPrice: null,
      }),
    ).includes('No matching in-stock offers with a listed price'),
  );
});

test('configuration result links use default availability and preserve the retailer and specification IDs', () => {
  const previousDefaults = defaultQuery();
  configureCatalogDefaults({
    stock: [...defaultStockStatuses],
    section: 'GamingLaptop',
    sort: 'price_asc',
  });
  try {
    const props = {
      products: [
        {
          id: 'matching-product',
          section: 'GamingPC',
          site: 'Tunisianet',
          title: 'Matching PC',
          image: '',
          price: 2499,
          status: 'InStock',
        } as HomeProduct,
      ],
      section: 'GamingPC',
      retailer: 'Tunisianet',
      filters: { gpu: 'GeForce RTX 5060 8GB', memory_size: '16 GB' },
      catalogFilters: { gpu: ['Nvidia/5060/8GB'], memory_size: ['16 GB'] },
      onNavigate: () => {},
    };
    const html = renderToStaticMarkup(createElement(ConfigurationMatches, props));
    const href = html.match(/href="([^"]+)"[^>]*>View all matches/)?.[1]?.replaceAll('&amp;', '&');
    assert.ok(href);
    const query = readQuery(new URL(href, 'http://localhost'));
    assert.deepEqual(query.filters, props.catalogFilters);
    assert.deepEqual(query.site, ['Tunisianet']);
    assert.equal(query.grouped, false);
    assert.deepEqual(query.stock, [...defaultStockStatuses]);
    assert.equal(new URL(href, 'http://localhost').searchParams.has('availability'), false);
    assert.deepEqual(readQuery(new URL(queryURL(query), 'http://localhost')).stock, [
      ...defaultStockStatuses,
    ]);
    assert.match(html, /href="\/product\/matching-product"/);
    assert.doesNotMatch(
      renderToStaticMarkup(createElement(ConfigurationMatches, { ...props, catalogFilters: null })),
      /View all matches/,
    );
    assert.doesNotMatch(
      renderToStaticMarkup(createElement(ConfigurationMatches, { ...props, section: 'All' })),
      /View all matches/,
    );
  } finally {
    configureCatalogDefaults({
      stock: previousDefaults.stock,
      section: previousDefaults.section,
      sort: previousDefaults.sort,
    });
  }
});

test('catalog links default to available statuses before metadata arrives and with legacy metadata', () => {
  assert.deepEqual(defaultQuery('Memory').stock, [...defaultStockStatuses]);
  const withoutStatus = readQuery(
    new URL('https://flagship.test/Memory?filters=%7B%22memory_size%22%3A%5B%2232GB%22%5D%7D'),
  );
  assert.deepEqual(withoutStatus.stock, [...defaultStockStatuses]);
  assert.deepEqual(readQuery(new URL(queryURL(withoutStatus), 'https://flagship.test')).stock, [
    ...defaultStockStatuses,
  ]);
  assert.deepEqual(
    parseServerInfo({ categories: { Components: [{ name: 'Memory', url: '/Memory' }] } }).defaults
      .stock,
    [...defaultStockStatuses],
  );
});

test('newest sorting survives shared URLs and API requests and has a visible label', () => {
  const query = readQuery({ pathname: '/Smartphone', search: '?sort=newest&grouping_mode=true' });
  assert.equal(query.sort, 'newest');
  assert.equal(queryParams(query).get('sort'), 'newest');
  assert.equal(apiQueryParams(query, false).get('sort'), 'newest');
  assert.match(
    renderToStaticMarkup(createElement(SortSelect, { value: query.sort, onChange: () => {} })),
    /Newest listings/,
  );
});

test('catalog defaults come from the server and explicit availability survives shared URLs', () => {
  const info = parseServerInfo({
    categories: { Computers: [{ name: 'Laptops', url: '/Laptop' }] },
    defaults: { stock: ['InStock', 'Arriving', 'OnDemand'], section: 'Laptop', sort: 'price_asc' },
  });
  configureCatalogDefaults(info.defaults);
  try {
    assert.deepEqual(readQuery(new URL('https://flagship.test/')).stock, info.defaults.stock);
    assert.equal(defaultQuery().section, 'Laptop');
    assert.deepEqual(readQuery(new URL('https://flagship.test/Laptop?availability=all')).stock, []);
    assert.deepEqual(readQuery(new URL('https://flagship.test/Laptop?stock=OutOfStock')).stock, [
      'OutOfStock',
    ]);
    assert.deepEqual(
      readQuery(new URL(queryURL(defaultQuery()), 'https://flagship.test')).stock,
      info.defaults.stock,
    );
  } finally {
    configureCatalogDefaults({ stock: [], section: '', sort: 'price_asc' });
  }
});

test('retailer chart retains unknown legacy history and gaps for missing offers', () => {
  const base = {
    timestamp: '2026-09-01T00:00:00Z',
    lowest_price: 100,
    lowest_price_retailer: 'Shop',
    offer_count: 1,
    in_stock_count: 1,
    retailer_count: 1,
  };
  const chart = parseChart({
    range: 'all',
    interval: 'scan',
    scope: 'variant',
    currency: 'TND',
    summary: {
      current_price: 100,
      period_low: 100,
      period_high: 100,
      all_time_low: 100,
      average_price: 100,
      ninety_day_average: 100,
    },
    points: [
      base,
      {
        ...base,
        timestamp: '2026-09-02T00:00:00Z',
        retailer_prices: { Shop: { InStock: 100, OutOfStock: 'invalid' } },
        status_counts: { InStock: 1 },
      },
      {
        ...base,
        timestamp: '2026-09-03T00:00:00Z',
        lowest_price: null,
        retailer_prices: {},
        status_counts: {},
      },
    ],
  });
  assert.ok(chart);
  assert.equal(chart.points[0].retailer_prices, null);
  assert.deepEqual(chart.points[1].retailer_prices, { Shop: { InStock: 100 } });
  assert.deepEqual(chart.points[2].retailer_prices, {});
  const html = renderToStaticMarkup(createElement(RetailerHistoryChart, { points: chart.points }));
  assert.match(html, /Lowest in stock/);
  assert.doesNotMatch(html, /<select|Price to show/);
  assert.doesNotMatch(html, /NaN|Infinity/);
  const empty = renderToStaticMarkup(createElement(RetailerHistoryChart, { points: [] }));
  assert.match(empty, /No recorded/);
  assert.doesNotMatch(empty, /NaN|Infinity/);
});

test('URLs preserve nested filters, retailers, sorting, grouping and pagination', () => {
  const query = {
    ...defaultQuery('GamingLaptop'),
    search: 'RTX & OLED',
    min_price: '1500',
    max_price: '4000',
    site: ['Mytek', 'Tunisianet'],
    stock: ['Arriving'],
    filters: { cpu: ['Intel/Core i7'], memory_size: ['16GB'] },
    sort: 'price_desc' as const,
    grouped: true,
    page: 3,
  };
  const url = new URL(queryURL(query), 'https://flagship.test');
  assert.deepEqual(readQuery(url), query);
  assert.deepEqual(queryParams(query).getAll('site'), query.site);
  assert.equal(queryParams(query, true).get('send_section_info'), 'true');
});
test('legacy lowercase URLs are recognized and invalid parameters are bounded', () => {
  assert.equal(readQuery({ pathname: '/gaminglaptop', search: '' }).section, 'GamingLaptop');
  const query = readQuery({
    pathname: '/CPU',
    search: '?page=-5&min_price=-2&max_price=999999999999&filters=null',
  });
  assert.equal(query.page, 1);
  assert.equal(query.min_price, '');
  assert.equal(query.max_price, '');
  assert.deepEqual(query.filters, {});
  assert.deepEqual(readQuery({ pathname: '/CPU', search: '?filters={broken' }).filters, {});
  assert.deepEqual(
    readQuery({
      pathname: '/CPU',
      search: '?filters=%7B%22cpu%22%3A%5B%22Intel%2FCore+i7%22%5D%7D',
    }).filters,
    { cpu: ['Intel/Core i7'] },
  );
});
test('parent selections replace descendants without matching unrelated prefixes', () => {
  const coreI7 = { id: 'Intel/Core i7', label: 'Core i7', count: 2 };
  const intel = { id: 'Intel', label: 'Intel', count: 3, children: [coreI7] };
  assert.deepEqual(toggleNode(['Intel/Core i7', 'AMD'], intel), ['AMD', 'Intel']);
  assert.deepEqual(toggleNode(['Intel', 'IntelOther'], intel), ['IntelOther']);
  assert.deepEqual(toggleNode(['Intel'], coreI7), ['Intel', '!Intel/Core i7']);
  assert.equal(nodeStatus(intel, ['Intel/Core i7']), 'partial');
  assert.equal(nodeStatus(coreI7, ['Intel']), 'checked');
  const connector = { label: 'USB-A/USB-C', count: 1 };
  assert.deepEqual(toggleNode(['USB-A'], connector), ['USB-A', 'USB-A/USB-C']);
  assert.equal(nodeStatus(connector, ['USB-A']), 'unchecked');
});
test('price ranges handle equal bounds and cannot cross or go out of bounds', () => {
  assert.deepEqual(clampRange(500, 100, 0, 1000), [500, 500]);
  assert.deepEqual(clampRange(-10, 3000, 100, 2000), [100, 2000]);
  assert.deepEqual(clampRange(0, 100, 500, 500), [500, 500]);
  assert.deepEqual(clampRange(NaN, NaN, 100, 1000), [100, 1000]);
});
test('API original_price maps correctly and missing availability is not fabricated', () => {
  const product = parseProduct({
    id: 'a',
    title: 'Laptop',
    price: 1500,
    original_price: 1800,
    components: {
      cpu: { id: 'Intel/Core i5/Intel Core i5-14400F', label: 'Intel Core i5-14400F' },
      memory: '16GB',
      invalid: 4,
    },
  });
  assert.equal(product.original_price, 1800);
  assert.equal(product.status, 'Unknown');
  assert.equal(product.image, '');
  assert.deepEqual(
    productSpecs(product, ['memory', 'cpu', 'gpu']).map((spec) => spec.value),
    ['16GB', 'Intel Core i5-14400F', 'Unknown'],
  );
  assert.throws(() => parseProduct({ title: 'bad', price: 'NaN' }));
});
test('product detail responses retain the retailer name', () => {
  assert.equal(
    parseProduct({ id: 'a', title: 'Laptop', price: 1500, retailer: 'Mytek' }).site,
    'Mytek',
  );
});
test('product detail responses accept the backend history array', () => {
  const product = parseProduct({
    id: 'a',
    title: 'Laptop',
    price: 1500,
    retailer: 'Mytek',
    history: [
      {
        timestamp: '2026-09-16T10:30:00Z',
        changes: [{ field: 'price', old_value: 1600, new_value: 1500 }],
      },
    ],
  });
  assert.equal(product.history.entries.length, 1);
  assert.deepEqual(product.history.entries[0].changes[0], {
    field: 'price',
    old_value: 1600,
    new_value: 1500,
  });
});
test('response validation rejects malformed data and accepts empty catalogs', () => {
  assert.throws(() => parsePage({ products: [], total_pages: '1' }));
  assert.throws(() => parseServerInfo({ categories: { Components: [123] } }));
  const categories = {
    Computers: [{ id: 'Laptop', name: 'Laptops', url: '/Laptop' }],
    Components: [{ id: 'CPU', name: 'Processors', url: '/CPU' }],
  };
  assert.deepEqual(parseServerInfo({ categories }).categories, categories);
  assert.deepEqual(Object.keys(parseServerInfo({ categories }).categories), [
    'Computers',
    'Components',
  ]);
  assert.throws(() =>
    parseServerInfo({
      categories: {
        Components: [
          { id: 'CPU', name: 'CPU' },
          { id: 'CPU', name: 'Duplicate' },
        ],
      },
    }),
  );
  assert.equal(
    parsePage({
      items: [],
      total_items: 0,
      total_products: 0,
      total_pages: 0,
      page: 1,
      section_info: null,
    }).total_products,
    0,
  );
  const sectionInfo = parsePage({
    items: [],
    total_items: 0,
    total_products: 0,
    total_pages: 0,
    page: 1,
    section_info: {
      filters: [],
      sites: [
        { value: 'Mytek', count: 8 },
        { value: 'Empty shop', count: 0 },
      ],
      statuses: [
        { value: 'InStock', count: 6 },
        { value: 'OutOfStock', count: 2 },
        { value: 'Arriving', count: 0 },
        { value: 'OnDemand', count: 0 },
      ],
      components: ['cpu'],
      group_by: [],
      group_by_default: false,
      min_price: 100,
      max_price: 1000,
    },
  }).section_info;
  assert.equal(sectionInfo?.sites[1].count, 0);
  assert.equal(sectionInfo?.statuses[0].value, 'InStock');
  assert.throws(() =>
    parsePage({
      items: [],
      total_items: 0,
      total_products: 0,
      total_pages: 0,
      page: 1,
      section_info: {
        filters: [],
        sites: ['Mytek'],
        statuses: [],
        components: [],
        group_by: [],
        group_by_default: false,
        min_price: 0,
        max_price: 0,
      },
    }),
  );
});

test('retailer and availability counts render, including zero-count retailers', () => {
  const info = {
    filters: [],
    sites: [
      { value: 'Empty shop', count: 0 },
      { value: 'Mytek', count: 8 },
    ],
    statuses: [
      { value: 'InStock', count: 6 },
      { value: 'OutOfStock', count: 2 },
      { value: 'Arriving', count: 0 },
      { value: 'OnDemand', count: 0 },
    ],
    components: [],
    group_by: [],
    group_by_default: false,
    min_price: 100,
    max_price: 1000,
  };
  const html = renderToStaticMarkup(
    createElement(Filters, {
      query: defaultQuery(),
      info,
      baselineSectionInfo: info,
      update: () => {},
      reset: () => {},
      disabled: false,
    }),
  );
  assert.match(html, /In stock<\/span><small>6<\/small>/);
  assert.match(html, /Empty shop<\/span><small>0<\/small>/);
  assert.match(html, /Mytek<\/span><small>8<\/small>/);
  assert.ok(html.indexOf('Mytek') < html.indexOf('Empty shop'));
});
test('section grouping defaults select optional fields without locking them', () => {
  const info = {
    filters: [],
    sites: [],
    statuses: [],
    components: [],
    group_by_default: true,
    group_by: [
      { key: 'cpu', required: true, default: false },
      { key: 'gpu', required: false, default: true },
      { key: 'memory_size', required: false, default: false },
    ],
    min_price: 0,
    max_price: 0,
  };
  const html = renderToStaticMarkup(
    createElement(Filters, {
      query: defaultQuery('PC'),
      info,
      baselineSectionInfo: info,
      update: () => {},
      reset: () => {},
      disabled: false,
    }),
  );
  assert.match(html, /role="switch" aria-checked="true"/);
  assert.match(html, /aria-pressed="true" disabled="" title="Required for grouping"/);
  assert.match(html, /aria-pressed="true" title="Match by GPU"/);
  assert.match(html, /aria-pressed="false" title="Match by RAM size"/);
});
test('external links reject executable or relative URLs', () => {
  assert.equal(safeURL('javascript:alert(1)'), undefined);
  assert.equal(safeURL('data:text/html,hello'), undefined);
  assert.equal(safeURL('//example.com'), undefined);
  assert.equal(safeURL('https://example.com/product'), 'https://example.com/product');
});

test('configured components are authoritative, including missing and blank values', () => {
  const product = parseProduct({
    id: 'a',
    title: 'CPU',
    price: 100,
    components: { cpu: '  ', gpu: 'Extra value' },
  });
  assert.deepEqual(productSpecs(product, ['cpu']), [
    { key: 'cpu', label: 'CPU', value: 'Unknown', unknown: true },
  ]);
  assert.deepEqual(productSpecs(product, []), []);
});

test('pagination shows a useful window at the start, middle and end', () => {
  assert.deepEqual(pageNumbers(1, 20), [1, 2, 3, 4, 20]);
  assert.deepEqual(pageNumbers(10, 20), [1, 7, 8, 9, 10, 11, 12, 13, 20]);
  assert.deepEqual(pageNumbers(20, 20), [1, 17, 18, 19, 20]);
  assert.deepEqual(pageNumbers(1, 2), [1, 2]);
  assert.deepEqual(pageNumbers(1, 0), []);
});

test('metadata identity ignores pagination, sorting and grouping but tracks selections', () => {
  const query = defaultQuery();
  assert.equal(
    sectionInfoKey(query),
    sectionInfoKey({ ...query, page: 4, sort: 'price_desc', grouped: true }),
  );
  for (const changes of [
    { section: 'CPU' },
    { search: 'intel' },
    { site: ['Mytek'] },
    { stock: ['OutOfStock'] },
    { min_price: '100' },
    { filters: { cpu: ['Intel'] } },
  ])
    assert.notEqual(sectionInfoKey(query), sectionInfoKey({ ...query, ...changes }));
  assert.equal(apiQueryParams(query, false).has('send_section_info'), false);
  assert.deepEqual(
    apiQueryParams({ ...query, grouped: true, groupBy: ['cpu', 'gpu'] }, false).getAll('group_by'),
    ['cpu', 'gpu'],
  );
  assert.deepEqual(
    apiQueryParams({ ...query, grouped: true, groupBy: null }, false).getAll('group_by'),
    [],
  );
});
test('grouped API summaries keep the joined-fields label and gallery images', () => {
  const page = parsePage({
    section_info: null,
    page: 1,
    total_pages: 1,
    total_items: 1,
    total_products: 2,
    items: [
      {
        type: 'variant',
        data: {
          id: 'Intel Core i5 | 16 GB',
          representative_product_id: 'offer-b',
          section: 'CPU',
          image: 'https://example.com/one.jpg',
          images: ['https://example.com/two.jpg'],
          price: 120,
          offer_count: 2,
          retailer_count: 2,
          in_stock_count: 1,
          components: { cpu: 'Intel Core' },
        },
      },
    ],
  });
  assert.equal(page.items[0].type, 'variant');
  if (page.items[0].type === 'variant')
    assert.equal(page.items[0].data.id, 'Intel Core i5 | 16 GB');
  if (page.items[0].type === 'variant')
    assert.deepEqual(page.items[0].data.images, ['https://example.com/two.jpg']);
  if (page.items[0].type === 'variant')
    assert.equal(page.items[0].data.representative_product_id, 'offer-b');
});
import { buildProductHistoryChart } from './productHistory';
test('listing history utility reconstructs observations without variant history', () => {
  const product = parseProduct({
    id: 'standalone',
    title: 'Dell',
    price: 1425,
    status: 'InStock',
    added_at: '2026-09-01T00:00:00Z',
    history: null,
  });
  const chart = buildProductHistoryChart(
    product,
    '90d',
    'auto',
    Date.parse('2026-09-07T00:00:00Z'),
  );
  assert.equal(chart.scope, 'product');
  assert.equal(chart.points.length, 2);
  assert.deepEqual(
    chart.points.map((p) => p.lowest_price),
    [1425, 1425],
  );
  assert.deepEqual(
    chart.points.map((p) => p.in_stock_count),
    [1, 1],
  );
});
test('individual charts reconstruct price changes and respect period and interval', () => {
  const product = parseProduct({
    id: 'p',
    title: 'Test',
    price: 100,
    status: 'OutOfStock',
    added_at: '2026-01-01T00:00:00Z',
    history: {
      entries: [
        {
          timestamp: '2026-09-02T00:00:00Z',
          changes: [{ field: 'price', old_value: 150, new_value: 100 }],
        },
        {
          timestamp: '2026-09-03T00:00:00Z',
          changes: [{ field: 'status', old_value: 'InStock', new_value: 'OutOfStock' }],
        },
      ],
    },
  });
  const chart = buildProductHistoryChart(
    product,
    '30d',
    'scan',
    Date.parse('2026-09-07T00:00:00Z'),
  );
  assert.equal(chart.points[0].timestamp, '2026-08-08T00:00:00.000Z');
  assert.equal(chart.points[0].lowest_price, 150);
  assert.equal(chart.points.at(-1)?.in_stock_count, 0);
  assert.equal(chart.summary.period_low, 100);
  assert.equal(buildProductHistoryChart(product, 'all', 'week').interval, 'week');
});
import { ProductAnalytics } from '../components/ProductAnalytics';
import { ProductDialog } from '../components/ProductDialog';

test('product details hide empty components and show listings only for multi-product variants', () => {
  const product = parseProduct({
    id: 'a',
    title: 'Product A',
    site: 'Shop A',
    price: 120,
    status: 'InStock',
    components: {},
  });
  const render = (offers: (typeof product)[], detailsEndpoint: string) =>
    renderToStaticMarkup(
      createElement(ProductDialog, {
        offers,
        chart: null,
        fields: ['cpu'],
        detailsEndpoint,
        onClose: () => {},
      }),
    );
  const single = render([product], '/products/a');
  assert.doesNotMatch(single, /<h2>Components<\/h2>/);
  assert.doesNotMatch(single, /detail-listings/);
  assert.doesNotMatch(single, /detail-analytics|<svg[^>]*aria-labelledby/);
  assert.doesNotMatch(render([product], '/variants/a'), /detail-listings/);
  const grouped = render(
    [product, { ...product, id: 'b', title: 'Product B', site: 'Shop B' }],
    '/variants/a',
  );
  assert.match(grouped, /detail-listings/);
  assert.match(grouped, /<b>Shop B<\/b> · Product B/);
  const previewed = renderToStaticMarkup(
    createElement(ProductDialog, {
      offers: [product, { ...product, id: 'b', title: 'Product B', site: 'Shop B', price: 200 }],
      chart: null,
      fields: [],
      detailsEndpoint: '/variants/a',
      initialProductId: 'b',
      onClose: () => {},
    }),
  );
  assert.match(previewed, /<h2 id="product-overview-title" tabindex="-1">Product B<\/h2>/);
  assert.match(
    render([{ ...product, components: { cpu: 'Core i5' } }], '/products/a'),
    /<h2>Components<\/h2>/,
  );
});
test('product history groups one update and renders every typed field change', () => {
  const product = parseProduct({
    id: 'history-product',
    title: 'History product',
    site: 'Shop',
    price: 120,
    status: 'OutOfStock',
    history: {
      entries: [
        {
          timestamp: '2026-09-15T10:30:00Z',
          changes: [
            { field: 'price', old_value: 150, new_value: 120 },
            { field: 'status', old_value: 'InStock', new_value: 'OutOfStock' },
          ],
        },
      ],
    },
  });
  const html = renderToStaticMarkup(
    createElement(ProductDialog, {
      offers: [product],
      chart: null,
      fields: [],
      detailsEndpoint: '/products/history-product',
      onClose: () => {},
    }),
  );
  assert.match(html, /1 recorded updates · 2 changes/);
  assert.match(html, /2 fields changed/);
  assert.match(html, /dateTime="2026-09-15T10:30:00Z"/);
  assert.match(html, /150 DT/);
  assert.match(html, /Out of stock/);
});
test('product history renders structured component labels instead of object placeholders', () => {
  const product = parseProduct({
    id: 'component-history',
    title: 'Component history',
    site: 'Shop',
    price: 120,
    history: [
      {
        timestamp: '2026-09-15T10:30:00Z',
        changes: [
          {
            field: 'components',
            old_value: { cpu: { id: 'Others/Intel/Intel Core i5', label: 'Intel Core i5' } },
            new_value: {
              cpu: { id: 'Intel/Core i5/Intel Core i5-12400', label: 'Intel Core i5-12400' },
            },
          },
        ],
      },
    ],
  });
  const html = renderToStaticMarkup(
    createElement(ProductDialog, {
      offers: [product],
      chart: null,
      fields: [],
      detailsEndpoint: '/products/component-history',
      onClose: () => {},
    }),
  );
  assert.match(html, /CPU: Intel Core i5/);
  assert.match(html, /CPU: Intel Core i5-12400/);
  assert.doesNotMatch(html, /\[object Object\]/);
});
test('product analytics keeps separate compact price and availability charts', () => {
  const product = parseProduct({
    id: 'dell',
    title: 'Dell',
    price: 1425,
    status: 'InStock',
    added_at: '2026-09-01T00:00:00Z',
  });
  const chart = buildProductHistoryChart(
    product,
    'all',
    'scan',
    Date.parse('2026-09-07T00:00:00Z'),
  );
  const html = renderToStaticMarkup(
    createElement(ProductAnalytics, { initialChart: chart, endpoint: '/products/dell' }),
  );
  assert.equal((html.match(/class="simple-history-svg"/g) || []).length, 2);
  assert.match(html, /Availability/);
  assert.doesNotMatch(html, /type="range"|history-readout/);
  assert.match(html, /Listed price/);
  assert.doesNotMatch(html, /NaN|Infinity|Visible chart lines|Retailer offer status/);
});

test('detail description precedes every populated component, including fields absent from catalog metadata', () => {
  const product = parseProduct({
    id: 'full',
    title: 'Complete listing',
    price: 100,
    description: 'Detailed retailer description',
    components: { cpu: 'Core i5', warranty: 'Two years', blank: '' },
  });
  const html = renderToStaticMarkup(
    createElement(ProductDialog, {
      offers: [product],
      chart: null,
      fields: ['cpu'],
      detailsEndpoint: '/products/full',
      onClose: () => {},
    }),
  );
  assert.match(html, /Two years/);
  assert.doesNotMatch(html, />Blank</);
  assert.ok(html.indexOf('Detailed retailer description') < html.indexOf('<h2>Components</h2>'));
});

test('one scan changing both price and availability yields one coherent chart observation', () => {
  const product = parseProduct({
    id: 'atomic',
    title: 'Atomic',
    price: 100,
    status: 'InStock',
    added_at: '2026-01-01T00:00:00Z',
    history: [
      {
        timestamp: '2026-09-02T00:00:00Z',
        changes: [
          { field: 'price', old_value: 150, new_value: 100 },
          { field: 'status', old_value: 'OutOfStock', new_value: 'InStock' },
        ],
      },
    ],
  });
  const chart = buildProductHistoryChart(
    product,
    'all',
    'scan',
    Date.parse('2026-09-07T00:00:00Z'),
  );
  assert.deepEqual(
    chart.points.map((p) => [p.lowest_price, p.in_stock_count]),
    [
      [150, 0],
      [100, 1],
      [100, 1],
    ],
  );
  assert.equal(chart.summary.ninety_day_average, 116);
});

test('file-based categories preserve filtered links sharing a section', () => {
  const info = parseServerInfo({
    Components: [
      { name: 'Laptop Memory', url: 'Memory?filters=%7B%22platform%22%3A%5B%22Laptop%22%5D%7D' },
      { name: 'Desktop Memory', url: 'Memory?filters=%7B%22platform%22%3A%5B%22Desktop%22%5D%7D' },
    ],
  });
  assert.equal(info.categories.Components.length, 2);
  assert.equal(info.categories.Components[0].id, 'Memory');
  assert.deepEqual(
    readQuery(new URL(info.categories.Components[0].url, 'https://flagship.test')).filters,
    { platform: ['Laptop'] },
  );
  assert.deepEqual(
    readQuery(new URL(info.categories.Components[1].url, 'https://flagship.test')).filters,
    { platform: ['Desktop'] },
  );
  assert.throws(() => parseServerInfo({ Bad: [{ name: 'External', url: '//example.com' }] }));
  assert.throws(() => parseServerInfo({ Bad: [{ name: 'Script', url: 'javascript:alert(1)' }] }));
});

test('category search and grouping defaults survive shared URLs', () => {
  assert.equal(readQuery({ pathname: '/GPU', search: '' }).grouped, null);
  assert.equal(queryParams(defaultQuery('GPU')).has('grouping_mode'), false);
  const query = { ...defaultQuery('GPU'), grouped: false, search: 'RTX 5070' };
  assert.deepEqual(readQuery(new URL(queryURL(query), 'https://flagship.test')), query);
  assert.equal(apiQueryParams(query, false).get('search'), 'RTX 5070');
  assert.equal(apiQueryParams(query, false).get('grouping_mode'), 'false');
});

test('discount sorting survives shared URLs and API requests', () => {
  const query = readQuery({
    pathname: '/GamingLaptop',
    search: '?sort=discount_desc&stock=InStock&grouping_mode=false',
  });
  assert.equal(query.sort, 'discount_desc');
  assert.equal(queryParams(query).get('sort'), 'discount_desc');
  assert.equal(apiQueryParams(query, false).get('sort'), 'discount_desc');
});

test('chart configuration, price context and period average remain visible without a retailer selector', () => {
  const chart = parseChart({
    scope: 'variant',
    range: '90d',
    interval: 'day',
    currency: 'TND',
    combination_fields: { cpu: 'Core i7', memory_size: '16 GB' },
    points: [],
    summary: {
      current_price: 80,
      period_low: 80,
      period_high: 120,
      all_time_low: 80,
      average_price: 100,
      ninety_day_average: 100,
    },
  });
  assert.ok(chart);
  const html = renderToStaticMarkup(
    createElement(ProductAnalytics, { initialChart: chart, endpoint: '/variants/test' }),
  );
  assert.match(html, /Compared configuration/);
  assert.match(html, /Core i7/);
  assert.match(html, /16 GB/);
  assert.doesNotMatch(html, /Great price/);
  assert.match(html, /90-day average/);
  assert.doesNotMatch(html, /<select/);
});

test('Flagship deals sort is preserved in shared links and API queries', () => {
  const query = readQuery({
    pathname: '/GamingPC',
    search: '?sort=best_deals&grouping_mode=false',
  });
  assert.equal(query.sort, 'best_deals');
  assert.equal(apiQueryParams(query, false).get('sort'), 'best_deals');
});
test('deal evidence distinguishes historical lows from competing retailer prices', () => {
  const evidence = parseDealEvidence({
    new_low: true,
    recent_drop: true,
    enough_history: true,
    typical_price: 2500,
    history_saving_percent: 20,
    next_best_price: 2400,
    next_best_retailer: 'Shop B',
    retailer_count: 3,
    gap_percent: 100 / 6,
    comparison_fields: { model: 'Phone A', memory_size: '8 GB' },
  });
  assert.ok(evidence);
  const html = renderToStaticMarkup(createElement(DealContext, { evidence }));
  assert.match(html, /New 30-day low/);
  assert.match(html, /Below next-best offer/);
  assert.match(html, /16.7% below/);
  assert.match(html, /Shop B/);
  assert.match(html, /3 distinct retailers/);
  assert.match(html, /Phone A/);
  assert.equal(parseDealEvidence({ new_low: 'true' }), null);
});

import {
  insightParams,
  readInsightQuery,
  historyLabel,
  priceTick,
  retailerRankingScore,
  rankRetailers,
  statisticsAPIParams,
} from './insights';
import { trendBounds, zeroBasedAxis } from './chartScale';

test('retailer axes fit category data with readable ticks and nonzero empty ranges', () => {
  assert.deepEqual(zeroBasedAxis([0, 11.7, 21.3], { maximumValue: 100 }), {
    maximum: 25,
    ticks: [0, 5, 10, 15, 20, 25],
  });
  assert.deepEqual(zeroBasedAxis([11, 42, 51], { integerTicks: true }), {
    maximum: 60,
    ticks: [0, 20, 40, 60],
  });
  for (const values of [[], [0, 0], [1, 3], [11, 12], [42, 51], [600, 1900]]) {
    const axis = zeroBasedAxis(values, { integerTicks: true });
    assert.ok(axis.maximum > 0 && axis.maximum >= Math.max(0, ...values));
    assert.equal(axis.ticks[0], 0);
    assert.equal(axis.ticks.at(-1), axis.maximum);
    assert.ok(axis.ticks.every(Number.isInteger));
  }
  assert.equal(zeroBasedAxis([99.9, 100], { maximumValue: 100 }).maximum, 100);
});

test('all statistics tabs request individual offers without leaking configuration filters', () => {
  for (const view of ['overview', 'retailers', 'explorer'] as const) {
    const query = readInsightQuery(
      `?view=${view}&section=GPU&range=90d&retailer=Shop&filters={"gpu":"RTX 4060"}`,
    );
    const params = statisticsAPIParams(query);
    assert.equal(params.get('individual_offers'), 'true');
    assert.equal(params.get('section'), 'GPU');
    assert.equal(params.get('range'), '90d');
    assert.equal(params.get('retailer'), view === 'explorer' ? 'Shop' : null);
    assert.equal(params.get('filters'), view === 'explorer' ? '{"gpu":"RTX 4060"}' : null);
  }
  const all = statisticsAPIParams(readInsightQuery('?view=explorer&section=All&retailer=Shop'));
  assert.equal(all.get('retailer'), 'Shop');
  assert.equal(all.has('filters'), false);
});

test('in-stock chart bounds stay within 0–100 and preserve close-up and signed price scales', () => {
  const options = { minimumPadding: 2, maximumValue: 100 };
  for (const values of [
    [0, 0],
    [100, 100],
    [0, 100],
    [99.8, 100],
    [0, 0.2],
  ]) {
    const { low, high } = trendBounds(values, options);
    assert.ok(low >= 0 && high <= 100 && high > low);
    assert.ok(low <= Math.min(...values) && high >= Math.max(...values));
  }
  assert.deepEqual(trendBounds([70, 74], options), { low: 68, high: 76 });
  assert.deepEqual(trendBounds([100, 100], options), { low: 98, high: 100 });
  const signed = trendBounds([-4, 3], { baseline: 0, minimumPadding: 0.5 });
  assert.ok(signed.low < -4 && signed.high > 3);
  assert.ok(trendBounds([1000, 2000], { minimumPadding: 4 }).high > 2000);
});

test('retailer ranking rewards stronger samples without changing observed win rates', () => {
  const smallPerfect = retailerRankingScore(5, 5);
  const largerStrong = retailerRankingScore(19, 22);
  assert.ok(largerStrong > smallPerfect);
  assert.ok(retailerRankingScore(100, 100) > smallPerfect);
  assert.ok(retailerRankingScore(90, 100) > retailerRankingScore(9, 10));
  assert.ok(Math.abs(largerStrong - 66.6646) < 0.001);
  assert.ok(Math.abs(retailerRankingScore(33, 52) - 49.8714) < 0.001);
  assert.equal(retailerRankingScore(0, 0), 0);
  assert.equal(retailerRankingScore(0, 5), 0);
});
test('price ranking weights small perfect samples and includes near-lowest prices', () => {
  const retailer = (name: string, wins: number, comparisons: number, near = 0) => ({
    name,
    offers: comparisons,
    lowest_price_wins: wins,
    price_comparisons: comparisons,
    within_two_percent: near,
  });
  const stores = [
    retailer('Small perfect', 5, 5),
    retailer('Strong sample', 20, 23, 1),
    retailer('Broad sample', 33, 52, 7),
    retailer('Too few', 4, 4),
  ];
  const ranked = rankRetailers(stores, 'price');
  assert.deepEqual(
    ranked.map((item) => item.name),
    ['Strong sample', 'Broad sample', 'Small perfect'],
  );
  assert.ok(ranked[0].value > ranked[1].value && ranked[1].value > ranked[2].value);
  assert.equal(ranked[2].competitiveShare, 100);
  assert.ok(Math.abs(ranked[2].value - 56.55) < 0.02);
  assert.ok(ranked[0].value > retailerRankingScore(20, 23));
  assert.equal(rankRetailers([retailer('B', 7, 10), retailer('A', 14, 20)], 'price')[0].name, 'A');
});
test('offer coverage ranks individual in-stock products and needs no shared comparisons', () => {
  const make = (name: string, offers: number, stock: number | undefined) => ({
    name,
    offers,
    in_stock_offers: stock,
    lowest_price_wins: 0,
    within_two_percent: 0,
    price_comparisons: 0,
  });
  const ranked = rankRetailers(
    [
      make('BestBuyTunisie', 102, 8),
      make('GamingGearTN', 65, 53),
      make('Small shop', 9, 9),
      make('No stock', 200, 0),
      make('Unknown stock', 500, undefined),
    ],
    'offers',
  );
  assert.deepEqual(
    ranked.map((item) => [item.name, item.value]),
    [
      ['GamingGearTN', 53],
      ['Small shop', 9],
      ['BestBuyTunisie', 8],
    ],
  );
  assert.equal(ranked[2].offers, 102);
});
test('statistics links keep refinements and discard obsolete catalog parameters', () => {
  const query = {
    view: 'explorer' as const,
    section: 'GamingLaptop',
    range: '90d',
    metric: 'price_changes' as const,
    retailerMeasure: 'price' as const,
    retailer: 'A & B',
    filters: { gpu: 'RTX 4060' },
  };
  assert.deepEqual(readInsightQuery(insightParams(query).toString()), query);
  const invalid = readInsightQuery('?filters=bad&page=-2&range=invalid&view=invalid');
  assert.equal(invalid.range, '30d');
  assert.equal(invalid.view, 'overview');
  assert.equal(invalid.section, 'All');
  assert.equal(readInsightQuery('?section=GamingPC').section, 'GamingPC');
  assert.equal(invalid.metric, 'index');
  assert.equal(invalid.retailerMeasure, 'price');
  assert.equal(readInsightQuery('?retailer_measure=invalid').retailerMeasure, 'price');
  assert.equal(readInsightQuery('?retailer_measure=lowest').retailerMeasure, 'price');
  assert.equal(readInsightQuery('?retailer_measure=competitiveness').retailerMeasure, 'price');
  assert.equal(readInsightQuery('?retailer_measure=offers').retailerMeasure, 'offers');
  assert.equal(readInsightQuery('?metric=availability').metric, 'availability');
  assert.equal(readInsightQuery('?view=explorer&metric=lowest_price').metric, 'lowest_price');
  assert.equal(readInsightQuery('?metric=invalid').metric, 'index');
  assert.equal(readInsightQuery('?metric=median_price').metric, 'price_changes');
  assert.deepEqual(invalid.filters, {});
  const oldLink = readInsightQuery(
    '?section=All&retailer=A&filters={"gpu":"RTX"}&q=ASUS&page=3&configuration=cpu&price_change=rose&price_change_page=2',
  );
  assert.deepEqual(oldLink.filters, {});
  const params = insightParams(oldLink);
  for (const key of ['filters', 'q', 'page', 'configuration', 'price_change', 'price_change_page'])
    assert.equal(params.has(key), false);
  assert.equal(params.get('retailer'), 'A');
  assert.equal(historyLabel([]), 'No supported history yet');
  assert.notEqual(priceTick(3999), priceTick(4001));
});

import { parsePriceChanges } from './api';
import { retailerLandscape } from './insights';
import { RetailerLandscape } from '../components/RetailerLandscape';
import { retailerLabelLayout } from './retailerLabels';

test('every retailer keeps its name beside its dot in desktop and mobile plots', () => {
  const stores = Array.from({ length: 32 }, (_, i) => ({
    name: `Retailer ${i.toString().padStart(2, '0')}`,
    value: 45 + (i % 4) * 0.1,
    inStock: i % 3,
  }));
  for (const width of [1280, 314, 280]) {
    const { labels, bottom } = retailerLabelLayout(stores, width, 80, 50);
    assert.equal(labels.length, stores.length);
    assert.equal(new Set(labels.map((label) => label.name)).size, stores.length);
    for (const label of labels) {
      assert.ok(label.x >= 58 && label.x + label.width <= width - 24);
      assert.ok(label.y >= 28 && label.y + 23 <= bottom + 12);
      assert.equal(
        label.dotX,
        58 + (stores.find((s) => s.name === label.name)!.value / 50) * (width - 82),
      );
      const gapX = Math.max(label.x - label.dotX, label.dotX - label.x - label.width, 0);
      const gapY = Math.max(label.y - label.dotY, label.dotY - label.y - 23, 0);
      assert.ok(Math.hypot(gapX, gapY) >= 8 && Math.hypot(gapX, gapY) <= 14);
    }
  }
});

test('retailer landscape uses weighted scores, individual stock counts, and eligible samples', () => {
  const make = (name: string, wins: number, near: number, comparisons: number, stock: number) => ({
    name,
    offers: stock + 20,
    in_stock_offers: stock,
    lowest_price_wins: wins,
    within_two_percent: near,
    two_to_five_percent: 0,
    above_five_percent: comparisons - wins - near,
    price_comparisons: comparisons,
  });
  const retailers = [
    make('Broad selection', 17, 2, 20, 600),
    make('Small shop', 5, 0, 5, 40),
    make('Too few', 4, 0, 4, 100),
    { ...make('Legacy', 10, 0, 20, 40), in_stock_offers: undefined },
  ];
  const stores = retailerLandscape(retailers);
  assert.deepEqual(
    stores.map((store) => [store.name, store.competitiveShare, store.inStock]),
    [
      ['Broad selection', 95, 600],
      ['Small shop', 100, 40],
    ],
  );
  assert.equal(retailers[0].name, 'Broad selection');
  const markup = renderToStaticMarkup(
    createElement(RetailerLandscape, {
      overview: { comparable_configurations: 20, retailers },
      section: 'GamingPC',
      range: '30d',
      onNavigate: () => {},
    }),
  );
  assert.match(
    markup,
    /Broad selection: [\d.]+ price score out of 100, 95\.0% within 2%, 600 products in stock, 620 total offers, 20 price comparisons/,
  );
  assert.match(markup, /section=GamingPC&amp;range=30d&amp;retailer=Broad\+selection/);
  assert.doesNotMatch(markup, /Too few:|Legacy:/);
  assert.doesNotMatch(markup, /Highlight retailer/);
  assert.equal([...markup.matchAll(/class="retailer-landscape-label"/g)].length, stores.length);
  const empty = renderToStaticMarkup(
    createElement(RetailerLandscape, {
      overview: { comparable_configurations: 0, retailers: [retailers[2]] },
      section: 'CPU',
      range: '7d',
      onNavigate: () => {},
    }),
  );
  assert.match(empty, /Not enough shared price comparisons yet/);
  assert.doesNotMatch(empty, /class="retailer-landscape-dot/);
});

test('price-change responses reject inconsistent totals and invalid reference prices', () => {
  const summary = {
    since: '2026-09-20T12:00:00Z',
    until: '2026-09-30T12:00:00Z',
    compared_offers: 3,
    excluded_offers: 2,
    fell: 1,
    rose: 1,
    unchanged: 1,
    selected: 'fell',
    page: 1,
    pages: 1,
    offers: [
      {
        id: 'A',
        title: 'Offer A',
        retailer: 'Shop A',
        before: 100,
        after: 80,
        change_percent: -20,
        in_stock: true,
      },
    ],
  };
  assert.equal(parsePriceChanges(summary)?.fell, 1);
  assert.equal(parsePriceChanges(undefined), null);
  assert.throws(() => parsePriceChanges({ ...summary, compared_offers: 4 }));
  assert.throws(() => parsePriceChanges({ ...summary, selected: 'other' }));
  assert.throws(() =>
    parsePriceChanges({ ...summary, offers: [{ ...summary.offers[0], before: 0 }] }),
  );
  assert.throws(() => parsePriceChanges({ ...summary, page: 2 }));
});

import { addedAgo, retailerDiscount } from './homepage';
test('homepage addition times handle minute and hour boundaries without future negatives', () => {
  const now = Date.parse('2026-09-30T10:00:00Z');
  assert.equal(addedAgo('2026-09-30T08:00:00Z', now), 'Added 2 hours ago');
  assert.equal(addedAgo('2026-09-30T09:59:00Z', now), 'Added 1 minute ago');
  assert.equal(addedAgo('2026-09-29T10:00:00Z', now), 'Added 1 day ago');
  assert.equal(addedAgo('2026-10-01T10:00:00Z', now), 'Added just now');
  assert.equal(addedAgo('invalid', now), '');
});
test('homepage discounts use only a valid retailer original price', () => {
  const product = { price: 80, original_price: 100 } as import('../types').HomeProduct;
  assert.deepEqual(retailerDiscount(product), { original: 100, saving: 20, percent: 20 });
  assert.equal(retailerDiscount({ ...product, original_price: 80 }), null);
  assert.equal(retailerDiscount({ ...product, price: 0 }), null);
});

import { parseConfigurationSummary } from './api';
test('configuration aggregates validate complete counts, finite spreads, and non-overlapping price bands', () => {
  const summary = {
    configuration_count: 30,
    available_configurations: 24,
    shared_configurations: 4,
    average_spread_percent: 7,
    average_spread_dt: 7,
    price_bands: [
      { from: 0, to: 100, count: 0 },
      { from: 100, to: null, count: 24 },
    ],
    spread_bands: [
      { label: '0–2%', count: 1 },
      { label: '>2–5%', count: 1 },
      { label: '>5–10%', count: 1 },
      { label: '>10%', count: 1 },
    ],
  };
  assert.equal(parseConfigurationSummary(undefined), null);
  assert.equal(parseConfigurationSummary(null), null);
  assert.equal(parseConfigurationSummary(summary)?.configuration_count, 30);
  assert.throws(() => parseConfigurationSummary({ ...summary, available_configurations: 31 }));
  assert.throws(() => parseConfigurationSummary({ ...summary, average_spread_percent: Infinity }));
  assert.throws(() => parseConfigurationSummary({ ...summary, shared_configurations: 3 }));
  assert.throws(() =>
    parseConfigurationSummary({ ...summary, price_bands: [{ from: 0, to: null, count: 12 }] }),
  );
  assert.throws(() =>
    parseConfigurationSummary({
      ...summary,
      price_bands: [
        { from: 0, to: 100, count: 0 },
        { from: 90, to: null, count: 24 },
      ],
    }),
  );
  assert.throws(() =>
    parseConfigurationSummary({
      ...summary,
      price_bands: [
        { from: 0, to: 100, count: 0 },
        { from: 110, to: null, count: 24 },
      ],
    }),
  );
  assert.equal(
    parseConfigurationSummary({
      configuration_count: 1,
      available_configurations: 0,
      shared_configurations: 0,
      average_spread_percent: null,
      average_spread_dt: null,
      price_bands: [],
      spread_bands: [],
    })?.available_configurations,
    0,
  );
});

test('price-change size bands cover every change and reject incomplete or invalid counts', () => {
  const magnitude_bands = [0, 2, 5, 10, 20].map((from, index, boundaries) => ({
    from,
    to: boundaries[index + 1] ?? null,
    fell: 1,
    rose: index === 4 ? 2 : 1,
  }));
  const response = {
    since: '2026-09-20T12:00:00Z',
    until: '2026-09-30T12:00:00Z',
    compared_offers: 12,
    excluded_offers: 0,
    fell: 5,
    rose: 6,
    unchanged: 1,
    selected: 'fell',
    page: 1,
    pages: 1,
    offers: [],
    magnitude_bands,
  };
  assert.deepEqual(parsePriceChanges(response)?.magnitude_bands, magnitude_bands);
  assert.equal(
    parsePriceChanges({ ...response, magnitude_bands: undefined })?.magnitude_bands,
    null,
  );
  assert.throws(() =>
    parsePriceChanges({ ...response, magnitude_bands: magnitude_bands.slice(0, 4) }),
  );
  for (const patch of [{ fell: 2 }, { rose: -1 }, { fell: NaN }, { fell: 0.5 }, { to: 3 }]) {
    assert.throws(() =>
      parsePriceChanges({
        ...response,
        magnitude_bands: [{ ...magnitude_bands[0], ...patch }, ...magnitude_bands.slice(1)],
      }),
    );
  }
  const noChanges = magnitude_bands.map((band) => ({ ...band, fell: 0, rose: 0 }));
  assert.deepEqual(
    parsePriceChanges({ ...response, fell: 0, rose: 0, unchanged: 12, magnitude_bands: noChanges })
      ?.magnitude_bands,
    noChanges,
  );
});

test('price-change aggregate summaries preserve signed averages and reject incorrect direction or totals', () => {
  const empty = {
    count: 0,
    average_percent: null,
    average_amount_dt: null,
    largest_percent: null,
    largest_amount_dt: null,
  };
  const fell = {
    count: 20,
    average_percent: -15,
    average_amount_dt: -30,
    largest_percent: -20,
    largest_amount_dt: -100,
  };
  const response = {
    since: '2026-09-20T12:00:00Z',
    until: '2026-09-30T12:00:00Z',
    compared_offers: 22,
    excluded_offers: 2,
    fell: 20,
    rose: 0,
    unchanged: 2,
    selected: 'fell',
    page: 1,
    pages: 2,
    offers: [],
    summary: { fell, rose: empty },
  };
  assert.equal(parsePriceChanges(response)?.summary?.fell.average_percent, -15);
  assert.equal(parsePriceChanges({ ...response, summary: undefined })?.summary, null);
  assert.throws(() =>
    parsePriceChanges({ ...response, summary: { fell: { ...fell, count: 19 }, rose: empty } }),
  );
  assert.throws(() =>
    parsePriceChanges({
      ...response,
      summary: { fell: { ...fell, average_percent: 15 }, rose: empty },
    }),
  );
  assert.throws(() =>
    parsePriceChanges({
      ...response,
      summary: { fell: { ...fell, average_amount_dt: NaN }, rose: empty },
    }),
  );
  assert.throws(() =>
    parsePriceChanges({
      ...response,
      summary: { fell: { ...fell, largest_percent: -10 }, rose: empty },
    }),
  );
  assert.throws(() =>
    parsePriceChanges({ ...response, summary: { fell, rose: { ...empty, average_amount_dt: 0 } } }),
  );
});
