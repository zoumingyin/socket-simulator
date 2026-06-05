import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';

const url = import.meta.url;
console.log('import.meta.url:', url);

const fp = fileURLToPath(url);
console.log('fileURLToPath:', fp);

console.log('process.cwd():', process.cwd());
console.log('platform:', process.platform);

// 检查 cwd 是否包含换行符
const cwd = process.cwd();
console.log('cwd contains \\\\n?:', cwd.includes('\n'));
console.log('cwd char codes at position 9:', cwd.charCodeAt(9), cwd.charCodeAt(10));

// 尝试直接拼接路径
const configDir = path.resolve(process.cwd(), '../config');
console.log('resolved configDir:', configDir);

// 检查路径是否可访问
try {
  const files = fs.readdirSync(configDir);
  console.log('configDir readable, files:', files);
} catch (e) {
  console.log('configDir NOT readable:', e.message);
  // 尝试替代路径
  const alt = 'E:\\work\\nengna\\git\\socket-service-manager\\config';
  console.log('trying hardcoded:', alt);
  try {
    const files = fs.readdirSync(alt);
    console.log('hardcoded path works:', files);
  } catch (e2) {
    console.log('hardcoded also fails:', e2.message);
  }
}
