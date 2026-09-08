import assert from 'node:assert/strict';

const base = process.env.MYTHOS_TEST_URL || 'http://127.0.0.1:3000';
const get = async (path) => {
  const response = await fetch(base + path);
  assert.equal(response.status, 200, path);
  return response.json();
};
const info = await get('/api/info');
assert.deepEqual(Object.keys(info.categories), [
  'Computers',
  'Components',
  'Peripherals',
  'Consoles',
  'Smartphones & TVs',
]);
const ids = Object.values(info.categories)
  .flat()
  .map((category) => category.id);
assert.equal(new Set(ids).size, ids.length);

// Requires a populated saved GamingLaptop catalog.
const initial = await get('/api/GamingLaptop/products?send_section_info=true');
assert(initial.items.length, 'Load saved GamingLaptop data before running smoke checks.');
const bounds = (result) => [
  result.section_info.sites,
  result.section_info.min_price,
  result.section_info.max_price,
  result.section_info.components,
];
for (const query of [
  'search=NO_MATCH_729A',
  'site=Mytek',
  'stock=InStock',
  'min_price=2200&max_price=3000',
]) {
  const page = await get('/api/GamingLaptop/products?send_section_info=true&' + query);
  assert.deepEqual(bounds(page), bounds(initial), 'Stable metadata for ' + query);
}
const cpu = initial.section_info.filters.find((filter) => filter.key === 'cpu');
const query = new URLSearchParams({
  send_section_info: 'true',
  filters: JSON.stringify({ cpu: [cpu.options[0].id] }),
});
const selected = await get('/api/GamingLaptop/products?' + query);
assert.deepEqual(
  selected.section_info.filters.find((filter) => filter.key === 'cpu'),
  cpu,
  'Facet excludes its own selection',
);
assert.deepEqual(bounds(selected), bounds(initial));
assert.equal((await get('/api/GamingLaptop/products')).section_info, null);
for (const invalid of ['min_price=-1', 'max_price=2147483648', 'min_price=5000&max_price=1000']) {
  assert.equal((await fetch(base + '/api/GamingLaptop/products?' + invalid)).status, 400, invalid);
}
const last = await get('/api/GamingLaptop/products?page=18446744073709551615');
assert.equal(last.page, last.total_pages);
assert.equal((await get('/api/GamingLaptop/products?search=NO_MATCH_729A&page=999')).page, 1);
const first = await get('/api/GamingLaptop/products');
assert.deepEqual(
  first.items.map((item) => item.data.id),
  (await get('/api/GamingLaptop/products')).items.map((item) => item.data.id),
);
for (const sort of ['price_asc', 'price_desc']) {
  const groups = await get('/api/GamingLaptop/products?grouping_mode=true&sort=' + sort);
  const prices = groups.items.map((item) => item.data.price);
  assert(
    prices.every(
      (price, index) =>
        !index || (sort === 'price_asc' ? prices[index - 1] <= price : prices[index - 1] >= price),
    ),
    'Grouped order ' + sort,
  );
}
assert.equal((await fetch(base + '/GamingLaptop')).status, 200);
console.log(
  'Live checks passed: ordered navigation, stable metadata, facet counts, validation, pagination, repeatable results, and grouped ordering.',
);

for (const path of ['/api/products?search=RTX', '/products?search=RTX', '/search']) {
  const response = await fetch(base + path);
  assert([400, 404].includes(response.status), 'Removed global search: ' + path);
}
const fields = initial.section_info.group_by;
const grouped = await get('/api/GamingLaptop/products?' + new URLSearchParams([
  ['grouping_mode', 'true'], ...fields.map((field) => ['group_by', field.key]),
]));
assert(grouped.total_items < grouped.total_products, 'Optional fields group saved components');
const requiredOnly = await get('/api/GamingLaptop/products?' + new URLSearchParams([
  ['grouping_mode', 'true'], ...fields.filter((field) => field.required).map((field) => ['group_by', field.key]),
]));
assert(requiredOnly.total_items <= grouped.total_items, 'Optional fields refine groups');
const variant = grouped.items.find((item) => item.type === 'variant');
assert(variant, 'Grouped cards include a variant');
const detail = await get('/api/variants/' + encodeURIComponent(variant.data.id));
assert(detail, 'Optional group details resolve');
console.log('Global search removal and optional grouping checks passed.');
