import Database from 'better-sqlite3';
import { join } from 'path';
import { env } from 'process';

const dbPath = join(env.APPDATA, 'work-review', 'workreview.db');
console.log('DB path:', dbPath);

const db = new Database(dbPath, { readonly: true });

const rows = db.prepare(`
  SELECT id, app_name, screenshot_path, 
         datetime(timestamp, 'unixepoch', 'localtime') as time
  FROM activities 
  ORDER BY timestamp DESC 
  LIMIT 15
`).all();

console.log('\nRecent activities:');
rows.forEach(r => {
  const sp = r.screenshot_path || '(empty)';
  const spShort = sp.length > 60 ? sp.slice(0, 60) + '...' : sp;
  console.log(`  id=${r.id} | ${r.time} | ${r.app_name} | screenshot_path: ${spShort}`);
});

const withScreenshot = db.prepare("SELECT COUNT(*) as cnt FROM activities WHERE screenshot_path != ''").get();
const withoutScreenshot = db.prepare("SELECT COUNT(*) as cnt FROM activities WHERE screenshot_path = ''").get();
console.log(`\nWith screenshot: ${withScreenshot.cnt}, Without screenshot: ${withoutScreenshot.cnt}`);

db.close();