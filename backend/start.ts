/**
 * start.ts - 打包入口（esbuild → pkg）
 *
 * 后端被打包成独立 exe 后，main.ts 顶层的 isMain 检测
 * （判断 process.argv[1] 是否以 main.js / main.ts / start.js / start.ts 结尾）
 * 会因文件名变为 backend-exe.exe 而失效，导致 startAll() 不会被自动调用。
 * 因此这里显式调用 startAll() 作为打包后的真正入口。
 */
import { startAll } from './main.js';

startAll().catch((err) => {
  console.error('[start] 启动失败:', err);
  process.exit(1);
});
