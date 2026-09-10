// GTK4/WebKit6 login only. Tokens stay in this main-frame closure until a bounded
// native main-frame evaluation drains them; cross-frame IPC carries only a wake bit.
(() => {
  if (window !== window.top || location.origin !== "https://discord.com") return;
  const capability = "__SEREIN_LOGIN_CAPABILITY__";
  const opened = Date.now();
  let pending = null;
  let delivered = false;
  const active = () => window === window.top && location.origin === "https://discord.com" && Date.now() - opened <= 600000;
  Object.defineProperty(window, "__serein_login_take_" + capability.slice(0, -1), {
    value: () => {
      const value = active() ? pending : null;
      pending = null;
      return value;
    },
    writable: false,
    configurable: false,
  });
  Object.defineProperty(window, "ipc", {
    value: Object.freeze({ postMessage(value) {
      if (!active() || delivered || typeof value !== "string" || value.length > 2113 || !value.startsWith(capability)) return;
      const token = value.slice(capability.length);
      if (token.length < 16 || token.length > 2048 || !/^[\x21-\x7e]+$/.test(token)) return;
      delivered = true;
      pending = value;
      window.webkit.messageHandlers.sereinLogin.postMessage(true);
    }}),
    writable: false,
    configurable: false,
  });
})();
