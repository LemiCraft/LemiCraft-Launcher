const PROXY = 'https://lemicraft.ru/api/proxy/image?url=';

// A picture of Discord that a provider does not let through is asked once again through the site
export function proxiedImageOnError(event) {
  const img = event.target;
  const src = img.getAttribute('src') || '';
  if (!src.startsWith('https://cdn.discordapp.com/') || img.dataset.proxied === src) return;
  img.dataset.proxied = src;
  img.src = PROXY + encodeURIComponent(src);
}
