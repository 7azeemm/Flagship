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
});
test('parent selections replace descendants without matching unrelated prefixes', () => {
  assert.deepEqual(toggleNode(['Intel/Core i7', 'AMD'], 'Intel'), ['AMD', 'Intel']);
  assert.deepEqual(toggleNode(['Intel', 'IntelOther'], 'Intel'), ['IntelOther']);
  assert.deepEqual(toggleNode(['Intel'], 'Intel/Core i7'), ['Intel', '!Intel/Core i7']);
  assert.equal(nodeStatus({ id: 'Intel', label: 'Intel', count: 3 }, ['Intel/Core i7']), 'partial');
  assert.equal(
    nodeStatus({ id: 'Intel/Core i7', label: 'Core i7', count: 3 }, ['Intel']),
    'checked',
  );
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
    components: { cpu: 'Core i5', memory: '16GB', invalid: 4 },
  });
  assert.equal(product.original_price, 1800);
  assert.equal(product.status, 'Unknown');
  assert.equal(product.image, '');
  assert.deepEqual(
    productSpecs(product, ['memory', 'cpu', 'gpu']).map((spec) => spec.value),
    ['16GB', 'Core i5', 'Unknown'],
  );
  assert.throws(() => parseProduct({ title: 'bad', price: 'NaN' }));
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
test('grouped API summaries do not require a removed name field', () => {
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
          id: 'v',
          section: 'CPU',
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
  if (page.items[0].type === 'variant') assert.equal(page.items[0].data.name, 'v');
});
import { productHistoryChart } from './productHistory';
test('ungroupable product gets a price and stock chart without variant history', () => {
  const product = parseProduct({
    id: 'standalone',
    title: 'Dell',
    price: 1425,
    status: 'InStock',
    added_at: '2026-09-01T00:00:00Z',
    history: null,
  });
  const chart = productHistoryChart(product, '90d', 'auto', Date.parse('2026-09-07T00:00:00Z'));
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
    history: [
      { field: 'price', old_value: 150, new_value: 100, timestamp: '2026-09-02T00:00:00Z' },
      {
        field: 'status',
        old_value: 'InStock',
        new_value: 'OutOfStock',
        timestamp: '2026-09-03T00:00:00Z',
      },
    ],
  });
  const chart = productHistoryChart(product, '30d', 'scan', Date.parse('2026-09-07T00:00:00Z'));
  assert.equal(chart.points[0].timestamp, '2026-08-08T00:00:00.000Z');
  assert.equal(chart.points[0].lowest_price, 150);
  assert.equal(chart.points.at(-1)?.in_stock_count, 0);
  assert.equal(chart.summary.period_low, 100);
  assert.equal(productHistoryChart(product, 'all', 'week').interval, 'week');
});
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
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
  const render = (products: (typeof product)[], endpoint: string) =>
    renderToStaticMarkup(
      createElement(ProductDialog, {
        products,
        chart: null,
        fields: ['cpu'],
        endpoint,
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
  assert.match(html, /Lowest price/);
  assert.match(html, /Availability/);
  assert.match(html, /Typical Price/);
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

import { retailerHistory } from './retailerHistory';
test('retailer comparison uses the lowest available offer and preserves stock gaps', () => {
  const now = Date.parse('2026-09-08T00:00:00Z');
  const first = parseProduct({
    id: 'a',
    title: 'A',
    site: 'Shop',
    price: 120,
    status: 'InStock',
    added_at: '2026-09-01T00:00:00Z',
    history: [
      { field: 'price', timestamp: '2026-09-03T00:00:00Z', old_value: 100, new_value: 120 },
      {
        field: 'status',
        timestamp: '2026-09-02T00:00:00Z',
        old_value: 'OutOfStock',
        new_value: 'InStock',
      },
    ],
  });
  const second = { ...first, id: 'b', price: 150, history: [] };
  const alone = retailerHistory([first], 'all', 'scan', now)[0];
  assert.equal(alone.points[0].lowest_price, null);
  assert.equal(alone.low, 100);
  assert.equal(alone.high, 120);
  const combined = retailerHistory([first, second], 'all', 'scan', now)[0];
  assert.equal(combined.points[0].lowest_price, 150);
  assert.equal(combined.current, 120);
  assert.equal(combined.low, 100);
  assert.equal(combined.high, 150);
  const unavailable = retailerHistory(
    [{ ...first, status: 'OutOfStock', history: [] }],
    'all',
    'scan',
    now,
  )[0];
  assert.equal(unavailable.current, null);
  assert.equal(unavailable.low, null);
});

test('category search and grouping defaults survive shared URLs', () => {
  assert.equal(readQuery({ pathname: '/GPU', search: '' }).grouped, true);
  const query = { ...defaultQuery('GPU'), grouped: false, search: 'RTX 5070' };
  assert.deepEqual(readQuery(new URL(queryURL(query), 'https://mythos.test')), query);
  assert.equal(apiQueryParams(query, false).get('search'), 'RTX 5070');
  assert.equal(apiQueryParams(query, false).get('grouping_mode'), 'false');
});
