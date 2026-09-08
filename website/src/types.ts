export interface Category {
  id: string;
  name: string;
  url: string;
}
export interface ServerInfo {
  categories: Record<string, Category[]>;
}
export interface Product {
  id: string;
  url: string;
  image: string;
  status: string;
  site: string;
  section: string;
  title: string;
  price: number;
  original_price: number | null;
  description: string | null;
  components: Record<string, string>;
  filter_ids: Record<string, string>;
  history: unknown[];
  updated_at: string | null;
  added_at: string | null;
  views: number;
  missing_group_fields: string[];
}
export interface VariantSummary {
  id: string;
  name: string;
  section: string;
  image: string;
  price: number;
  original_price: number | null;
  components: Record<string, string>;
  offer_count: number;
  retailer_count: number;
  in_stock_count: number;
  views: number;
}
export type CatalogItem =
  | { type: 'product'; data: Product }
  | { type: 'variant'; data: VariantSummary };
export interface VariantChartPoint {
  timestamp: string;
  lowest_price: number | null;
  lowest_price_retailer: string | null;
  offer_count: number;
  in_stock_count: number;
  retailer_count: number;
}
export interface VariantChart {
  scope?: 'product' | 'variant';
  range: string;
  interval: string;
  currency: string;
  points: VariantChartPoint[];
  summary: {
    current_price: number | null;
    period_low: number | null;
    period_high: number | null;
    all_time_low: number | null;
    average_price: number | null;
    ninety_day_average: number | null;
  };
}
export interface FilterValue {
  id: string;
  label: string;
  count: number;
  children?: FilterValue[];
}
export interface FilterGroup {
  key: string;
  label: string;
  options: FilterValue[];
}
export interface SectionInfo {
  filters: FilterGroup[];
  sites: string[];
  components: string[];
  components_by_section: Record<string, string[]>;
  group_by: { key: string; required: boolean }[];
  min_price: number;
  max_price: number;
}
export interface PageResponse {
  section_info: SectionInfo | null;
  items: CatalogItem[];
  total_items: number;
  total_products: number;
  total_pages: number;
  page: number;
}
export interface CatalogQuery {
  section: string;
  search: string;
  min_price: string;
  max_price: string;
  site: string[];
  stock: string[];
  filters: Record<string, string[]>;
  sort: 'price_asc' | 'price_desc';
  grouped: boolean;
  groupBy: string[] | null;
  page: number;
}
