import useSWR, { SWRConfiguration } from "swr";
import { fetchApi } from "./api";

/**
 * P3: SWR-backed API hook — stale-while-revalidate for instant repeat renders.
 *
 * Usage:
 *   const { data, error, isLoading } = useApi<FleetHealth>("/fleet/health", { refreshInterval: 5000 });
 *   const { data } = useApi<GameCatalog>("/games/catalog"); // cached across navigations
 */
export function useApi<T>(path: string | null, config?: SWRConfiguration) {
  return useSWR<T>(
    path,
    (url: string) => fetchApi<T>(url),
    {
      dedupingInterval: 2000,
      revalidateOnFocus: true,
      ...config,
    }
  );
}
