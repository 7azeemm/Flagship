import { useEffect, useRef, useState } from 'react';
import type { CatalogQuery, PageResponse, SectionInfo, ServerInfo } from '../types';
import { parsePage, parseServerInfo, request } from '../lib/api';
import { defaultQuery, apiQueryParams, sectionInfoKey, queryURL, readQuery } from '../lib/catalog';

export function useCatalog() {
  const [query, setQuery] = useState(() => readQuery(window.location));
  const [categories, setCategories] = useState<ServerInfo['categories']>({});
  const [page, setPage] = useState<PageResponse | null>(null);
  const [base, setBase] = useState<SectionInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const cache = useRef(new Map<string, SectionInfo>());
  const metadataKey = useRef<string | null>(null);
  const serverInfo = useRef<ServerInfo | null>(null);

  useEffect(() => {
    const pop = () => setQuery(readQuery(window.location));
    window.addEventListener('popstate', pop);
    return () => window.removeEventListener('popstate', pop);
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError('');
    setPage(null);
    setBase(cache.current.get(query.section) || null);
    const load = async () => {
      try {
        const info =
          serverInfo.current || parseServerInfo(await request('/info', controller.signal));
        if (controller.signal.aborted) return;
        serverInfo.current = info;
        setCategories(info.categories);
        const available = Object.values(info.categories).flat();
        if (!available.length) return;
        const category = available.find(
          (category) => category.id.toLowerCase() === query.section.toLowerCase(),
        );
        if (!category) throw new Error('This category is not available. Choose another category.');
        const result = parsePage(
          await request(
            `${`/${category.id}/products`}?${apiQueryParams(
              query,
              metadataKey.current !== sectionInfoKey(query),
            )}`,
            controller.signal,
          ),
        );
        if (controller.signal.aborted) return;
        if (result.section_info) {
          cache.current.set(query.section, result.section_info);
          metadataKey.current = sectionInfoKey(query);
        }
        setBase(result.section_info || cache.current.get(query.section) || null);
        if (query.page !== result.page) {
          const next = { ...query, page: result.page };
          window.history.replaceState(null, '', queryURL(next));
          setQuery(next);
          return;
        }
        setPage(result);
      } catch (cause) {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'The catalog could not be loaded.');
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    };
    void load();
    return () => controller.abort();
  }, [query, revision]);

  const update = (changes: Partial<CatalogQuery>, replace = false) => {
    const next = { ...query, ...changes, page: changes.page ?? 1 };
    window.history[replace ? 'replaceState' : 'pushState'](null, '', queryURL(next));
    setQuery(next);
  };
  return {
    query,
    categories,
    sections: Object.values(categories)
      .flat()
      .map((category) => category.id),
    page,
    base,
    loading,
    error,
    update,
    retry: () => setRevision((value) => value + 1),
    changeSection: (url: string) =>
      update(readQuery(new URL(url.replace(/^\/?/, '/'), window.location.origin))),
    reset: () =>
      update({
        ...defaultQuery(query.section),
        sort: query.sort,
        grouped: query.grouped,
        groupBy: query.groupBy,
      }),
  };
}
