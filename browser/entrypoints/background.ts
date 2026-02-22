export default defineBackground(() => {
  chrome.runtime.onMessage.addListener(
    (
      msg: { type: string; host?: string },
      _sender,
      sendResponse
    ) => {
      if (msg.type === 'resolve' && msg.host) {
        resolveDeployment(msg.host)
          .then(sendResponse)
          .catch((err) => sendResponse({ error: String(err) }));
        return true; // async response
      }
    }
  );
});

async function resolveDeployment(host: string): Promise<unknown> {
  const url = `https://api.appz.dev/v0/extension/resolve?host=${encodeURIComponent(host)}`;
  const res = await fetch(url, { credentials: 'include' });
  if (!res.ok) return { error: `HTTP ${res.status}` };
  return res.json();
}
