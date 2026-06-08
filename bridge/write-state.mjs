import fs from 'node:fs/promises';
import path from 'node:path';

export async function writeStateAtomically(filePath, state) {
  const targetPath = path.resolve(filePath);
  const dirPath = path.dirname(targetPath);
  const tempPath = path.join(
    dirPath,
    `.${path.basename(targetPath)}.${process.pid}.${Date.now()}.tmp`
  );

  await fs.mkdir(dirPath, { recursive: true });
  await fs.writeFile(tempPath, `${JSON.stringify(state, null, 2)}\n`, 'utf8');
  await fs.rename(tempPath, targetPath);
}
