import fs from 'node:fs/promises';

export async function appendDebugLog(logPath, entry) {
  const line = `${JSON.stringify(entry)}\n`;
  await fs.appendFile(logPath, line, 'utf8');
}
