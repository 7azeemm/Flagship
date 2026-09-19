import test from 'node:test';
import assert from 'node:assert/strict';
import {
  apiQueryParams,
  sectionInfoKey,
  clampRange,
  defaultQuery,
  nodeStatus,
  productSpecs,
  queryParams,
  queryURL,
  readQuery,
  safeURL,
  toggleNode,
  pageNumbers,
} from './catalog';
import { parsePage, parseProduct, parseServerInfo } from './api';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Filters } from '../components/Filters';

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
  const url = new URL(queryURL(query), 'https://mythos.test');
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
    readQuery({ pathname: '/CPU', search: '?filters=%7B%22cpu%22%3A%5B%22Intel%2FCore+i7%22%5D%7D' }).filters,
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
    { stock: [] },
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
});
import { buildProductHistoryChart } from './productHistory';
test('ungroupable product gets a price and stock chart without variant history', () => {
  const product = parseProduct({
    id: 'standalone',
    title: 'Dell',
    price: 1425,
    status: 'InStock',
    added_at: '2026-09-01T00:00:00Z',
    history: null,
  });
  const chart = buildProductHistoryChart(product, '90d', 'auto', Date.parse('2026-09-07T00:00:00Z'));
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
  const chart = buildProductHistoryChart(product, '30d', 'scan', Date.parse('2026-09-07T00:00:00Z'));
  assert.equal(chart.points[0].timestamp, '2026-08-08T00:00:00.000Z');
  assert.equal(chart.points[0].lowest_price, 150);
  assert.equal(chart.points.at(-1)?.in_stock_count, 0);
  assert.equal(chart.summary.period_low, 100);
  assert.equal(buildProductHistoryChart(product, 'all', 'week').interval, 'week');
});
import { classifyPrice, ProductAnalytics } from '../components/ProductAnalytics';
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
  assert.doesNotMatch(render([product], '/variants/a'), /detail-listings/);
  const grouped = render(
    [product, { ...product, id: 'b', title: 'Product B', site: 'Shop B' }],
    '/variants/a',
  );
  assert.match(grouped, /detail-listings/);
  assert.match(grouped, /<b>Shop B<\/b> · Product B/);
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
            new_value: { cpu: { id: 'Intel/Core i5/Intel Core i5-12400', label: 'Intel Core i5-12400' } },
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
test('product page renders the requested price and availability charts without sliders', () => {
  const product = parseProduct({
    id: 'dell',
    title: 'Dell Vostro',
    price: 1425,
    status: 'InStock',
    added_at: '2026-09-01T00:00:00Z',
  });
  const html = renderToStaticMarkup(
    createElement(ProductAnalytics, { initialChart: null, endpoint: '/products/dell', product }),
  );
  assert.equal((html.match(/<svg/g) || []).length, 2);
  assert.match(html, /linearGradient/);
  assert.match(html, /Listed price/);
  assert.match(html, /Availability/);
  assert.doesNotMatch(html, /Typical Price/);
  assert.match(html, />7D</);
  assert.match(html, />All</);
  assert.doesNotMatch(html, /type="range"/);
  assert.match(html, / d="[^"]* C /);
  assert.doesNotMatch(html, /No price observations/);
  assert.match(html, /Price and availability recorded for this retailer listing/);
});

test('price labels use five tiers against the 90-day average', () => {
  assert.equal(classifyPrice(84, 100).label, 'Exceptional Price');
  assert.equal(classifyPrice(91, 100).label, 'Great Price');
  assert.equal(classifyPrice(96, 100).label, 'Good Price');
  assert.equal(classifyPrice(100, 100).label, 'Typical Price');
  assert.equal(classifyPrice(109, 100).label, 'High Price');
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
    readQuery(new URL(info.categories.Components[0].url, 'https://mythos.test')).filters,
    { platform: ['Laptop'] },
  );
  assert.deepEqual(
    readQuery(new URL(info.categories.Components[1].url, 'https://mythos.test')).filters,
    { platform: ['Desktop'] },
  );
  assert.throws(() => parseServerInfo({ Bad: [{ name: 'External', url: '//example.com' }] }));
  assert.throws(() => parseServerInfo({ Bad: [{ name: 'Script', url: 'javascript:alert(1)' }] }));
});

test('category search and grouping defaults survive shared URLs', () => {
  assert.equal(readQuery({ pathname: '/GPU', search: '' }).grouped, true);
  const query = { ...defaultQuery('GPU'), grouped: false, search: 'RTX 5070' };
  assert.deepEqual(readQuery(new URL(queryURL(query), 'https://mythos.test')), query);
  assert.equal(apiQueryParams(query, false).get('search'), 'RTX 5070');
  assert.equal(apiQueryParams(query, false).get('grouping_mode'), 'false');
});
