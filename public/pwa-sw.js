// Minimal PWA service worker: offline app-shell caching via
// stale-while-revalidate for same-origin GETs.
//
// IMPORTANT: this is only registered on hosts that provide cross-origin
// isolation via real HTTP headers (see index.html), where the COI
// service-worker shim stays dormant. That avoids two service workers
// fighting over the same scope.

const CACHE = "pixelwise-shell-v1";

self.addEventListener("install", (e) => {
  self.skipWaiting();
});

self.addEventListener("activate", (e) => {
  e.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)));
      await self.clients.claim();
    })()
  );
});

self.addEventListener("fetch", (e) => {
  const req = e.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;

  e.respondWith(
    (async () => {
      const cache = await caches.open(CACHE);
      const cached = await cache.match(req);
      const network = fetch(req)
        .then((res) => {
          if (res && res.ok) cache.put(req, res.clone());
          return res;
        })
        .catch(() => cached);
      // Serve cache immediately when present, refresh in the background.
      return cached || network;
    })()
  );
});
