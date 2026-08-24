const BASE = 'http://127.0.0.1:47831';
const HEADERS = { 'Content-Type': 'application/json', 'X-Forwarded-Host': 'localhost:5173' };

async function main() {
  const getRes = await fetch(`${BASE}/v1/config`, { headers: HEADERS });
  const config = await getRes.json();
  console.log('Current screenshots_enabled:', config.storage.screenshots_enabled);

  config.storage.screenshots_enabled = true;

  const putRes = await fetch(`${BASE}/v1/config`, {
    method: 'PUT',
    headers: HEADERS,
    body: JSON.stringify(config),
  });

  if (putRes.ok) {
    const result = await putRes.json();
    console.log('Update result:', JSON.stringify(result));
  } else {
    const text = await putRes.text();
    console.error('Update failed:', putRes.status, text);
  }

  const verifyRes = await fetch(`${BASE}/v1/config`, { headers: HEADERS });
  const verifyConfig = await verifyRes.json();
  console.log('Verified screenshots_enabled:', verifyConfig.storage.screenshots_enabled);
}

main().catch(console.error);