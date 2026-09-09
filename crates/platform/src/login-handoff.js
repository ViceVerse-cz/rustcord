// Runs only in Serein's newly-created ephemeral authentication webview.
// Observe its own same-origin API Authorization header after the owner logs in.
// Discord still performs the entire authentication/challenge/QR flow.
(() => {
  if (window !== window.top || location.origin !== "https://discord.com") return;
  const capability = "__SEREIN_LOGIN_CAPABILITY__";
  let delivered = false;
  const opened = Date.now();
  const allowed = value => {
    try {
      const url = new URL(value, location.href);
      return url.origin === "https://discord.com" && /^\/api\/v\d+\//.test(url.pathname);
    } catch { return false; }
  };
  const deliver = value => {
    if (delivered || Date.now() - opened > 600000 || typeof value !== "string" || value.length < 16 || value.length > 2048 || /\s/.test(value)) return;
    delivered = true;
    window.ipc.postMessage(capability + value);
  };
  const destinations = new WeakMap();
  const open = XMLHttpRequest.prototype.open;
  const setHeader = XMLHttpRequest.prototype.setRequestHeader;
  XMLHttpRequest.prototype.open = function(method, url, ...rest) {
    destinations.set(this, allowed(url));
    return open.call(this, method, url, ...rest);
  };
  XMLHttpRequest.prototype.setRequestHeader = function(name, value) {
    if (destinations.get(this) && String(name).toLowerCase() === "authorization") deliver(value);
    return setHeader.call(this, name, value);
  };
  const originalFetch = window.fetch;
  window.fetch = function(input, init) {
    if (allowed(input instanceof Request ? input.url : input)) {
      const headers = new Headers(init?.headers ?? (input instanceof Request ? input.headers : undefined));
      deliver(headers.get("authorization"));
    }
    return originalFetch.apply(this, arguments);
  };
})();
